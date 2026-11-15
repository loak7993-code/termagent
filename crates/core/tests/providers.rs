mod common;

use serde_json::json;
use termagent_core::providers::{self, ChatRequest, ProviderSpec};
use termagent_core::registry::Protocol;
use termagent_core::types::{Delta, Effort, Message};
use termagent_core::{Config, Registry};
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
        project: None,
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

#[test]
fn coreweave_project_is_optional() {
    let registry = Registry::snapshot().unwrap();
    let provider = registry.provider("wandb").expect("wandb provider");
    let mut cfg = Config {
        provider: "wandb".into(),
        model: "deepseek-ai/DeepSeek-V4.1-Flash-1017".into(),
        ..Config::default()
    };
    cfg.keys.insert("wandb".into(), "key".into());
    let spec = ProviderSpec::resolve(provider, &cfg).unwrap();
    assert!(spec.project.is_none());
    cfg.project = Some("acme/agent".into());
    let spec = ProviderSpec::resolve(provider, &cfg).unwrap();
    assert_eq!(spec.project.as_deref(), Some("acme/agent"));
}

#[tokio::test]
async fn openai_sends_the_project_header() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 65536];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
        let (status, body) = if request.contains("openai-project: acme/agent") {
            (
                "200 OK",
                "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n",
            )
        } else {
            (
                "401 Unauthorized",
                "{\"error\":{\"message\":\"missing project\"}}",
            )
        };
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    let mut spec = spec(&format!("http://{addr}"), Protocol::OpenAi);
    spec.project = Some("acme/agent".into());
    let out = providers::stream(&spec, &req(), &CancellationToken::new(), &mut |_| {})
        .await
        .unwrap();
    assert_eq!(out.text, "ok");
}
