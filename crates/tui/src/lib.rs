pub mod app;
pub mod input;
pub mod onboarding;
pub mod theme;
pub mod ui;
pub mod util;
pub mod worker;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::cursor::Show;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
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

pub fn run(
    cfg: Config,
    registry: Registry,
    cwd: PathBuf,
    session_id: Option<String>,
) -> termagent_core::Result<()> {
    run_inner(cfg, registry, cwd, false, session_id)
}

/// Always show the provider/key/model flow, even when a config exists.
pub fn run_onboarding(cfg: Config, registry: Registry, cwd: PathBuf) -> termagent_core::Result<()> {
    run_inner(cfg, registry, cwd, true, None)
}

fn run_inner(
    cfg: Config,
    registry: Registry,
    cwd: PathBuf,
    force_onboarding: bool,
    initial_session: Option<String>,
) -> termagent_core::Result<()> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(termagent_core::Error::Terminal(
            "the terminal UI needs an interactive terminal. Run `termagent --web` and configure in the browser, or start termagent from a real terminal.".into(),
        ));
    }

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

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            restore_terminal();
            return Err(error.into());
        }
    };

    // Termux and some ptys briefly report 0x0. Wait for a real size instead of
    // showing an empty alternate screen.
    let mut size = match terminal.size() {
        Ok(size) => size,
        Err(error) => {
            restore_terminal();
            return Err(error.into());
        }
    };
    for _ in 0..20 {
        if size.width >= 40 && size.height >= 12 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        size = match terminal.size() {
            Ok(size) => size,
            Err(error) => {
                restore_terminal();
                return Err(error.into());
            }
        };
    }
    if size.width < 40 || size.height < 12 {
        restore_terminal();
        return Err(termagent_core::Error::Terminal(format!(
            "terminal reported {}x{}; resize it to at least 40x12 and retry. In Termux, reopen the terminal or run `stty rows 24 cols 80`.",
            size.width, size.height
        )));
    }

    let mut app = App::new(
        cfg,
        registry,
        cmd_tx,
        approvals,
        ui_rx,
        force_onboarding,
        initial_session,
    );
    let result = event_loop(&mut terminal, &mut app);

    restore_terminal();
    result
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = std::io::stdout();
    let _ = execute!(stdout, DisableBracketedPaste, LeaveAlternateScreen, Show);
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
