#![allow(clippy::field_reassign_with_default)]

use std::sync::mpsc::channel;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use termagent_core::types::{ApprovalRequest, Effort, Risk};
use termagent_core::{Config, Registry};
use termagent_tui::app::{App, Item, ModelPicker, Overlay, Pane, ToolCard};
use termagent_tui::onboarding::Step;
use termagent_tui::ui;
use termagent_tui::worker::{Approvals, Command, TuiEvent};

fn app(cfg: Config) -> App {
    let registry = Registry::snapshot().unwrap();
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<Command>();
    let (_ui_tx, ui_rx) = channel::<TuiEvent>();
    App::new(
        cfg,
        registry,
        cmd_tx,
        Approvals::default(),
        ui_rx,
        false,
        None,
    )
}

fn render(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn configured() -> Config {
    let mut cfg = Config::default();
    cfg.provider = "anthropic".into();
    cfg.model = "claude-fable-5-2".into();
    cfg.api_key = "sk-test".into();
    cfg.effort = Effort::Auto;
    cfg
}

#[test]
fn first_run_shows_provider_picker() {
    let app = app(Config::default());
    let screen = render(&app, 120, 36);
    assert!(screen.contains("termagent"), "{screen}");
    assert!(screen.contains("first run"));
    assert!(screen.contains("Pick a provider"));
    assert!(screen.contains("Anthropic"));
    assert!(screen.contains("provider → key → model → effort"));
}

#[test]
fn onboarding_key_step_masks_the_key() {
    let mut app = app(Config::default());
    {
        let ob = app.onboarding.as_mut().unwrap();
        ob.provider = Some(app.registry.provider("anthropic").unwrap().clone());
        ob.step = Step::Key;
        ob.key.insert_str("sk-secret-123");
    }
    let screen = render(&app, 120, 36);
    assert!(screen.contains("•••"), "{screen}");
    assert!(!screen.contains("sk-secret-123"));
    assert!(screen.contains("ANTHROPIC_API_KEY"));
}

#[test]
fn chat_renders_questions_tools_and_usage() {
    let mut app = app(configured());
    app.chat.message(Item::User("read src/main.rs".into()));
    app.chat.message(Item::Assistant(
        "It is a small binary. Two things stand out.".into(),
    ));
    app.chat.message(Item::Tool(ToolCard {
        id: "t1".into(),
        name: "read".into(),
        summary: "src/main.rs".into(),
        risk: Risk::Low,
        running: false,
        ok: true,
        output: "     1\tfn main() {}".into(),
        elapsed_ms: 12,
    }));
    app.chat.usage.input = 8400;
    app.chat.usage.output = 512;
    app.chat.resolved = Some(Effort::High);
    let screen = render(&app, 120, 36);
    assert!(screen.contains("It is a small binary"));
    assert!(screen.contains("read"));
    assert!(screen.contains("[low]"));
    assert!(screen.contains("8.4K in / 512 out"), "{screen}");
    assert!(screen.contains("auto→high"), "{screen}");
    assert!(screen.contains("enter send"));
}

#[test]
fn approval_modal_takes_over() {
    let mut app = app(configured());
    app.chat.pending = Some(ApprovalRequest {
        id: "a1".into(),
        tool: "bash".into(),
        summary: "bash: rm -rf build".into(),
        args: serde_json::json!({"command": "rm -rf build"}),
        risk: Risk::High,
    });
    let screen = render(&app, 120, 36);
    assert!(screen.contains("approval"));
    assert!(screen.contains("high risk"));
    assert!(screen.contains("rm -rf build"));
    assert!(screen.contains("deny"));
}

#[test]
fn model_picker_scrolls_more_than_twenty_rows() {
    let mut app = app(configured());
    app.overlay = Overlay::Models(ModelPicker {
        pane: Pane::Models,
        provider_sel: 0,
        model_sel: 0,
        provider_query: Default::default(),
        model_query: Default::default(),
        provider_offset: 0,
        model_offset: 0,
    });
    // Put a provider with many models first.
    let openrouter = app
        .registry
        .provider("openrouter")
        .cloned()
        .expect("openrouter");
    app.registry.providers.retain(|p| p.id != "openrouter");
    app.registry.providers.insert(0, openrouter);
    let first = render(&app, 120, 40);
    assert!(first.contains("models"), "{first}");
    for _ in 0..25 {
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    let later = render(&app, 120, 40);
    assert_ne!(first, later, "picker did not scroll");
}

#[test]
fn narrow_terminal_does_not_overflow() {
    let mut app = app(configured());
    app.chat
        .message(Item::User("averylongwordwithoutspaces".repeat(12)));
    let screen = render(&app, 40, 24);
    for line in screen.lines() {
        assert!(line.chars().count() <= 40, "overflow: {line}");
    }
}
