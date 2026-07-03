use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, Notify};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;
use ym_model::{MediaController, VolumeAction};
use ym_model::time::epoch_secs;
use ym_model::{ActionResult, MediaState, StateEvent};

use crate::discovery;
use crate::{LocalCommand, INJECTED_API_JS, JS_CONTROLLER_NAME};

const TICK: Duration = Duration::from_secs(2);
const RPC_TIMEOUT: Duration = Duration::from_secs(5);
const OPT_WINDOW: Duration = Duration::from_secs(2);

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("нет соединения")]
    Disconnected,
    #[error("таймаут RPC")]
    Timeout,
    #[error("ошибка CDP: {0}")]
    Cdp(String),
}

struct RpcMsg {
    method: &'static str,
    params: Value,
    reply: oneshot::Sender<Result<Value, CdpError>>,
}

#[derive(Clone)]
struct RpcHandle {
    tx: mpsc::Sender<RpcMsg>,
}

impl RpcHandle {
    async fn call(&self, method: &'static str, params: Value) -> Result<Value, CdpError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(RpcMsg { method, params, reply: tx })
            .await
            .map_err(|_| CdpError::Disconnected)?;
        match tokio::time::timeout(RPC_TIMEOUT, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(CdpError::Disconnected),
            Err(_) => Err(CdpError::Timeout),
        }
    }
    async fn eval(&self, expr: &str) -> Result<Value, CdpError> {
        let v = self
            .call(
                "Runtime.evaluate",
                serde_json::json!({"expression": expr, "awaitPromise": true, "returnByValue": true}),
            )
            .await?;
        Ok(v.get("result").and_then(|r| r.get("value")).cloned().unwrap_or(Value::Null))
    }
}

#[derive(Default)]
struct Opt {
    playback_until: Option<Instant>,
    volume_until: Option<Instant>,
}

pub struct CdpController {
    port: AtomicU16,
    state: RwLock<MediaState>,
    connected: AtomicBool,
    bus: broadcast::Sender<StateEvent>,
    rpc: RwLock<Option<RpcHandle>>,
    opt: StdMutex<Opt>,
    reconnect: Notify,
    download_tx: RwLock<Option<mpsc::Sender<String>>>,
}

impl CdpController {
    pub fn new(port: u16, bus: broadcast::Sender<StateEvent>) -> Arc<Self> {
        Arc::new(Self {
            port: AtomicU16::new(port),
            state: RwLock::new(MediaState::default()),
            connected: AtomicBool::new(false),
            bus,
            rpc: RwLock::new(None),
            opt: StdMutex::new(Opt::default()),
            reconnect: Notify::new(),
            download_tx: RwLock::new(None),
        })
    }

    pub fn set_download_tx(&self, tx: mpsc::Sender<String>) {
        *self.download_tx.write().unwrap() = Some(tx);
    }

    pub fn local_port(&self) -> u16 {
        self.port.load(Ordering::Acquire)
    }

    pub fn start(self: &Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(connection_loop(self.clone(), shutdown))
    }

    fn set_connected(&self, b: bool) {
        if self.connected.swap(b, Ordering::AcqRel) != b {
            let _ = self.bus.send(StateEvent::Connection(b));
        }
    }

