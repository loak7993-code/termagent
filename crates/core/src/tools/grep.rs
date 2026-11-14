use futures_util::future::BoxFuture;
use globset::GlobBuilder;
use regex::RegexBuilder;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::safety;
use crate::tools::{bool_arg, string_arg, Tool, ToolCtx};

pub struct Grep;

impl Tool for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file contents with a regular expression. Returns path:line: text."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Rust regex syntax."},
                "path": {"type": "string", "description": "Base directory, default workspace root."},
                "glob": {"type": "string", "description": "Only search files matching this glob."},
                "case_sensitive": {"type": "boolean", "description": "Default false."}
            },
            "required": ["pattern"]
        })
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let pattern = string_arg(args, "pattern")?;
            let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let base = safety::resolve_in_root(&ctx.root, &ctx.cwd, raw)?;
            let case_sensitive = bool_arg(args, "case_sensitive");
            let re = RegexBuilder::new(&pattern)
                .case_insensitive(!case_sensitive)
                .build()
                .map_err(|e| Error::Tool(format!("bad regex: {e}")))?;
            let file_filter = match args.get("glob").and_then(Value::as_str) {
                Some(g) => Some(
                    GlobBuilder::new(g)
                        .build()
                        .map_err(|e| Error::Tool(format!("bad glob: {e}")))?
                        .compile_matcher(),
                ),
                None => None,
            };

            let mut out = Vec::new();
            'walk: for entry in walkdir::WalkDir::new(&base)
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
                if let Some(f) = &file_filter {
                    if !f.is_match(rel) {
                        continue;
                    }
                }
                let Ok(meta) = entry.metadata() else { continue };
                if meta.len() > 2 * 1024 * 1024 {
                    continue;
                }
                let Ok(bytes) = std::fs::read(path) else {
                    continue;
                };
                if bytes.iter().take(2000).any(|b| *b == 0) {
                    continue;
                }
                let text = String::from_utf8_lossy(&bytes);
                for (n, line) in text.lines().enumerate() {
                    if re.is_match(line) {
                        out.push(format!(
                            "{}:{}: {}",
                            rel.display(),
                            n + 1,
                            safety::truncate(line.trim(), 300)
                        ));
                        if out.len() >= 200 {
                            break 'walk;
                        }
                    }
                }
            }
            if out.is_empty() {
                Ok(format!("no matches for {pattern}"))
            } else {
                Ok(out.join("\n"))
            }
        })
    }
}
