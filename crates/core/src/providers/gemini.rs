use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::providers::{
    gemini_contents, http_error, sse, tool_defs, ChatOutcome, ChatRequest, ProviderSpec,
};
use crate::types::{Delta, Effort, ToolCall, Usage};

pub async fn stream(
    spec: &ProviderSpec,
    req: &ChatRequest,
    cancel: &CancellationToken,
    on_delta: &mut (dyn FnMut(Delta) + Send),
) -> Result<ChatOutcome> {
    let body = payload(req);
    let model =
        percent_encoding::utf8_percent_encode(&req.model, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
    let url = format!(
        "{}/models/{model}:streamGenerateContent?alt=sse",
        spec.base_url
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| Error::Http(e.to_string()))?;
    let fut = client
        .post(url)
        .header("x-goog-api-key", &spec.api_key)
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
    let mut failure: Option<Error> = None;

    let res = sse::read_sse(resp, cancel, |data| {
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return Ok(());
        };
        if let Some(err) = v.get("error") {
            let msg = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("gemini error")
                .to_string();
            failure = Some(Error::api(200, msg.clone()));
            return Err(Error::api(200, msg));
        }
        if let Some(u) = v.get("usageMetadata") {
            out.usage = Usage {
                input: u
                    .get("promptTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                output: u
                    .get("candidatesTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
            };
            if out.usage.input == 0 {
                out.usage.input = u
                    .get("totalTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            }
        }
        let Some(parts) = v
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
        else {
            return Ok(());
        };
        for part in parts {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                if let Some(t) = part.get("text").and_then(Value::as_str) {
                    out.reasoning.push_str(t);
                    on_delta(Delta {
                        text: None,
                        reasoning: Some(t.to_string()),
                    });
                }
                continue;
            }
            if let Some(t) = part.get("text").and_then(Value::as_str) {
                out.text.push_str(t);
                on_delta(Delta {
                    text: Some(t.to_string()),
                    reasoning: None,
                });
            }
            if let Some(call) = part.get("functionCall") {
                let name = call.get("name").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() {
                    continue;
                }
                let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
                out.tool_calls.push(ToolCall {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: name.to_string(),
                    arguments: args.to_string(),
                });
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
    Ok(out)
}

fn payload(req: &ChatRequest) -> Value {
    let (system, contents) = gemini_contents(&req.messages);
    let mut body = json!({
        "contents": contents,
        "generationConfig": {"maxOutputTokens": req.max_tokens.max(2048)},
    });
    if !system.trim().is_empty() {
        body["systemInstruction"] = json!({"parts": [{"text": system.trim_end()}]});
    }
    let decls: Vec<Value> = tool_defs(&req.tools)
        .into_iter()
        .map(|(name, desc, params)| {
            json!({
                "name": name,
                "description": desc,
                "parameters": if params.is_null() { json!({"type": "object", "properties": {}}) } else { params.clone() },
            })
        })
        .collect();
    if !decls.is_empty() {
        body["tools"] = json!([{ "functionDeclarations": decls }]);
    }
    if req.reasoning && req.effort != Effort::Auto {
        let budget = req.effort.budget();
        body["generationConfig"]["thinkingConfig"] = json!({"thinkingBudget": budget});
    }
    body
}
