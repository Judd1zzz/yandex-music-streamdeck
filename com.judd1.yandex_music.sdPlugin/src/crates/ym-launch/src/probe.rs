use std::time::Duration;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortStatus {
    YmAlive,
    Foreign,
    Dead,
}

pub async fn probe(port: u16) -> PortStatus {
    let url = format!("http://127.0.0.1:{port}/json/version");
    let Ok(client) = reqwest::Client::builder().timeout(Duration::from_secs(2)).build() else {
        return PortStatus::Dead;
    };
    match client.get(&url).send().await {
        Ok(resp) => match resp.text().await {
            Ok(body) => classify(&body),
            Err(_) => PortStatus::Foreign,
        },
        Err(_) => PortStatus::Dead,
    }
}

pub fn classify(body: &str) -> PortStatus {
    let is_ym = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("User-Agent").and_then(Value::as_str).map(|ua| ua.contains("YandexMusic/")))
        .unwrap_or(false);
    if is_ym { PortStatus::YmAlive } else { PortStatus::Foreign }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const YM_VERSION_BODY: &str = r#"{
        "Browser": "Chrome/140.0.7339.133",
        "Protocol-Version": "1.3",
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) YandexMusic/5.108.3 Chrome/140.0.7339.133 Electron/38.2.2 Safari/537.36",
        "V8-Version": "14.0.365.4",
        "WebKit-Version": "537.36",
        "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/053cf0ad"
    }"#;

    const CHROME_VERSION_BODY: &str = r#"{
        "Browser": "Chrome/126.0.6478.127",
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    }"#;

    async fn serve_http_once(body: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.flush().await;
            }
        });
        port
    }

    #[test]
    fn classify_real_ym_fixture() {
        assert_eq!(classify(YM_VERSION_BODY), PortStatus::YmAlive);
    }

    #[test]
    fn classify_foreign_bodies() {
        assert_eq!(classify(CHROME_VERSION_BODY), PortStatus::Foreign);
        assert_eq!(classify("<html>hi</html>"), PortStatus::Foreign);
        assert_eq!(classify(""), PortStatus::Foreign);
        assert_eq!(classify(r#"{"Browser": "Chrome/1"}"#), PortStatus::Foreign);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn probe_ym_server() {
        let port = serve_http_once(YM_VERSION_BODY).await;
        assert_eq!(probe(port).await, PortStatus::YmAlive);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn probe_foreign_server() {
        let port = serve_http_once(CHROME_VERSION_BODY).await;
        assert_eq!(probe(port).await, PortStatus::Foreign);
    }

    #[tokio::test]
    async fn probe_closed_port_is_dead() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        assert_eq!(probe(p).await, PortStatus::Dead);
    }
}
