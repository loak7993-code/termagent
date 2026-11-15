use std::collections::BTreeMap;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::providers::{http_error, sse, ChatOutcome, ChatRequest, ProviderSpec};
use crate::types::{Delta, Message, Role, ToolCall, Usage};

pub async fn stream(
    spec: &ProviderSpec,
    req: &ChatRequest,
    cancel: &CancellationToken,
    on_delta: &mut (dyn FnMut(Delta) + Send),
) -> Result<ChatOutcome> {
    let mut include_usage = true;
    loop {
        let body = payload(req, include_usage);
        let resp = send(spec, &body, cancel).await?;
        if resp.status().as_u16() == 400 && include_usage {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if text.contains("stream_options") {
                include_usage = false;
                continue;
            }
            return Err(Error::api(status.as_u16(), text));
        }
        if !resp.status().is_success() {
            return Err(http_error(resp).await);
        }
        return parse(resp, cancel, on_delta).await;
    }
}

async fn send(
    spec: &ProviderSpec,
    body: &Value,
    cancel: &CancellationToken,
) -> Result<reqwest::Response> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| Error::Http(e.to_string()))?;
    let mut request = client
        .post(format!("{}/chat/completions", spec.base_url))
        .bearer_auth(&spec.api_key)
        .header("accept", "text/event-stream");
    if let Some(project) = &spec.project {
        request = request.header("OpenAI-Project", project);
    }
    let fut = request.json(body).send();
    tokio::select! {
        _ = cancel.cancelled() => Err(Error::Cancelled),
        resp = fut => resp.map_err(|e| Error::Http(format!("request: {e}"))),
    }
}

fn payload(req: &ChatRequest, include_usage: bool) -> Value {
    let mut body = json!({
        "model": req.model,
        "messages": req.messages.iter().map(openai_message).collect::<Vec<_>>(),
        "stream": true,
    });
    if let Some(arr) = req.tools.as_array() {
        if !arr.is_empty() {
            body["tools"] = req.tools.clone();
            body["tool_choice"] = json!("auto");
        }
    }
    if include_usage {
        body["stream_options"] = json!({"include_usage": true});
    }
    if req.reasoning {
        body["reasoning_effort"] = json!(req.effort.api_value());
    }
    body
}

async fn parse(
    resp: reqwest::Response,
    cancel: &CancellationToken,
    on_delta: &mut (dyn FnMut(Delta) + Send),
) -> Result<ChatOutcome> {
    let mut out = ChatOutcome::default();
    let mut calls: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
    let mut failure: Option<Error> = None;

    let res = sse::read_sse(resp, cancel, |data| {
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(());
        };
        if let Some(err) = v.get("error") {
            let msg = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("provider error");
            failure = Some(Error::api(200, msg));
            return Err(Error::api(200, msg));
        }
        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
            out.usage = Usage {
                input: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
                output: u
                    .get("completion_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            };
        }
        let Some(choice) = v.get("choices").and_then(|c| c.get(0)) else {
            return Ok(());
        };
        let Some(delta) = choice.get("delta") else {
            return Ok(());
        };
        let text = delta
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_string);
        let reasoning = delta
            .get("reasoning_content")
            .or_else(|| delta.get("reasoning"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(t) = &text {
            out.text.push_str(t);
        }
        if let Some(r) = &reasoning {
            out.reasoning.push_str(r);
        }
        if text.is_some() || reasoning.is_some() {
            on_delta(Delta {
                text: text.clone(),
                reasoning: reasoning.clone(),
            });
        }
        if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
            for tc in tcs {
                let index = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let entry = calls
                    .entry(index)
                    .or_insert_with(|| (String::new(), String::new(), String::new()));
                if let Some(id) = tc.get("id").and_then(Value::as_str) {
                    if !id.is_empty() {
                        entry.0 = id.to_string();
                    }
                }
                if let Some(f) = tc.get("function") {
                    if let Some(name) = f.get("name").and_then(Value::as_str) {
                        if !name.is_empty() {
                            entry.1 = name.to_string();
                        }
                    }
                    if let Some(args) = f.get("arguments").and_then(Value::as_str) {
                        entry.2.push_str(args);
                    }
                }
            }
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
        if name.is_empty() {
            continue;
        }
        out.tool_calls.push(ToolCall {
            id: if id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                id
            },
            name,
            arguments: if args.trim().is_empty() {
                "{}".to_string()
            } else {
                args
            },
        });
    }
    Ok(out)
}

/// Message JSON in OpenAI Chat Completions shape.
fn openai_message(m: &Message) -> Value {
    match m.role {
        Role::Tool => json!({
            "role": "tool",
            "tool_call_id": m.tool_call_id,
            "content": m.content,
        }),
        Role::Assistant => {
            let mut v = json!({
                "role": "assistant",
                "content": m.content,
            });
            if !m.tool_calls.is_empty() {
                v["tool_calls"] = json!(m
                    .tool_calls
                    .iter()
                    .map(|tc| json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {"name": tc.name, "arguments": tc.arguments}
                    }))
                    .collect::<Vec<_>>());
            }
            v
        }
        Role::System => json!({"role": "system", "content": m.content}),
        Role::User => json!({"role": "user", "content": m.content}),
    }
}
