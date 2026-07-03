use std::time::Duration;
use serde_json::Value;

pub async fn find_ws_url(port: u16) -> Option<String> {
    let url = format!("http://127.0.0.1:{port}/json/list");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let pages: Value = resp.json().await.ok()?;
    let arr = pages.as_array()?;
    let pick = arr.iter().find(|p| is_ym_page(p)).or_else(|| arr.first())?;
    pick.get("webSocketDebuggerUrl").and_then(Value::as_str).map(str::to_owned)
}

fn is_ym_page(p: &Value) -> bool {
    let url = p.get("url").and_then(Value::as_str).unwrap_or("");
    let title = p.get("title").and_then(Value::as_str).unwrap_or("");
    url.contains("music.yandex") || title.contains("Music") || title.contains("Музыка")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn picks_yandex_music_page() {
        let body = r#"[{"type":"page","url":"about:blank","title":"x","webSocketDebuggerUrl":"ws://nope"},{"type":"page","url":"music-application://desktop/","title":"Яндекс Музыка","webSocketDebuggerUrl":"ws://127.0.0.1:1/ws"}]"#;
        let port = serve_http_once(body).await;
        assert_eq!(find_ws_url(port).await.as_deref(), Some("ws://127.0.0.1:1/ws"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn falls_back_to_first_page() {
        let body = r#"[{"type":"page","url":"about:blank","title":"x","webSocketDebuggerUrl":"ws://first"}]"#;
        let port = serve_http_once(body).await;
        assert_eq!(find_ws_url(port).await.as_deref(), Some("ws://first"));
    }

    #[tokio::test]
    async fn no_server_is_none() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        assert!(find_ws_url(p).await.is_none());
    }
}
