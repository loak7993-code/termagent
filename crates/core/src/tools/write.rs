use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::Result;
use crate::safety;
use crate::tools::{string_arg, Tool, ToolCtx};
use crate::types::Risk;

pub struct Write;

impl Tool for Write {
    fn name(&self) -> &'static str {
        "write"
    }

    fn description(&self) -> &'static str {
        "Create or overwrite a file inside the workspace. Creates parent directories."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path relative to the workspace."},
                "content": {"type": "string", "description": "Full file content."}
            },
            "required": ["path", "content"]
        })
    }

    fn risk(&self, _args: &Value) -> Risk {
        Risk::Medium
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let raw = string_arg(args, "path")?;
            let content = string_arg(args, "content")?;
            let path = safety::resolve_in_root(&ctx.root, &ctx.cwd, &raw)?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&path, content.as_bytes()).await?;
            Ok(format!(
                "wrote {} bytes to {}",
                content.len(),
                path.strip_prefix(&ctx.root).unwrap_or(&path).display()
            ))
        })
    }
}
