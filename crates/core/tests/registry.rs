#![allow(clippy::field_reassign_with_default)]

mod common;

use termagent_core::registry::{Model, Protocol, Provider, Registry};
use termagent_core::types::Effort;
use termagent_core::Config;

#[test]
fn snapshot_has_supported_providers_and_models() {
    let reg = Registry::snapshot().expect("snapshot");
    assert!(
        reg.providers.len() > 100,
        "providers: {}",
        reg.providers.len()
    );
    assert!(reg.total_models() > 1000, "models: {}", reg.total_models());
    for p in &reg.providers {
        assert!(!p.api.is_empty(), "{} has no base url", p.id);
        assert!(!p.models.is_empty(), "{} has no tool models", p.id);
        assert!(p
            .models
            .iter()
            .all(|m| m.tool_call && !m.id.is_empty() && !m.name.is_empty()));
    }
    let names: Vec<&str> = reg.providers.iter().map(|p| p.id.as_str()).collect();
    for excluded in ["amazon-bedrock", "google-vertex", "azure", "github-copilot"] {
        assert!(!names.contains(&excluded), "{excluded} must be excluded");
    }
    for required in [
        "anthropic",
        "openai",
        "google",
        "openrouter",
        "opencode",
        "lmstudio",
    ] {
        assert!(names.contains(&required), "{required} missing");
    }
}

#[test]
fn protocols_are_assigned_by_provider() {
    let reg = Registry::snapshot().unwrap();
    assert_eq!(
        reg.provider("anthropic").unwrap().protocol,
        Protocol::Anthropic
    );
    assert_eq!(reg.provider("google").unwrap().protocol, Protocol::Gemini);
    assert_eq!(
        reg.provider("openrouter").unwrap().protocol,
        Protocol::OpenAi
    );
}

#[test]
fn reasoning_models_expose_efforts() {
    let reg = Registry::snapshot().unwrap();
    let with_effort = reg
        .providers
        .iter()
        .flat_map(|p| &p.models)
        .filter(|m| m.reasoning && !m.efforts.is_empty())
        .count();
    assert!(
        with_effort > 50,
        "only {with_effort} models expose effort values"
    );
}