    async fn ensure_injection(&self, rpc: &RpcHandle) -> Result<(), CdpError> {
        let present = rpc.eval(&format!("!!({JS_CONTROLLER_NAME})")).await?;
        if present.as_bool() != Some(true) {
            rpc.eval(INJECTED_API_JS).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    async fn run_cmd(&self, cmd: LocalCommand) -> ActionResult {
        self.run_js(&cmd.expr(), cmd.kind()).await
    }

    async fn run_js(&self, expr: &str, kind: &str) -> ActionResult {
        let handle = self.rpc.read().unwrap().clone();
        let Some(rpc) = handle else {
            return ActionResult::default();
        };
        if let Err(e) = self.ensure_injection(&rpc).await {
            tracing::warn!("cdp: инъекция перед '{kind}' не удалась: {e}");
            return ActionResult { success: false, error: Some("Injection failed".into()), ..Default::default() };
        }
        match rpc.eval(expr).await {
            Ok(v) if v.is_object() => {
                let r = ActionResult::from_value(&v);
                self.apply_optimistic(kind, &r);
                r
            }
            Ok(_) => ActionResult { success: false, error: Some("Invalid response".into()), ..Default::default() },
            Err(e) => {
                tracing::warn!("cdp: ошибка выполнения '{kind}': {e}");
                ActionResult { success: false, error: Some("RPC error".into()), ..Default::default() }
            }
        }
    }

    fn apply_optimistic(&self, kind: &str, r: &ActionResult) {
        if !r.success {
            return;
        }
        let mut events = Vec::new();
        let mut set_pb = false;
        let mut set_vol = false;
        {
            let mut st = self.state.write().unwrap();
            match kind {
                "playPause" => {
                    if let Some(p) = r.is_playing {
                        st.playback.is_playing = p;
                        st.playback.timestamp = epoch_secs();
                        set_pb = true;
                        events.push(StateEvent::Playback(st.playback.clone()));
                    }
                }
                "toggleLike" => {
                    if let Some(v) = r.new_state {
                        st.like.is_liked = v;
                        events.push(StateEvent::Like(st.like.clone()));
                    }
                }
                "toggleDislike" => {
                    if let Some(v) = r.new_state {
                        st.dislike.is_disliked = v;
                        events.push(StateEvent::Dislike(st.dislike.clone()));
                    }
                }
                "changeVolume" => {
                    let mut changed = false;
                    if let Some(v) = r.volume {
                        st.volume.current = v;
                        changed = true;
                    }
                    if let Some(m) = r.is_muted {
                        st.volume.is_muted = m;
                        changed = true;
                    }
                    if changed {
                        set_vol = true;
                        events.push(StateEvent::Volume(st.volume.clone()));
                    }
                }
                _ => {}
            }
        }
        if set_pb {
            self.opt.lock().unwrap().playback_until = Some(Instant::now() + OPT_WINDOW);
        }
        if set_vol {
            self.opt.lock().unwrap().volume_until = Some(Instant::now() + OPT_WINDOW);
        }
        for e in events {
            let _ = self.bus.send(e);
        }
    }

    fn set_full(&self, new_state: MediaState) {
        let events = {
            let mut st = self.state.write().unwrap();
            *st = new_state;
            vec![
                StateEvent::Connection(true),
                StateEvent::Track(st.track.clone()),
                StateEvent::Playback(st.playback.clone()),
                StateEvent::Volume(st.volume.clone()),
                StateEvent::Like(st.like.clone()),
                StateEvent::Dislike(st.dislike.clone()),
            ]
        };
        for e in events {
            let _ = self.bus.send(e);
        }
    }

    fn apply_delta(&self, payload: &Value) {
        let suppress_pb = self.in_window(|o| o.playback_until);
        let suppress_vol = self.in_window(|o| o.volume_until);
        let mut events = Vec::new();
        {
            let mut st = self.state.write().unwrap();
            if let Some(track) = payload.get("track") {
                let mut changed = false;
                if let Some(id) = track.get("id") {
                    st.track.track_id = id_to_string(id);
                    changed = true;
                }
                if let Some(t) = track.get("title").and_then(Value::as_str) {
                    st.track.title = t.to_owned();
                    changed = true;
                }
                if let Some(a) = track.get("artist").and_then(Value::as_str) {
                    st.track.artist = a.to_owned();
                    changed = true;
                }
                if let Some(c) = track.get("cover").and_then(Value::as_str) {
                    st.track.cover_url = c.to_owned();
                    changed = true;
                }
                if changed {
                    events.push(StateEvent::Track(st.track.clone()));
                }
            }
            if let Some(s) = payload.get("state") {
                if let Some(p) = s.get("playing").and_then(Value::as_bool) {
                    st.playback.is_playing = p;
                    events.push(StateEvent::Playback(st.playback.clone()));
                }
                if let Some(l) = s.get("liked").and_then(Value::as_bool) {
                    st.like.is_liked = l;
                    events.push(StateEvent::Like(st.like.clone()));
                }
                if let Some(d) = s.get("disliked").and_then(Value::as_bool) {
                    st.dislike.is_disliked = d;
                    events.push(StateEvent::Dislike(st.dislike.clone()));
                }
            }
            if let Some(pr) = payload.get("progress") {
                let mut changed = false;
                if let Some(n) = pr.get("now_sec").and_then(Value::as_f64) {
                    st.playback.current_sec = n;
                    changed = true;
                }
                if let Some(t) = pr.get("total_sec").and_then(Value::as_f64) {
                    st.playback.total_sec = t;
                    changed = true;
                }
                if let Some(rt) = pr.get("ratio").and_then(Value::as_f64) {
                    st.playback.progress = rt;
                    changed = true;
                }
                if changed {
                    st.playback.timestamp = epoch_secs();
                    if !suppress_pb {
                        events.push(StateEvent::Playback(st.playback.clone()));
                    }
                }
            }
            if let Some(v) = payload.get("volume") {
                let mut changed = false;
                if let Some(c) = v.get("current").and_then(Value::as_f64) {
                    st.volume.current = c;
                    changed = true;
                }
                if let Some(m) = v.get("is_muted").and_then(Value::as_bool) {
                    st.volume.is_muted = m;
                    changed = true;
                }
                if changed && !suppress_vol {
                    events.push(StateEvent::Volume(st.volume.clone()));
                }
            }
        }
        for e in events {
            let _ = self.bus.send(e);
        }
    }

    fn in_window(&self, pick: impl Fn(&Opt) -> Option<Instant>) -> bool {
        pick(&self.opt.lock().unwrap()).is_some_and(|t| Instant::now() <= t)
    }
}

#[async_trait]
impl MediaController for CdpController {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    fn snapshot(&self) -> MediaState {
        self.state.read().unwrap().clone()
    }
    fn set_local_port(&self, port: u16) {
        if self.port.swap(port, Ordering::AcqRel) != port {
            self.reconnect.notify_one();
        }
    }
    async fn oauth_token(&self) -> Option<String> {
        let handle = self.rpc.read().unwrap().clone();
        let rpc = handle?;
        let expr = "(()=>{try{return JSON.parse(localStorage.getItem('oauth')).value}catch(e){return null}})()";
        match rpc.eval(expr).await {
            Ok(Value::String(s)) if !s.is_empty() => Some(s),
            _ => None,
        }
    }
    async fn play_pause(&self) -> ActionResult {
        self.run_cmd(LocalCommand::PlayPause).await
    }
    async fn next(&self) -> ActionResult {
        self.run_cmd(LocalCommand::Next).await
    }
    async fn prev(&self) -> ActionResult {
        self.run_cmd(LocalCommand::Prev).await
    }
    async fn toggle_like(&self) -> ActionResult {
        self.run_cmd(LocalCommand::ToggleLike).await
    }
    async fn toggle_dislike(&self) -> ActionResult {
        self.run_cmd(LocalCommand::ToggleDislike).await
    }
    async fn change_volume(&self, action: VolumeAction) -> ActionResult {
        self.run_cmd(LocalCommand::ChangeVolume(action)).await
    }
}

fn id_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

type Pending = Arc<StdMutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>>;

async fn writer_loop(
    mut write: SplitSink<Ws, Message>,
    mut rpc_rx: mpsc::Receiver<RpcMsg>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
) {
    while let Some(msg) = rpc_rx.recv().await {
        let id = next_id.fetch_add(1, Ordering::Relaxed);
        pending.lock().unwrap().insert(id, msg.reply);
        let frame = serde_json::json!({"id": id, "method": msg.method, "params": msg.params}).to_string();
        if write.send(Message::Text(frame)).await.is_err() {
            break;
        }
    }
}

async fn reader_loop(mut read: SplitStream<Ws>, pending: Pending, ctrl: Arc<CdpController>) {
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(t)) => {
                let v = match serde_json::from_str::<Value>(&t) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!("cdp: битый JSON от CDP: {e}");
                        continue;
                    }
                };
                if let Some(id) = v.get("id").and_then(Value::as_u64) {
                    if let Some(reply) = pending.lock().unwrap().remove(&id) {
                        if let Some(err) = v.get("error") {
                            let _ = reply.send(Err(CdpError::Cdp(err.to_string())));
                        } else {
                            let _ = reply.send(Ok(v.get("result").cloned().unwrap_or(Value::Null)));
                        }
                    }
                } else if v.get("method").and_then(Value::as_str) == Some("Runtime.bindingCalled") {
                    handle_binding(&v, &ctrl);
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
    for (_, reply) in pending.lock().unwrap().drain() {
        let _ = reply.send(Err(CdpError::Disconnected));
    }
}

fn handle_binding(v: &Value, ctrl: &CdpController) {
    let params = v.get("params");
    if params.and_then(|p| p.get("name")).and_then(Value::as_str) != Some("sdNotify") {
        return;
    }
    let payload_str = params.and_then(|p| p.get("payload")).and_then(Value::as_str).unwrap_or("{}");
    let data = match serde_json::from_str::<Value>(payload_str) {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("cdp: битый payload sdNotify: {e}");
            return;
        }
    };
    let typ = data.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = data.get("payload").cloned().unwrap_or(Value::Null);
    match typ {
        "FULL_STATE" => ctrl.set_full(MediaState::from_full_value(&payload)),
        "DELTA" => ctrl.apply_delta(&payload),
        "DOWNLOAD" => {
            if let Some(id) = payload.get("track_id").and_then(Value::as_str)
                && !id.is_empty()
                && let Some(tx) = ctrl.download_tx.read().unwrap().clone()
            {
                let _ = tx.try_send(id.to_owned());
            }
        }
        _ => {}
    }
}

