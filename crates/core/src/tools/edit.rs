use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::safety;
use crate::tools::{bool_arg, string_arg, Tool, ToolCtx};
use crate::types::Risk;

pub struct Edit;

impl Tool for Edit {
    fn name(&self) -> &'static str {
        "edit"
    }

    fn description(&self) -> &'static str {
        "Replace an exact string in a file. The old string must appear exactly once unless replace_all is true."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "old_string": {"type": "string", "description": "Exact text to replace, including indentation."},
                "new_string": {"type": "string", "description": "Replacement text."},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence."}
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn risk(&self, _args: &Value) -> Risk {
        Risk::Medium
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let raw = string_arg(args, "path")?;
            let old = string_arg(args, "old_string")?;
            let new = string_arg(args, "new_string")?;
            let replace_all = bool_arg(args, "replace_all");
            if old.is_empty() {
                return Err(Error::Tool("old_string is empty".into()));
            }
            if old == new {
                return Err(Error::Tool(
                    "old_string and new_string are identical".into(),
                ));
            }
            let path = safety::resolve_in_root(&ctx.root, &ctx.cwd, &raw)?;
            let text = tokio::fs::read_to_string(&path).await?;
            let count = text.matches(&old).count();
            if count == 0 {
                return Err(Error::Tool(format!(
                    "{raw} does not contain that text; read the file and copy it exactly"
                )));
            }
            if count > 1 && !replace_all {
                return Err(Error::Tool(format!(
                    "old_string appears {count} times in {raw}; add context or set replace_all"
                )));
            }
            let updated = if replace_all {
                text.replace(&old, &new)
            } else {
                text.replacen(&old, &new, 1)
            };
            tokio::fs::write(&path, updated.as_bytes()).await?;
            let replaced = if replace_all { count } else { 1 };
            Ok(format!(
                "edited {raw}: {replaced} replacement{}, {} lines now",
                if replaced == 1 { "" } else { "s" },
                updated.lines().count()
            ))
        })
    }
}