#[test]
fn config_roundtrip_and_permissions() {
    let dir = common::test_config_dir();
    let mut cfg = Config::default();
    cfg.provider = "anthropic".into();
    cfg.model = "claude-fable-5-2".into();
    cfg.api_key = "sk-test".into();
    cfg.effort = Effort::High;
    cfg.save().unwrap();

    let loaded = Config::load().unwrap();
    assert_eq!(loaded.provider, "anthropic");
    assert_eq!(loaded.effort, Effort::High);
    let path = dir.join("config.json");
    let meta = std::fs::metadata(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn keys_are_scoped_to_one_provider() {
    let reg = Registry::snapshot().unwrap();
    let mut cfg = Config::default();
    cfg.provider = "anthropic".into();
    cfg.model = "claude-fable-5-2".into();
    cfg.keys.insert("anthropic".into(), "sk-ant".into());
    assert_eq!(
        cfg.key_for(reg.provider("anthropic").unwrap()).as_deref(),
        Some("sk-ant")
    );
    assert!(cfg.key_for(reg.provider("openai").unwrap()).is_none());
    assert!(cfg.key_for(reg.provider("groq").unwrap()).is_none());
}

#[test]
fn env_key_counts_as_configured() {
    let reg = Registry::snapshot().unwrap();
    let provider = reg.provider("anthropic").unwrap();
    std::env::set_var("ANTHROPIC_API_KEY", "sk-from-env");
    let mut cfg = Config::default();
    cfg.provider = "anthropic".into();
    cfg.model = "claude-fable-5-2".into();
    assert!(cfg.is_configured(&reg));
    assert_eq!(cfg.key_for(provider).unwrap(), "sk-from-env");
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
fn parse_rejects_unsupported_provider_shape() {
    let body = r#"{
      "anthropic": {"id":"anthropic","name":"Anthropic","env":["ANTHROPIC_API_KEY"],
        "models":{"x":{"id":"x","name":"X","tool_call":false,"modalities":{"output":["text"]}}}},
      "weird": {"id":"weird","name":"Weird","env":[], "api":null,
        "models":{"m":{"id":"m","name":"M","tool_call":true,"modalities":{"output":["text"]}}}}
    }"#;
    let reg = Registry::from_models_dev(body).unwrap();
    assert!(reg.provider("anthropic").is_none());
    assert!(reg.provider("weird").is_none());
}

#[test]
fn parse_keeps_effort_options_and_costs() {
    let body = r#"{
      "p": {"id":"p","name":"P","env":["P_API_KEY"],"api":"https://p.example/v1",
        "models":{"m":{"id":"m","name":"M","tool_call":true,"reasoning":true,
          "reasoning_options":[{"type":"effort","values":["low","high"]}],
          "limit":{"context":128000},"cost":{"input":1.5,"output":6.0},
          "modalities":{"output":["text"]}}}}
    }"#;
    let reg = Registry::from_models_dev(body).unwrap();
    let m = &reg.provider("p").unwrap().models[0];
    assert_eq!(m.context, 128000);
    assert_eq!(m.cost_in, 1.5);
    assert_eq!(m.efforts, vec!["low".to_string(), "high".to_string()]);
}

#[test]
fn seeded_provider_roundtrip() {
    let reg = Registry {
        fetched_at: 0,
        source: "test".into(),
        providers: vec![Provider {
            id: "mock".into(),
            name: "Mock".into(),
            env: vec!["MOCK_API_KEY".into()],
            api: "http://127.0.0.1:1".into(),
            protocol: Protocol::OpenAi,
            models: vec![Model {
                id: "mock-model".into(),
                name: "Mock Model".into(),
                reasoning: true,
                efforts: vec!["low".into(), "high".into()],
                tool_call: true,
                context: 1000,
                cost_in: 0.0,
                cost_out: 0.0,
            }],
        }],
    };
    assert_eq!(reg.model("mock", "mock-model").unwrap().name, "Mock Model");
    assert!(reg.model("mock", "nope").is_none());
}

#[test]
fn sessions_rename_and_delete() {
    let _ = common::test_config_dir();
    let work = tempfile::tempdir().unwrap();
    let session = termagent_core::session::Session::new(
        work.path(),
        "anthropic",
        "claude-fable-5-2",
        Effort::Auto,
    )
    .unwrap();
    let id = session.meta.id.clone();
    termagent_core::session::rename(&id, "fix the flaky test").unwrap();
    let listed = termagent_core::session::list();
    let meta = listed.iter().find(|m| m.id == id).unwrap();
    assert_eq!(meta.title, "fix the flaky test");
    termagent_core::session::delete(&id).unwrap();
    assert!(!termagent_core::session::list().iter().any(|m| m.id == id));
    assert!(termagent_core::session::Session::open("../../etc/passwd").is_err());
}

#[test]
fn session_replays_history_and_title() {
    let _ = common::test_config_dir();
    let work = tempfile::tempdir().unwrap();
    let mut session = termagent_core::session::Session::new(
        work.path(),
        "anthropic",
        "claude-fable-5-2",
        Effort::Auto,
    )
    .unwrap();
    let id = session.meta.id.clone();
    use termagent_core::session::Stored;
    use termagent_core::types::Risk;
    session
        .append(&Stored::User {
            text: "read main.rs".into(),
        })
        .unwrap();
    session
        .append(&Stored::Assistant {
            text: "Sure.".into(),
            reasoning: Some("plan".into()),
            tool_calls: vec![],
        })
        .unwrap();
    session
        .append(&Stored::Tool {
            id: "t1".into(),
            name: "read".into(),
            args: serde_json::json!({"path": "main.rs"}),
            output: "fn main() {}".into(),
            ok: true,
            risk: Risk::Low,
        })
        .unwrap();
    session
        .append(&Stored::Usage {
            input: 10,
            output: 4,
        })
        .unwrap();
    session.apply_title("read main.rs").unwrap();

    let reopened = termagent_core::session::Session::open(&id).unwrap();
    assert_eq!(reopened.meta.title, "read main.rs");
    let messages = reopened.messages();
    assert_eq!(messages.len(), 3);
    assert!(messages[0]
        .content
        .as_deref()
        .unwrap()
        .contains("read main.rs"));
    assert!(messages[1].reasoning.as_deref().unwrap().contains("plan"));
    assert!(messages[2].content.as_deref().unwrap().contains("fn main"));
    assert_eq!(reopened.totals.input, 10);
}
