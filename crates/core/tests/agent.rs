mod common;

use std::sync::Arc;

use termagent_core::registry::{Model, Protocol, Provider, Registry};
use termagent_core::session::Session;
use termagent_core::tools::ApproveFn;
use termagent_core::types::{AgentEvent, Effort};
use termagent_core::{Agent, Config};
use tokio::sync::mpsc;

fn registry(base: &str) -> Registry {
    Registry {
        fetched_at: 0,
        source: "test".into(),
        providers: vec![Provider {
            id: "mock".into(),
            name: "Mock".into(),
            env: vec!["MOCK_API_KEY".into()],
            api: base.to_string(),
            protocol: Protocol::OpenAi,
            models: vec![Model {
                id: "mock-model".into(),
                name: "Mock Model".into(),
                reasoning: true,
                efforts: vec!["low".into(), "medium".into(), "high".into()],
                tool_call: true,
                context: 128_000,
                cost_in: 1.0,
                cost_out: 2.0,
            }],
        }],
    }
}

fn cfg() -> Config {
    Config {
        provider: "mock".into(),
        model: "mock-model".into(),
        api_key: "test-key".into(),
        effort: Effort::Auto,
        ..Config::default()
    }
}

fn always_approve() -> ApproveFn {
    Arc::new(|_req| Box::pin(async { true }))
}

#[tokio::test]
async fn agent_runs_a_tool_round_then_answers() {
    let _ = common::test_config_dir();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("note.txt"), "the answer is 42\n").unwrap();

    let first = common::sse(&[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"read","arguments":"{\"path\":\"note.txt\"}"}}]}}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        r#"{"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":6}}"#,
        "[DONE]",
    ]);
    let second = common::sse(&[
        r#"{"choices":[{"delta":{"content":"The file says 42."}}]}"#,
        r#"{"choices":[],"usage":{"prompt_tokens":30,"completion_tokens":8}}"#,
        "[DONE]",
    ]);
    let (base, hits) = common::spawn_server(vec![(200, first), (200, second)]).await;

    let session = Session::new(work.path(), "mock", "mock-model", Effort::Auto).unwrap();
    let session_id = session.meta.id.clone();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut agent = Agent::new(cfg(), registry(&base), session, tx, always_approve()).unwrap();

    agent
        .run_turn(
            "refactor note.txt and tell me the answer",
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolStart { name, .. } if name == "read")));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::ToolResult { ok: true, .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::Text(t) if t.contains("42"))));
    // Auto effort promotes a task-shaped prompt to high.
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::Effort(Effort::High))));
    assert!(matches!(events.last(), Some(AgentEvent::Done)));

    let reloaded = Session::open(&session_id).unwrap();
    let messages = reloaded.messages();
    assert!(messages.iter().any(|m| m
        .content
        .as_deref()
        .unwrap_or_default()
        .contains("the answer is 42")));
    assert_eq!(reloaded.totals.input, 50);
    assert_eq!(reloaded.totals.output, 14);

    let raw = std::fs::read_to_string(reloaded.path).unwrap();
    assert!(!raw.contains("test-key"), "session leaked the api key");
    assert!(raw.contains("The file says 42."));
}

#[tokio::test]
async fn cancellation_stops_the_turn() {
    let _ = common::test_config_dir();
    let work = tempfile::tempdir().unwrap();
    let (base, _) = common::spawn_server(vec![(200, common::sse(&["[DONE]"]))]).await;
    let session = Session::new(work.path(), "mock", "mock-model", Effort::Low).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut agent = Agent::new(cfg(), registry(&base), session, tx, always_approve()).unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    cancel.cancel();
    let err = agent.run_turn("do work", cancel).await.unwrap_err();
    assert!(matches!(err, termagent_core::Error::Cancelled));
    assert!(agent.session.messages().is_empty());
}

#[tokio::test]
async fn denial_is_reported_to_the_model() {
    let _ = common::test_config_dir();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("secret.txt"), "hidden").unwrap();
    let first = common::sse(&[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c2","function":{"name":"bash","arguments":"{\"command\":\"rm -rf build\"}"}}]}}]}"#,
        "[DONE]",
    ]);
    let second = common::sse(&[
        r#"{"choices":[{"delta":{"content":"Understood, not running it."}}]}"#,
        "[DONE]",
    ]);
    let (base, _) = common::spawn_server(vec![(200, first), (200, second)]).await;

    let session = Session::new(work.path(), "mock", "mock-model", Effort::Low).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let deny: ApproveFn = Arc::new(|_| Box::pin(async { false }));
    let mut agent = Agent::new(cfg(), registry(&base), session, tx, deny).unwrap();
    agent
        .run_turn(
            "clean the build",
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();

    let mut approval_seen = false;
    let mut denied_seen = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            AgentEvent::Approval(req) => {
                approval_seen = true;
                assert_eq!(req.tool, "bash");
            }
            AgentEvent::ToolResult { ok, output, .. } => {
                denied_seen = !ok && output.contains("Denied");
            }
            _ => {}
        }
    }
    assert!(approval_seen && denied_seen);
}

#[tokio::test]
async fn auto_approve_skips_medium_but_not_high_risk() {
    let _ = common::test_config_dir();
    let work = tempfile::tempdir().unwrap();
    let first = common::sse(&[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c3","function":{"name":"write","arguments":"{\"path\":\"a.txt\",\"content\":\"x\"}"}}]}}]}"#,
        "[DONE]",
    ]);
    let second = common::sse(&[r#"{"choices":[{"delta":{"content":"done"}}]}"#, "[DONE]"]);
    let (base, _) = common::spawn_server(vec![(200, first), (200, second)]).await;
    let session = Session::new(work.path(), "mock", "mock-model", Effort::Low).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut config = cfg();
    config.auto_approve = true;
    let mut agent = Agent::new(config, registry(&base), session, tx, always_approve()).unwrap();
    agent
        .run_turn("write a.txt", tokio_util::sync::CancellationToken::new())
        .await
        .unwrap();
    let mut saw_approval = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, AgentEvent::Approval(_)) {
            saw_approval = true;
        }
    }
    assert!(
        !saw_approval,
        "medium-risk write asked despite auto-approve"
    );
    assert!(work.path().join("a.txt").exists());
}
