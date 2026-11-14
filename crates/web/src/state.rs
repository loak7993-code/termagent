use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::BoxFuture;
use serde_json::{json, Value};
use termagent_core::session::{self, Session, SessionMeta};
use termagent_core::tools::ApproveFn;
use termagent_core::types::{AgentEvent, ApprovalRequest, Usage};
use termagent_core::{Agent, Config, Registry};
use tokio::sync::{broadcast, oneshot, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct WebApprovals {
    pub pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl WebApprovals {
    pub fn resolve(&self, id: &str, allow: bool) {
        if let Ok(mut map) = self.pending.lock() {
            if let Some(tx) = map.remove(id) {
                let _ = tx.send(allow);
            }
        }
    }
}

fn gate(approvals: Arc<WebApprovals>) -> ApproveFn {
    Arc::new(move |req: ApprovalRequest| -> BoxFuture<'static, bool> {
        let approvals = approvals.clone();
        Box::pin(async move {
            let (tx, rx) = oneshot::channel();
            if let Ok(mut map) = approvals.pending.lock() {
                map.insert(req.id.clone(), tx);
            }
            let result = tokio::time::timeout(Duration::from_secs(64), rx).await;
            if let Ok(mut map) = approvals.pending.lock() {
                map.remove(&req.id);
            }
            matches!(result, Ok(Ok(true)))
        })
    })
}

pub struct State {
    pub cfg: Arc<RwLock<Config>>,
    pub registry: Arc<RwLock<Registry>>,
    pub agent: Arc<tokio::sync::Mutex<Option<Agent>>>,
    pub session: Arc<RwLock<Option<SessionMeta>>>,
    pub totals: Arc<RwLock<Usage>>,
    pub approvals: Arc<WebApprovals>,
    pub events: broadcast::Sender<String>,
    pub cancel: Arc<Mutex<Option<CancellationToken>>>,
    pub busy: Arc<AtomicBool>,
    pub token: String,
    pub port: u16,
    pub cwd: PathBuf,
    pub agent_tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
}

impl Clone for State {
    fn clone(&self) -> Self {
        State {
            cfg: self.cfg.clone(),
            registry: self.registry.clone(),
            agent: self.agent.clone(),
            session: self.session.clone(),
            totals: self.totals.clone(),
            approvals: self.approvals.clone(),
            events: self.events.clone(),
            cancel: self.cancel.clone(),
            busy: self.busy.clone(),
            token: self.token.clone(),
            port: self.port,
            cwd: self.cwd.clone(),
            agent_tx: self.agent_tx.clone(),
        }
    }
}

impl State {
    pub fn broadcast(&self, value: Value) {
        let _ = self.events.send(value.to_string());
    }

    pub fn configured(&self, cfg: &Config, registry: &Registry) -> bool {
        cfg.is_configured(registry)
    }

    pub async fn start_agent(
        &self,
        session_id: Option<String>,
    ) -> termagent_core::Result<SessionMeta> {
        let cfg = self.cfg.read().await.clone();
        let registry = self.registry.read().await.clone();
        let session = match session_id {
            Some(id) => Session::open(&id)?,
            None => Session::new(&self.cwd, &cfg.provider, &cfg.model, cfg.effort)?,
        };
        let agent = Agent::new(
            cfg,
            registry,
            session,
            self.agent_tx.clone(),
            gate(self.approvals.clone()),
        )?;
        let meta = agent.session.meta.clone();
        if let Some(token) = self.cancel.lock().ok().and_then(|mut c| c.take()) {
            token.cancel();
        }
        *self.agent.lock().await = Some(agent);
        *self.session.write().await = Some(meta.clone());
        *self.totals.write().await = Usage::default();
        self.broadcast(json!({"type": "session", "session": session_payload(&meta, None)}));
        Ok(meta)
    }

    pub async fn session_payload(&self) -> Value {
        let meta = self.session.read().await.clone();
        match meta {
            Some(meta) => {
                let events = Session::open(&meta.id)
                    .map(|s| s.events)
                    .unwrap_or_default();
                session_payload(&meta, Some(&events))
            }
            None => Value::Null,
        }
    }

    pub async fn snapshot(&self) -> Value {
        let cfg = self.cfg.read().await.clone();
        let registry = self.registry.read().await.clone();
        let configured = cfg.is_configured(&registry);
        let providers: Vec<Value> = registry
            .providers
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "name": p.name,
                    "protocol": p.protocol.label(),
                    "env": p.env,
                    "api": p.api,
                    "configured": cfg.key_for(p).is_some(),
                    "keySet": cfg.direct_key(p).is_some(),
                    "local": p.api.contains("127.0.0.1") || p.api.contains("localhost"),
                    "envSet": p.env.iter().any(|v| {
                        std::env::var(v).map(|s| !s.is_empty()).unwrap_or(false)
                    }),
                    "models": p.models.len(),
                    "reasoningModels": p.models.iter().filter(|m| m.reasoning).count(),
                })
            })
            .collect();
        let sessions: Vec<Value> = session::list()
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "title": m.title,
                    "created": m.created,
                    "provider": m.provider,
                    "model": m.model,
                    "effort": m.effort,
                })
            })
            .collect();
        let current = self.session_payload().await;
        let totals = *self.totals.read().await;
        let model = registry.model(&cfg.provider, &cfg.model).cloned();
        json!({
            "version": termagent_core::VERSION,
            "configured": configured,
            "provider": cfg.provider,
            "model": cfg.model,
            "modelName": model.as_ref().map(|m| m.name.clone()),
            "reasoning": model.as_ref().map(|m| m.reasoning).unwrap_or(false),
            "efforts": model.as_ref().map(|m| m.effort_values()).unwrap_or_default(),
            "effort": cfg.effort,
            "autoApprove": cfg.auto_approve,
            "theme": cfg.theme,
            "providers": providers,
            "sessions": sessions,
            "session": current,
            "totals": {"input": totals.input, "output": totals.output},
            "registry": {"source": registry.source, "fetchedAt": registry.fetched_at, "providers": registry.providers.len(), "models": registry.total_models()},
            "busy": self.busy.load(Ordering::SeqCst),
        })
    }
}

