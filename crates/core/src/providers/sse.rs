use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};

/// Minimal SSE reader for provider streams: yields each `data:` payload.
pub async fn read_sse<F>(
    resp: reqwest::Response,
    cancel: &CancellationToken,
    mut on_data: F,
) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    use futures_util::StreamExt;

    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let chunk = chunk.map_err(|e| Error::Http(format!("stream: {e}")))?;
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(payload) = line.strip_prefix("data:") {
                let payload = payload.trim_start();
                if payload == "[DONE]" {
                    return Ok(());
                }
                if !payload.is_empty() {
                    on_data(payload)?;
                }
            }
        }
    }
    Ok(())
}
