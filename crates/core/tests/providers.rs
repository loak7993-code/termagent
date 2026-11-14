mod common;

use serde_json::json;
use termagent_core::providers::{self, ChatRequest, ProviderSpec};
use termagent_core::registry::Protocol;
use termagent_core::types::{Delta, Effort, Message};
use tokio_util::sync::CancellationToken;

fn req() -> ChatRequest {
    ChatRequest {
        model: "m".into(),
        messages: vec![Message::user("hi")],
        tools: json!([{"type":"function","function":{"name":"read","description":"read","parameters":{"type":"object","properties":{}}}}]),
        effort: Effort::Medium,
        reasoning: true,
        max_tokens: 2048,
    }
}

fn spec(base: &str, protocol: Protocol) -> ProviderSpec {
    ProviderSpec {
        protocol,
        base_url: base.to_string(),
        api_key: "test-key".into(),
    }
}

#[tokio::test]
async fn openai_streams_text_usage_and_tool_calls() {
    let body = common::sse(&[
        r#"{"choices":[{"delta":{"content":"Hel"}}]}"#,
        r#"{"choices":[{"delta":{"content":"lo"}}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{\"path\":"}}]}}]}"#,
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a.txt\"}"}}]}}]}"#,
        r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
        "[DONE]",
    ]);
    let (base, _hits) = common::spawn_server(vec![(200, body)]).await;
    let mut deltas: Vec<Delta> = Vec::new();
    let out = providers::stream(
        &spec(&base, Protocol::OpenAi),
        &req(),
        &CancellationToken::new(),
        &mut |d| deltas.push(d),
    )
    .await
    .unwrap();

    assert_eq!(out.text, "Hello");
    assert_eq!(out.usage.input, 10);
    assert_eq!(out.usage.output, 5);
    assert_eq!(out.tool_calls.len(), 1);
    assert_eq!(out.tool_calls[0].name, "read");
    let args: serde_json::Value = serde_json::from_str(&out.tool_calls[0].arguments).unwrap();
    assert_eq!(args["path"], "a.txt");
    assert!(deltas.iter().any(|d| d.text.as_deref() == Some("Hel")));
}

#[tokio::test]
async fn openai_surfaces_auth_errors() {
    let (base, _) =
        common::spawn_server(vec![(401, "{\"error\":{\"message\":\"bad key\"}}".into())]).await;
    let err = providers::stream(
        &spec(&base, Protocol::OpenAi),
        &req(),
        &CancellationToken::new(),
        &mut |_| {},
    )
    .await
    .unwrap_err();
    let text = err.to_string();
    assert!(text.contains("401"), "{text}");
    assert!(text.contains("bad key"), "{text}");
}

#[tokio::test]
async fn anthropic_streams_text_thinking_and_tool_use() {
    let body = common::sse(&[
        r#"{"type":"message_start","message":{"usage":{"input_tokens":12}}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"plan"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read"}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"a.txt\"}"}}"#,
        r#"{"type":"message_delta","usage":{"output_tokens":7}}"#,
        r#"{"type":"message_stop"}"#,
    ]);
    let (base, _) = common::spawn_server(vec![(200, body)]).await;
    let out = providers::stream(
        &spec(&base, Protocol::Anthropic),
        &req(),
        &CancellationToken::new(),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(out.text, "Hi");
    assert_eq!(out.reasoning, "plan");
    assert_eq!(out.usage.input, 12);
    assert_eq!(out.usage.output, 7);
    assert_eq!(out.tool_calls[0].id, "toolu_1");
    let args: serde_json::Value = serde_json::from_str(&out.tool_calls[0].arguments).unwrap();
    assert_eq!(args["path"], "a.txt");
}

#[tokio::test]
async fn gemini_streams_text_and_function_call() {
    let body = common::sse(&[
        r#"{"candidates":[{"content":{"parts":[{"text":"Hi"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":2}}"#,
        r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read","args":{"path":"a.txt"}}}]}}]}"#,
    ]);
    let (base, _) = common::spawn_server(vec![(200, body)]).await;
    let out = providers::stream(
        &spec(&base, Protocol::Gemini),
        &req(),
        &CancellationToken::new(),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(out.text, "Hi");
    assert_eq!(out.usage.input, 3);
    assert_eq!(out.tool_calls[0].name, "read");
    let args: serde_json::Value = serde_json::from_str(&out.tool_calls[0].arguments).unwrap();
    assert_eq!(args["path"], "a.txt");
}

#[tokio::test]
async fn cancellation_wins_before_request() {
    let (base, _) = common::spawn_server(vec![(200, common::sse(&["[DONE]"]))]).await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = providers::stream(&spec(&base, Protocol::OpenAi), &req(), &cancel, &mut |_| {})
        .await
        .unwrap_err();
    assert!(matches!(err, termagent_core::Error::Cancelled));
}
