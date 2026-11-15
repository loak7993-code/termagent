use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::registry::{Protocol, Provider};
use crate::types::{Delta, Effort, Message, ToolCall, Usage};

pub mod anthropic;
pub mod gemini;
pub mod openai;
mod sse;

#[derive(Clone, Debug)]
pub struct ProviderSpec {
    pub protocol: Protocol,
    pub base_url: String,
    pub api_key: String,
    pub project: Option<String>,
}

impl ProviderSpec {
    pub fn resolve(provider: &Provider, cfg: &Config) -> Result<ProviderSpec> {
        let key = cfg.key_for(provider).ok_or(Error::NotConfigured)?;
        let base_url = cfg
            .base_url
            .clone()
            .filter(|b| !b.trim().is_empty())
            .unwrap_or_else(|| provider.api.clone());
        let project = cfg.project.clone().filter(|value| !value.trim().is_empty());
        Ok(ProviderSpec {
            protocol: provider.protocol,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: key,
            project,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Value,
    pub effort: Effort,
    pub reasoning: bool,
    pub max_tokens: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ChatOutcome {
    pub text: String,
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
}

pub async fn stream(
    spec: &ProviderSpec,
    req: &ChatRequest,
    cancel: &CancellationToken,
    on_delta: &mut (dyn FnMut(Delta) + Send),
) -> Result<ChatOutcome> {
    match spec.protocol {
        Protocol::OpenAi => openai::stream(spec, req, cancel, on_delta).await,
        Protocol::Anthropic => anthropic::stream(spec, req, cancel, on_delta).await,
        Protocol::Gemini => gemini::stream(spec, req, cancel, on_delta).await,
    }
}

/// Error body from a non-2xx response, with provider hints kept readable.
pub(crate) async fn http_error(resp: reqwest::Response) -> Error {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if body.is_empty() {
        return Error::api(status, "empty response body");
    }
    Error::api(status, body)
}

pub(crate) fn tool_defs(tools: &Value) -> Vec<(&str, &str, &Value)> {
    let mut out = Vec::new();
    if let Some(arr) = tools.as_array() {
        for t in arr {
            let f = t.get("function").unwrap_or(t);
            let name = f.get("name").and_then(Value::as_str).unwrap_or("");
            let desc = f.get("description").and_then(Value::as_str).unwrap_or("");
            let params = f.get("parameters").unwrap_or(&Value::Null);
            if !name.is_empty() {
                out.push((name, desc, params));
            }
        }
    }
    out
}

pub(crate) fn args_json(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::Object(Default::default()))
}

pub(crate) fn anthropic_messages(messages: &[Message]) -> (String, Vec<Value>) {
    use serde_json::json;
    let mut system = String::new();
    let mut out: Vec<Value> = Vec::new();

    for m in messages {
        match m.role {
            crate::types::Role::System => {
                if let Some(c) = &m.content {
                    system.push_str(c);
                    system.push('\n');
                }
            }
            crate::types::Role::User => {
                let text = m.content.clone().unwrap_or_default();
                if text.trim().is_empty() {
                    continue;
                }
                push_block(&mut out, "user", json!({"type": "text", "text": text}));
            }
            crate::types::Role::Assistant => {
                let mut blocks = Vec::new();
                if let Some(t) = &m.content {
                    if !t.trim().is_empty() {
                        blocks.push(json!({"type": "text", "text": t}));
                    }
                }
                for tc in &m.tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": args_json(&tc.arguments),
                    }));
                }
                if !blocks.is_empty() {
                    out.push(json!({"role": "assistant", "content": blocks}));
                }
            }
            crate::types::Role::Tool => {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": m.content.clone().unwrap_or_default(),
                });
                push_block(&mut out, "user", block);
            }
        }
    }
    (system, out)
}

fn push_block(out: &mut Vec<Value>, role: &str, block: Value) {
    if let Some(last) = out.last_mut() {
        if last.get("role").and_then(Value::as_str) == Some(role) {
            if let Some(arr) = last.get_mut("content").and_then(Value::as_array_mut) {
                arr.push(block);
                return;
            }
        }
    }
    out.push(serde_json::json!({"role": role, "content": [block]}));
}

pub(crate) fn gemini_contents(messages: &[Message]) -> (String, Vec<Value>) {
    use serde_json::json;
    let mut system = String::new();
    let mut out: Vec<Value> = Vec::new();

    for m in messages {
        match m.role {
            crate::types::Role::System => {
                if let Some(c) = &m.content {
                    system.push_str(c);
                    system.push('\n');
                }
            }
            crate::types::Role::User => {
                let text = m.content.clone().unwrap_or_default();
                if text.trim().is_empty() {
                    continue;
                }
                push_part(&mut out, "user", json!({"text": text}));
            }
            crate::types::Role::Assistant => {
                if let Some(t) = &m.content {
                    if !t.trim().is_empty() {
                        push_part(&mut out, "model", json!({"text": t}));
                    }
                }
                for tc in &m.tool_calls {
                    push_part(
                        &mut out,
                        "model",
                        json!({"functionCall": {"name": tc.name, "args": args_json(&tc.arguments)}}),
                    );
                }
            }
            crate::types::Role::Tool => {
                let name = m.name.clone().unwrap_or_default();
                push_part(
                    &mut out,
                    "user",
                    json!({"functionResponse": {"name": name, "response": {"result": m.content.clone().unwrap_or_default()}}}),
                );
            }
        }
    }
    (system, out)
}

fn push_part(out: &mut Vec<Value>, role: &str, part: Value) {
    if let Some(last) = out.last_mut() {
        if last.get("role").and_then(Value::as_str) == Some(role) {
            if let Some(arr) = last.get_mut("parts").and_then(Value::as_array_mut) {
                arr.push(part);
                return;
            }
        }
    }
    out.push(serde_json::json!({"role": role, "parts": [part]}));
}
