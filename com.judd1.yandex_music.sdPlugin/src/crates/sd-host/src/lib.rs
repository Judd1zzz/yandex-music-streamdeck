use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use sd_protocol::{outbound::register_message, Inbound, Outbound};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

const OUT_CHANNEL_CAP: usize = 256;
const IN_CHANNEL_CAP: usize = 256;
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_BACKOFF: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub port: u16,
    pub plugin_uuid: String,
    pub register_event: String,
    pub max_retries: u32,
    pub backoff: Duration,
}

impl HostConfig {
    pub fn new(port: u16, plugin_uuid: impl Into<String>, register_event: impl Into<String>) -> Self {
        Self {
            port,
            plugin_uuid: plugin_uuid.into(),
            register_event: register_event.into(),
            max_retries: DEFAULT_MAX_RETRIES,
            backoff: DEFAULT_BACKOFF,
        }
    }
}

pub type HostTx = mpsc::Sender<Outbound>;

pub struct HostHandle {
    pub tx: HostTx,
    pub inbound: mpsc::Receiver<Inbound>,
    pub task: JoinHandle<()>,
}

pub fn spawn(cfg: HostConfig) -> HostHandle {
    let (out_tx, out_rx) = mpsc::channel::<Outbound>(OUT_CHANNEL_CAP);
    let (in_tx, in_rx) = mpsc::channel::<Inbound>(IN_CHANNEL_CAP);
    let task = tokio::spawn(run(cfg, out_rx, in_tx));
    HostHandle { tx: out_tx, inbound: in_rx, task }
}

enum End {
    ClosedByHost,
    Errored,
}

enum Served {
    ConnectFailed,
    Connected(End),
}

async fn run(cfg: HostConfig, mut out_rx: mpsc::Receiver<Outbound>, in_tx: mpsc::Sender<Inbound>) {
    let mut retries = 0u32;
    loop {
        match connect_and_serve(&cfg, &mut out_rx, &in_tx).await {
            Served::ConnectFailed => {
                retries += 1;
                if retries >= cfg.max_retries {
                    tracing::error!("host: достигнут лимит {} попыток подключения, выход", cfg.max_retries);
                    break;
                }
                tokio::time::sleep(cfg.backoff).await;
            }
            Served::Connected(End::ClosedByHost) => {
                tracing::info!("host: соединение закрыто хостом — завершаюсь");
                break;
            }
            Served::Connected(End::Errored) => {
                retries = 0;
                tracing::warn!("host: ошибка соединения, переподключаюсь через {:?}", cfg.backoff);
                tokio::time::sleep(cfg.backoff).await;
            }
        }
    }
}

