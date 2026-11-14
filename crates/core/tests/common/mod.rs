#![allow(dead_code)]

use std::sync::OnceLock;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serves one canned HTTP body per accepted connection, in order.
/// Returns the base URL and a handle that counts requests.
pub async fn spawn_server(
    responses: Vec<(u16, String)>,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let idx = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let (status, body) = responses
                .get(idx)
                .cloned()
                .or_else(|| responses.last().cloned())
                .unwrap_or((200, String::new()));
            let mut buf = vec![0u8; 65536];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buf))
                .await;
            let reason = if status == 200 { "OK" } else { "Unauthorized" };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (format!("http://{addr}"), hits)
}

pub fn sse(events: &[&str]) -> String {
    let mut out = String::new();
    for event in events {
        out.push_str("data: ");
        out.push_str(event);
        out.push_str("\n\n");
    }
    out
}

/// One config dir per test binary, so session/config tests do not race.
pub fn test_config_dir() -> &'static std::path::Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("TERMAGENT_CONFIG_DIR", d.path());
        d
    });
    dir.path()
}
