pub mod api;
pub mod assets;
pub mod state;
pub mod ws;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use axum::Router;
use termagent_core::types::AgentEvent;
use termagent_core::{Config, Registry};
use tokio::sync::{broadcast, RwLock};

pub use state::State;

pub fn build_state(cfg: Config, registry: Registry, cwd: std::path::PathBuf, port: u16) -> State {
    let (events, _) = broadcast::channel::<String>(2048);
    let (agent_tx, mut agent_rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let broadcaster = events.clone();
    tokio::spawn(async move {
        while let Some(ev) = agent_rx.recv().await {
            if let Some(value) = state::event_json(&ev) {
                let _ = broadcaster.send(value.to_string());
            }
        }
    });

    State {
        cfg: Arc::new(RwLock::new(cfg)),
        registry: Arc::new(RwLock::new(registry)),
        agent: Arc::new(tokio::sync::Mutex::new(None)),
        session: Arc::new(RwLock::new(None)),
        totals: Arc::new(RwLock::new(Default::default())),
        approvals: Arc::new(state::WebApprovals::default()),
        events,
        cancel: Arc::new(Mutex::new(None)),
        busy: Arc::new(AtomicBool::new(false)),
        token: uuid::Uuid::new_v4().simple().to_string(),
        port,
        cwd,
        agent_tx,
    }
}

pub fn app(state: State) -> Router {
    Router::new()
        .merge(api::router(state.clone()))
        .merge(ws::router())
        .fallback(assets::handler)
        .with_state(state)
}

pub async fn serve(cfg: Config, registry: Registry, port: u16) -> termagent_core::Result<()> {
    let cwd = std::env::current_dir()?;
    let configured = cfg.is_configured(&registry);
    let state = build_state(cfg, registry, cwd, port);
    if configured {
        if let Err(e) = state.start_agent(None).await {
            eprintln!("termagent: {e}");
        }
    }

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| termagent_core::Error::Config(format!("cannot bind 127.0.0.1:{port}: {e}")))?;
    println!("termagent web: http://127.0.0.1:{port}  (local only, token protected)");
    axum::serve(listener, app(state))
        .await
        .map_err(|e| termagent_core::Error::Http(e.to_string()))?;
    Ok(())
}