async fn connect_and_serve(
    cfg: &HostConfig,
    out_rx: &mut mpsc::Receiver<Outbound>,
    in_tx: &mpsc::Sender<Inbound>,
) -> Served {
    let url = format!("ws://127.0.0.1:{}", cfg.port);
    let ws = match tokio_tungstenite::connect_async(&url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            tracing::warn!("host: не удалось подключиться к {url}: {e}");
            return Served::ConnectFailed;
        }
    };
    let (mut write, mut read) = ws.split();

    let reg = register_message(&cfg.register_event, &cfg.plugin_uuid).to_string();
    if write.send(Message::Text(reg)).await.is_err() {
        return Served::Connected(End::Errored);
    }
    let ggs = serde_json::to_string(&Outbound::GetGlobalSettings {
        context: cfg.plugin_uuid.clone(),
    })
    .unwrap_or_default();
    if write.send(Message::Text(ggs)).await.is_err() {
        return Served::Connected(End::Errored);
    }
    tracing::info!("host: подключено к {url}, зарегистрировано");

    let end = loop {
        tokio::select! {
            incoming = read.next() => match incoming {
                Some(Ok(Message::Text(t))) => match serde_json::from_str::<Inbound>(t.as_str()) {
                    Ok(ev) => {
                        if in_tx.send(ev).await.is_err() {
                            break End::ClosedByHost;
                        }
                    }
                    Err(e) => tracing::warn!("host: не разобрал входящее событие: {e}"),
                },
                Some(Ok(Message::Ping(p))) => {
                    let _ = write.send(Message::Pong(p)).await;
                }
                Some(Ok(Message::Close(_))) | None => break End::ClosedByHost,
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    tracing::warn!("host: ошибка чтения WS: {e}");
                    break End::Errored;
                }
            },
            out = out_rx.recv() => match out {
                Some(o) => {
                    let txt = serde_json::to_string(&o).unwrap_or_default();
                    if write.send(Message::Text(txt)).await.is_err() {
                        break End::Errored;
                    }
                }
                None => break End::ClosedByHost,
            },
        }
    };
    Served::Connected(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, WebSocketStream};

    async fn next_text(ws: &mut WebSocketStream<tokio::net::TcpStream>) -> String {
        loop {
            match ws.next().await.unwrap().unwrap() {
                Message::Text(t) => return t.as_str().to_owned(),
                Message::Close(_) => panic!("неожиданный Close"),
                _ => continue,
            }
        }
    }

    fn free_port() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    }

    #[tokio::test]
    async fn handshake_then_forwards_inbound_then_shuts_down_on_close() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(sock).await.unwrap();
            let reg = next_text(&mut ws).await;
            let ggs = next_text(&mut ws).await;
            ws.send(Message::Text(
                r#"{"event":"willAppear","context":"ctx","action":"a","payload":{"settings":{}}}"#.into(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            ws.send(Message::Close(None)).await.unwrap();
            (reg, ggs)
        });

        let mut h = spawn(HostConfig::new(port, "U", "registerPlugin"));

        let ev = tokio::time::timeout(Duration::from_secs(2), h.inbound.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(ev, Inbound::WillAppear(_)));

        tokio::time::timeout(Duration::from_secs(2), h.task)
            .await
            .expect("task должна завершиться после Close")
            .unwrap();

        let (reg, ggs) = server.await.unwrap();
        let reg: serde_json::Value = serde_json::from_str(&reg).unwrap();
        assert_eq!(reg["event"], "registerPlugin");
        assert_eq!(reg["uuid"], "U");
        let ggs: serde_json::Value = serde_json::from_str(&ggs).unwrap();
        assert_eq!(ggs["event"], "getGlobalSettings");
        assert_eq!(ggs["context"], "U");
    }

    #[tokio::test]
    async fn outbound_is_delivered_to_host() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(sock).await.unwrap();
            let _reg = next_text(&mut ws).await;
            let _ggs = next_text(&mut ws).await;
            let outbound = next_text(&mut ws).await;
            ws.send(Message::Close(None)).await.unwrap();
            outbound
        });

        let h = spawn(HostConfig::new(port, "U", "registerPlugin"));
        h.tx
            .send(Outbound::ShowOk { context: "c1".into() })
            .await
            .unwrap();

        let outbound = tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .unwrap()
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&outbound).unwrap();
        assert_eq!(v["event"], "showOk");
        assert_eq!(v["context"], "c1");
    }

    #[tokio::test]
    async fn reconnects_and_reregisters_after_connection_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(sock).await.unwrap();
            let first_reg = next_text(&mut ws).await;
            let _ggs = next_text(&mut ws).await;
            use tokio::io::AsyncWriteExt;
            ws.get_mut().write_all(b"\xff\xff not a websocket frame").await.unwrap();
            drop(ws);

            let (sock2, _) = listener.accept().await.unwrap();
            let mut ws2 = accept_async(sock2).await.unwrap();
            let second_reg = next_text(&mut ws2).await;
            let _ggs2 = next_text(&mut ws2).await;
            ws2.send(Message::Close(None)).await.unwrap();
            (first_reg, second_reg)
        });

        let mut cfg = HostConfig::new(port, "U", "registerPlugin");
        cfg.backoff = Duration::from_millis(10);
        let h = spawn(cfg);

        let (first, second) = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("сервер должен дождаться переподключения")
            .unwrap();
        for reg in [first, second] {
            let v: serde_json::Value = serde_json::from_str(&reg).unwrap();
            assert_eq!(v["event"], "registerPlugin");
            assert_eq!(v["uuid"], "U");
        }

        tokio::time::timeout(Duration::from_secs(2), h.task)
            .await
            .expect("task должна завершиться после Close второго соединения")
            .unwrap();
    }

    #[tokio::test]
    async fn gives_up_after_max_retries_without_server() {
        let port = free_port();
        let mut cfg = HostConfig::new(port, "U", "registerPlugin");
        cfg.backoff = Duration::from_millis(10);
        let h = spawn(cfg);
        tokio::time::timeout(Duration::from_secs(5), h.task)
            .await
            .expect("должен сдаться после 3 попыток")
            .unwrap();
    }
}
