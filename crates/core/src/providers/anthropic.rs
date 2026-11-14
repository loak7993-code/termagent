use std::collections::BTreeMap;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::providers::{
    anthropic_messages, http_error, sse, tool_defs, ChatOutcome, ChatRequest, ProviderSpec,
};
use crate::types::{Delta, ToolCall};

pub async fn stream(
    spec: &ProviderSpec,
    req: &ChatRequest,
    cancel: &CancellationToken,
    on_delta: &mut (dyn FnMut(Delta) + Send),
) -> Result<ChatOutcome> {
    let body = payload(req);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| Error::Http(e.to_string()))?;
    let fut = client
        .post(format!("{}/messages", spec.base_url))
        .header("x-api-key", &spec.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("accept", "text/event-stream")
        .json(&body)
        .send();
    let resp = tokio::select! {
        _ = cancel.cancelled() => return Err(Error::Cancelled),
        r = fut => r.map_err(|e| Error::Http(format!("request: {e}")))?,
    };
    if !resp.status().is_success() {
        return Err(http_error(resp).await);
    }

    let mut out = ChatOutcome::default();
    let mut calls: BTreeMap<u64, (String, String, String)> = BTreeMap::new();
    let mut failure: Option<Error> = None;

    let res = sse::read_sse(resp, cancel, |data| {
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(());
        };
        match v.get("type").and_then(Value::as_str).unwrap_or("") {
            "message_start" => {
                if let Some(u) = v
                    .pointer("/message/usage/input_tokens")
                    .and_then(Value::as_u64)
                {
                    out.usage.input = u;
                }
            }
            "content_block_start" => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(block) = v.get("content_block") {
                    if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                        let id = block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        calls.insert(index, (id, name, String::new()));
                    }
                }
            }
            "content_block_delta" => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                let Some(delta) = v.get("delta") else {
                    return Ok(());
                };
                match delta.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text_delta" => {
                        if let Some(t) = delta.get("text").and_then(Value::as_str) {
                            out.text.push_str(t);
                            on_delta(Delta {
                                text: Some(t.to_string()),
                                reasoning: None,
                            });
                        }
                    }
                    "thinking_delta" => {
                        if let Some(t) = delta.get("thinking").and_then(Value::as_str) {
                            out.reasoning.push_str(t);
                            on_delta(Delta {
                                text: None,
                                reasoning: Some(t.to_string()),
                            });
                        }
                    }
                    "input_json_delta" => {
                        if let Some(part) = delta.get("partial_json").and_then(Value::as_str) {
                            calls.entry(index).or_default().2.push_str(part);
                        }
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(u) = v.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    out.usage.output = u;
                }
            }
            "error" => {
                let msg = v
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("anthropic error")
                    .to_string();
                failure = Some(Error::api(200, msg.clone()));
                return Err(Error::api(200, msg));
            }
            _ => {}
        }
        Ok(())
    })
    .await;

    if let Err(e) = res {
        if matches!(e, Error::Cancelled) {
            return Err(e);
        }
        return Err(failure.unwrap_or(e));
    }

    for (_, (id, name, args)) in calls {
        out.tool_calls.push(ToolCall {
            id: if id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                id
            },
            name,
            arguments: if args.trim().is_empty() {
                "{}".into()
            } else {
                args
            },
        });
    }
    Ok(out)
}

fn payload(req: &ChatRequest) -> Value {
    let (system, messages) = anthropic_messages(&req.messages);
    let mut budget = 0u32;
    if req.reasoning && req.effort != crate::types::Effort::Auto {
        budget = req.effort.budget();
    }
    let max_tokens = if budget > 0 {
        (budget + 4096).min(32_000)
    } else {
        req.max_tokens.max(1024)
    };
    let mut body = json!({
        "model": req.model,
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": true,
    });
    if !system.trim().is_empty() {
        body["system"] = json!(system.trim_end());
    }
    let tools: Vec<Value> = tool_defs(&req.tools)
        .into_iter()
        .map(|(name, desc, params)| {
            json!({
                "name": name,
                "description": desc,
                "input_schema": if params.is_null() { json!({"type": "object", "properties": {}}) } else { params.clone() },
            })
        })
        .collect();
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    if budget > 0 {
        body["thinking"] =
            json!({"type": "enabled", "budget_tokens": budget.min(max_tokens - 1024)});
    }
    body
}
