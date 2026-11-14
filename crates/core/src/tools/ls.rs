use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::Result;
use crate::safety;
use crate::tools::{Tool, ToolCtx};

pub struct Ls;

impl Tool for Ls {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn description(&self) -> &'static str {
        "List a directory inside the workspace. Directories first, with sizes."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Directory, default workspace root."}
            }
        })
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let path = safety::resolve_in_root(&ctx.root, &ctx.cwd, raw)?;
            let mut entries = Vec::new();
            let mut rd = tokio::fs::read_dir(&path).await?;
            while let Some(entry) = rd.next_entry().await? {
                let meta = entry.metadata().await.ok();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = meta.map(|m| m.len()).unwrap_or(0);
                let name = entry.file_name().to_string_lossy().to_string();
                entries.push((is_dir, name, size));
            }
            entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            if entries.is_empty() {
                return Ok(format!("{} is empty", raw));
            }
            let mut out = String::new();
            for (is_dir, name, size) in entries.iter().take(500) {
                if *is_dir {
                    out.push_str(&format!("{name}/\n"));
                } else {
                    out.push_str(&format!("{name}  {} B\n", size));
                }
            }
            if entries.len() > 500 {
                out.push_str(&format!("… {} more entries\n", entries.len() - 500));
            }
            Ok(out)
        })
    }
}