async fn sleep_or_cancel(shutdown: &CancellationToken, dur: Duration) -> bool {
    tokio::select! {
        _ = shutdown.cancelled() => true,
        _ = tokio::time::sleep(dur) => false,
    }
}

async fn connection_loop(ctrl: Arc<CdpController>, shutdown: CancellationToken) {
    'outer: loop {
        if shutdown.is_cancelled() {
            break;
        }
        let port = ctrl.port.load(Ordering::Acquire);
        let Some(url) = discovery::find_ws_url(port).await else {
            if sleep_or_cancel(&shutdown, TICK).await {
                break;
            }
            continue;
        };
        let ws = match tokio_tungstenite::connect_async(&url).await {
            Ok((ws, _)) => ws,
            Err(e) => {
                tracing::warn!("cdp: подключение к {url} не удалось: {e}");
                if sleep_or_cancel(&shutdown, TICK).await {
                    break;
                }
                continue;
            }
        };
        let (write, read) = ws.split();
        let (rpc_tx, rpc_rx) = mpsc::channel::<RpcMsg>(64);
        let pending: Pending = Arc::new(StdMutex::new(HashMap::new()));
        let next_id = Arc::new(AtomicU64::new(1));
        let writer = tokio::spawn(writer_loop(write, rpc_rx, pending.clone(), next_id.clone()));
        let mut reader = tokio::spawn(reader_loop(read, pending.clone(), ctrl.clone()));

        let handle = RpcHandle { tx: rpc_tx };
        *ctrl.rpc.write().unwrap() = Some(handle.clone());
        ctrl.set_connected(true);

        if let Err(e) = handle.call("Runtime.enable", serde_json::json!({})).await {
            tracing::warn!("cdp: Runtime.enable не удался: {e}");
        }
        if let Err(e) = handle.call("Runtime.addBinding", serde_json::json!({"name": "sdNotify"})).await {
            tracing::warn!("cdp: addBinding(sdNotify) не удался: {e}");
        }
        if let Err(e) = ctrl.ensure_injection(&handle).await {
            tracing::warn!("cdp: первичная инъекция не удалась: {e}");
        }
        let _ = handle.eval(&format!("{JS_CONTROLLER_NAME}.stopObservation()")).await;
        let _ = handle.eval(&format!("{JS_CONTROLLER_NAME}.startObservation()")).await;

        let mut cancelled = false;
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    cancelled = true;
                    break;
                }
                _ = &mut reader => break,
                _ = ctrl.reconnect.notified() => break,
                _ = tokio::time::sleep(TICK) => {
                    let _ = ctrl.ensure_injection(&handle).await;
                }
            }
        }

        *ctrl.rpc.write().unwrap() = None;
        ctrl.set_connected(false);
        writer.abort();
        reader.abort();
        drop(handle);
        if cancelled || sleep_or_cancel(&shutdown, TICK).await {
            break 'outer;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    fn full_state_json() -> Value {
        serde_json::json!({
            "track": {"id": "777", "title": "Mock Track", "artist": "Mock Artist", "cover": "https://x/400x400"},
            "state": {"playing": false, "liked": false, "disliked": false},
            "progress": {"now_sec": 10.0, "total_sec": 200.0, "ratio": 0.05},
            "volume": {"current": 40.0, "is_muted": false}
        })
    }

    fn eval_reply(id: u64, value: Value) -> String {
        serde_json::json!({"id": id, "result": {"result": {"value": value}}}).to_string()
    }
    fn ok_reply(id: u64) -> String {
        serde_json::json!({"id": id, "result": {}}).to_string()
    }
    fn binding_frame(typ: &str, payload: Value) -> String {
        let inner = serde_json::json!({"type": typ, "payload": payload}).to_string();
        serde_json::json!({"method": "Runtime.bindingCalled", "params": {"name": "sdNotify", "payload": inner}}).to_string()
    }

    async fn start_mock() -> u16 {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ws_port = ws_listener.local_addr().unwrap().port();
        let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http_port = http_listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = http_listener.accept().await else { break };
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf).await;
                let body = format!(
                    r#"[{{"type":"page","url":"music-application://desktop/","title":"Яндекс Музыка","webSocketDebuggerUrl":"ws://127.0.0.1:{ws_port}/ws"}}]"#
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.flush().await;
            }
        });

        tokio::spawn(async move {
            let Ok((sock, _)) = ws_listener.accept().await else { return };
            let mut ws = accept_async(sock).await.unwrap();
            while let Some(Ok(msg)) = ws.next().await {
                let Message::Text(t) = msg else { continue };
                let v: Value = serde_json::from_str(&t).unwrap();
                let id = v["id"].as_u64().unwrap_or(0);
                let method = v["method"].as_str().unwrap_or("");
                if method == "Runtime.evaluate" {
                    let expr = v["params"]["expression"].as_str().unwrap_or("");
                    let value = if expr.contains("!!(") || expr.contains("startObservation") {
                        Value::Bool(true)
                    } else if expr.contains("getFullState") {
                        serde_json::json!({"success": true, "data": full_state_json()})
                    } else if expr.contains("playPause") {
                        serde_json::json!({"success": true, "is_playing": true})
                    } else if expr.contains("toggleLike") {
                        serde_json::json!({"success": true, "new_state": true})
                    } else if expr.contains("changeVolume") {
                        serde_json::json!({"success": true, "volume": 55, "is_muted": false})
                    } else {
                        Value::Null
                    };
                    ws.send(Message::Text(eval_reply(id, value))).await.unwrap();
                    if expr.contains("startObservation") {
                        ws.send(Message::Text(binding_frame("FULL_STATE", full_state_json()))).await.unwrap();
                        ws.send(Message::Text(binding_frame("DELTA", serde_json::json!({"state": {"liked": true}}))))
                            .await
                            .unwrap();
                    }
                } else {
                    ws.send(Message::Text(ok_reply(id))).await.unwrap();
                }
            }
        });

        http_port
    }

    async fn wait_connected(ctrl: &Arc<CdpController>) {
        for _ in 0..600 {
            if ctrl.is_connected() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("контроллер не подключился");
    }

    async fn wait_until(mut cond: impl FnMut() -> bool) -> bool {
        for _ in 0..600 {
            if cond() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connects_fetches_full_state_and_applies_pushed_delta() {
        let http_port = start_mock().await;
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(http_port, bus);
        let _task = ctrl.start(CancellationToken::new());

        wait_connected(&ctrl).await;
        let ctrl_c = ctrl.clone();
        assert!(wait_until(move || ctrl_c.snapshot().track.title == "Mock Track").await);
        assert_eq!(ctrl.snapshot().track.title, "Mock Track");
        assert_eq!(ctrl.snapshot().track.track_id, "777");
        assert_eq!(ctrl.snapshot().volume.current, 40.0, "громкость должна прийти из FULL_STATE");

        let ctrl_c = ctrl.clone();
        assert!(wait_until(move || ctrl_c.snapshot().like.is_liked).await, "DELTA лайка должна примениться");
    }

    #[tokio::test]
    async fn binding_download_forwards_track_id_to_channel() {
        let (bus, _rx) = broadcast::channel(8);
        let ctrl = CdpController::new(0, bus);
        let (tx, mut dl_rx) = mpsc::channel(8);
        ctrl.set_download_tx(tx);

        let msg = serde_json::json!({
            "method": "Runtime.bindingCalled",
            "params": { "name": "sdNotify", "payload": "{\"type\":\"DOWNLOAD\",\"payload\":{\"track_id\":\"12345\"}}" }
        });
        handle_binding(&msg, &ctrl);
        assert_eq!(dl_rx.try_recv().unwrap(), "12345");

        let empty = serde_json::json!({
            "method": "Runtime.bindingCalled",
            "params": { "name": "sdNotify", "payload": "{\"type\":\"DOWNLOAD\",\"payload\":{\"track_id\":\"\"}}" }
        });
        handle_binding(&empty, &ctrl);
        assert!(dl_rx.try_recv().is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn play_pause_command_returns_result_and_applies_optimistic() {
        let http_port = start_mock().await;
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(http_port, bus);
        let _task = ctrl.start(CancellationToken::new());
        wait_connected(&ctrl).await;

        let r = ctrl.play_pause().await;
        assert!(r.success);
        assert_eq!(r.is_playing, Some(true));
        assert!(ctrl.snapshot().playback.is_playing);
    }

    #[tokio::test]
    async fn disconnected_controller_returns_default_result() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(1, bus);
        let r = ctrl.play_pause().await;
        assert!(!r.success);
        assert!(!ctrl.is_connected());
    }

    #[tokio::test(start_paused = true)]
    async fn connection_loop_exits_on_cancel() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(1, bus);
        let shutdown = CancellationToken::new();
        let task = ctrl.start(shutdown.clone());
        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("connection_loop должен завершиться по cancel")
            .unwrap();
    }

    #[test]
    fn delta_suppression_respects_optimistic_window() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(9222, bus);
        ctrl.apply_optimistic("changeVolume", &ActionResult { success: true, volume: Some(50.0), ..Default::default() });
        let mut sub = ctrl.bus.subscribe();
        ctrl.apply_delta(&serde_json::json!({"volume": {"current": 99.0}}));
        assert!(sub.try_recv().is_err(), "volume-дельта должна быть подавлена в окне");
        assert_eq!(ctrl.snapshot().volume.current, 99.0);
    }
}
