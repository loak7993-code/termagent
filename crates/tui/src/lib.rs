pub mod app;
pub mod input;
pub mod onboarding;
pub mod theme;
pub mod ui;
pub mod util;
pub mod worker;

use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use termagent_core::types::AgentEvent;
use termagent_core::{Config, Registry};
use tokio::sync::mpsc::unbounded_channel;

use crate::app::App;
use crate::worker::{Approvals, Command, TuiEvent, Worker};

pub fn run(cfg: Config, registry: Registry, cwd: PathBuf) -> termagent_core::Result<()> {
    run_inner(cfg, registry, cwd, false)
}

/// Always show the provider/key/model flow, even when a config exists.
pub fn run_onboarding(cfg: Config, registry: Registry, cwd: PathBuf) -> termagent_core::Result<()> {
    run_inner(cfg, registry, cwd, true)
}

fn run_inner(
    cfg: Config,
    registry: Registry,
    cwd: PathBuf,
    force_onboarding: bool,
) -> termagent_core::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()?;

    let (agent_tx, mut agent_rx) = unbounded_channel::<AgentEvent>();
    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<TuiEvent>();
    let bridge = ui_tx.clone();
    runtime.spawn(async move {
        while let Some(ev) = agent_rx.recv().await {
            if bridge.send(TuiEvent::Agent(ev)).is_err() {
                break;
            }
        }
    });

    let (cmd_tx, cmd_rx) = unbounded_channel::<Command>();
    let approvals = Approvals::default();
    let worker = Worker {
        cfg: cfg.clone(),
        registry: registry.clone(),
        cwd,
        agent: None,
        events: agent_tx,
        ui: ui_tx,
        approvals: approvals.clone(),
    };
    runtime.spawn(worker.run(cmd_rx));

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cfg, registry, cmd_tx, approvals, ui_rx, force_onboarding);
    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> termagent_core::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        app.tick = app.tick.wrapping_add(1);
        if event::poll(Duration::from_millis(60))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.handle_key(key),
                Event::Paste(text) => app.paste(&text),
                _ => {}
            }
        }
        app.poll_events();
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
