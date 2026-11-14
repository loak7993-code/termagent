use futures_util::future::BoxFuture;
use globset::GlobBuilder;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::safety;
use crate::tools::{string_arg, Tool, ToolCtx};

pub struct Glob;

impl Tool for Glob {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn description(&self) -> &'static str {
        "Find files by glob pattern, newest-aware and ordered. Respects the workspace boundary."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob such as **/*.rs or src/**/*.ts"},
                "path": {"type": "string", "description": "Base directory, default workspace root."}
            },
            "required": ["pattern"]
        })
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let pattern = string_arg(args, "pattern")?;
            let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let base = safety::resolve_in_root(&ctx.root, &ctx.cwd, raw)?;
            let matcher = GlobBuilder::new(&pattern)
                .literal_separator(false)
                .build()
                .map_err(|e| Error::Tool(format!("bad glob: {e}")))?
                .compile_matcher();

            let mut hits = Vec::new();
            for entry in walkdir::WalkDir::new(&base)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    name != ".git" && name != "node_modules" && name != "target"
                })
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                let rel = path.strip_prefix(&ctx.root).unwrap_or(path);
                if matcher.is_match(rel) {
                    hits.push(rel.display().to_string());
                    if hits.len() >= 500 {
                        break;
                    }
                }
            }
            hits.sort();
            if hits.is_empty() {
                Ok(format!("no files match {pattern}"))
            } else {
                Ok(hits.join("\n"))
            }
        })
    }
}
