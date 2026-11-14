use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::BoxFuture;
use termagent_core::session::SessionMeta;
use termagent_core::tools::ApproveFn;
use termagent_core::types::{AgentEvent, ApprovalRequest};
use termagent_core::{Agent, Config, Error, Registry, Session};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub struct Approvals {
    pub pending: Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<bool>>>>,
    pub answered: Arc<Mutex<HashMap<String, bool>>>,
}

impl Approvals {
    pub fn answer(&self, id: &str, allow: bool) {
        let tx = self.pending.lock().ok().and_then(|mut map| map.remove(id));
        if let Some(tx) = tx {
            let _ = tx.send(allow);
        } else if let Ok(mut map) = self.answered.lock() {
            map.insert(id.to_string(), allow);
        }
    }
}

pub fn gate(approvals: Approvals) -> ApproveFn {
    Arc::new(move |req: ApprovalRequest| -> BoxFuture<'static, bool> {
        let approvals = approvals.clone();
        Box::pin(async move {
            let pre = approvals
                .answered
                .lock()
                .ok()
                .and_then(|mut map| map.remove(&req.id));
            if let Some(v) = pre {
                return v;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            if let Ok(mut map) = approvals.pending.lock() {
                map.insert(req.id.clone(), tx);
            }
            let id = req.id.clone();
            let waited = tokio::task::spawn_blocking(move || {
                rx.recv_timeout(Duration::from_secs(64)).unwrap_or(false)
            })
            .await
            .unwrap_or(false);
            if let Ok(mut map) = approvals.pending.lock() {
                map.remove(&id);
            }
            waited
        })
    })
}

#[derive(Debug)]
pub enum TuiEvent {
    Agent(AgentEvent),
    Session(SessionMeta),
    Info(String),
}

pub enum Command {
    Start(Config, Option<String>),
    Turn(String, CancellationToken),
    Reconfigure,
    NewSession,
    LoadSession(String),
}

pub struct Worker {
    pub cfg: Config,
    pub registry: Registry,
    pub cwd: PathBuf,
    pub agent: Option<Agent>,
    pub events: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    pub ui: std::sync::mpsc::Sender<TuiEvent>,
    pub approvals: Approvals,
}

impl Worker {
    fn agent_for(&self, session: Session) -> Result<Agent, Error> {
        Agent::new(
            self.cfg.clone(),
            self.registry.clone(),
            session,
            self.events.clone(),
            gate(self.approvals.clone()),
        )
    }

    fn announce(&self, agent: &Agent) {
        let _ = self.ui.send(TuiEvent::Session(agent.session.meta.clone()));
    }

    fn start(&mut self, session_id: Option<String>) -> Result<(), Error> {
        let session = match session_id {
            Some(id) => Session::open(&id)?,
            None => Session::new(
                &self.cwd,
                &self.cfg.provider,
                &self.cfg.model,
                self.cfg.effort,
            )?,
        };
        let agent = self.agent_for(session)?;
        self.announce(&agent);
        self.agent = Some(agent);
        Ok(())
    }

    pub async fn run(mut self, mut rx: UnboundedReceiver<Command>) {
        if let Err(e) = self.start(None) {
            let _ = self.ui.send(TuiEvent::Info(e.to_string()));
            let _ = self.events.send(AgentEvent::Error(e.to_string()));
        }
        while let Some(cmd) = rx.recv().await {
            if let Err(e) = self.handle(cmd).await {
                let _ = self.ui.send(TuiEvent::Info(e.to_string()));
                let _ = self.events.send(AgentEvent::Error(e.to_string()));
            }
        }
    }

    async fn handle(&mut self, cmd: Command) -> Result<(), Error> {
        match cmd {
            Command::Start(cfg, session_id) => {
                self.cfg = cfg;
                self.start(session_id)
            }
            Command::Turn(text, cancel) => {
                let Some(agent) = self.agent.as_mut() else {
                    return Err(Error::Config("session is not ready".into()));
                };
                let result = agent.run_turn(&text, cancel).await;
                let meta = agent.session.meta.clone();
                let _ = self.ui.send(TuiEvent::Session(meta));
                match result {
                    Ok(()) => Ok(()),
                    Err(Error::Cancelled) => Ok(()),
                    Err(e) => Err(e),
                }
            }
            Command::Reconfigure => {
                if let Some(agent) = self.agent.as_mut() {
                    agent.reconfigure(&self.cfg, &self.registry)?;
                    let meta = agent.session.meta.clone();
                    let _ = self.ui.send(TuiEvent::Session(meta));
                }
                Ok(())
            }
            Command::NewSession => self.start(None),
            Command::LoadSession(id) => {
                let session = Session::open(&id)?;
                let agent = self.agent_for(session)?;
                self.announce(&agent);
                self.agent = Some(agent);
                Ok(())
            }
        }
    }
}
