use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::safety;
use crate::tools::{string_arg, Tool, ToolCtx};

pub struct Read;

const MAX_LINES: usize = 2000;
const MAX_BYTES: usize = 4 * 1024 * 1024;

impl Tool for Read {
    fn name(&self) -> &'static str {
        "read"
    }

    fn description(&self) -> &'static str {
        "Read a text file inside the workspace. Returns numbered lines. Use offset/limit for large files."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path relative to the workspace."},
                "offset": {"type": "integer", "description": "First line to return, 1-based."},
                "limit": {"type": "integer", "description": "Maximum lines, capped at 2000."}
            },
            "required": ["path"]
        })
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let raw = string_arg(args, "path")?;
            let path = safety::resolve_in_root(&ctx.root, &ctx.cwd, &raw)?;
            let meta = tokio::fs::metadata(&path).await?;
            if meta.is_dir() {
                return Err(Error::Tool(format!("{raw} is a directory; use ls")));
            }
            if meta.len() as usize > MAX_BYTES {
                return Err(Error::Tool(format!(
                    "{raw} is {} MB; read it in slices with offset/limit",
                    meta.len() / 1024 / 1024
                )));
            }
            let bytes = tokio::fs::read(&path).await?;
            if bytes.iter().take(8000).any(|b| *b == 0) {
                return Err(Error::Tool(format!("{raw} looks binary")));
            }
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = text.lines().collect();
            let offset = args
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1) as usize;
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_LINES as u64)
                .min(MAX_LINES as u64) as usize;
            if lines.is_empty() {
                return Ok(format!("{raw} is empty"));
            }
            if offset > lines.len() {
                return Err(Error::Tool(format!(
                    "offset {offset} is past the end ({} lines)",
                    lines.len()
                )));
            }
            let start = offset - 1;
            let end = (start + limit).min(lines.len());
            let mut out = String::new();
            for (i, line) in lines[start..end].iter().enumerate() {
                out.push_str(&format!("{:>6}\t{}\n", start + i + 1, line));
            }
            if end < lines.len() {
                out.push_str(&format!("… {} more lines\n", lines.len() - end));
            }
            Ok(out)
        })
    }
}