pub fn session_payload(meta: &SessionMeta, events: Option<&[session::Stored]>) -> Value {
    json!({
        "id": meta.id,
        "title": meta.title,
        "created": meta.created,
        "cwd": meta.cwd,
        "provider": meta.provider,
        "model": meta.model,
        "effort": meta.effort,
        "events": events.map(|e| serde_json::to_value(e).unwrap_or(Value::Null)),
    })
}

pub fn event_json(ev: &AgentEvent) -> Option<Value> {
    let value = match ev {
        AgentEvent::Text(delta) => json!({"type": "text", "delta": delta}),
        AgentEvent::Reasoning(delta) => json!({"type": "reasoning", "delta": delta}),
        AgentEvent::Effort(effort) => json!({"type": "effort", "value": effort}),
        AgentEvent::ToolStart {
            id,
            name,
            args,
            risk,
        } => json!({
            "type": "tool_start",
            "id": id,
            "name": name,
            "summary": termagent_core::safety::summarize_args(args),
            "args": args,
            "risk": risk,
        }),
        AgentEvent::ToolResult {
            id,
            ok,
            output,
            duration_ms,
        } => json!({
            "type": "tool_result",
            "id": id,
            "ok": ok,
            "output": output,
            "duration_ms": duration_ms,
        }),
        AgentEvent::Approval(req) => json!({
            "type": "approval",
            "id": req.id,
            "tool": req.tool,
            "summary": req.summary,
            "args": req.args,
            "risk": req.risk,
        }),
        AgentEvent::Usage(usage) => {
            json!({"type": "usage", "input": usage.input, "output_tokens": usage.output})
        }
        AgentEvent::Done => json!({"type": "done"}),
        AgentEvent::Error(message) => json!({"type": "error", "message": message}),
    };
    Some(value)
}
