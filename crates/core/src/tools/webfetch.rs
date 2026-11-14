use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::safety;
use crate::tools::{string_arg, Tool, ToolCtx};
use crate::types::Risk;

pub struct WebFetch;

impl Tool for WebFetch {
    fn name(&self) -> &'static str {
        "webfetch"
    }

    fn description(&self) -> &'static str {
        "Fetch a URL over http(s) and return readable text. HTML is reduced to text."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {"type": "string"},
                "max_chars": {"type": "integer", "description": "Default 20000, capped at 60000."}
            },
            "required": ["url"]
        })
    }

    fn risk(&self, _args: &Value) -> Risk {
        Risk::Medium
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let url = string_arg(args, "url")?;
            let parsed =
                reqwest::Url::parse(&url).map_err(|e| Error::Tool(format!("bad url: {e}")))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(Error::Tool("only http(s) urls are allowed".into()));
            }
            let max = args
                .get("max_chars")
                .and_then(Value::as_u64)
                .unwrap_or(20_000)
                .clamp(500, 60_000) as usize;
            let resp = ctx
                .http
                .get(parsed)
                .send()
                .await
                .map_err(|e| Error::Tool(format!("fetch failed: {e}")))?;
            let status = resp.status();
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let body = resp
                .text()
                .await
                .map_err(|e| Error::Tool(format!("read failed: {e}")))?;
            if !status.is_success() {
                return Err(Error::api(status.as_u16(), body));
            }
            let text = if content_type.contains("html") {
                strip_html(&body)
            } else if content_type.starts_with("image/") || content_type.contains("octet-stream") {
                return Ok(format!(
                    "{url}: binary content ({content_type}), {len} bytes",
                    len = body.len()
                ));
            } else {
                body
            };
            Ok(safety::truncate(&collapse_ws(&text), max))
        })
    }
}

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut skip = false;
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if !in_tag && c == '<' {
            in_tag = true;
            let rest = &lower[i..];
            skip = rest.starts_with("<script")
                || rest.starts_with("<style")
                || rest.starts_with("<noscript");
            i += 1;
            continue;
        }
        if in_tag {
            if c == '>' {
                in_tag = false;
                if skip {
                    skip = false;
                }
                out.push(' ');
            }
            i += 1;
            continue;
        }
        if !skip {
            out.push(c);
        }
        i += 1;
    }
    out
}

fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank = 0;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}
