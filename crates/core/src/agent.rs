use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::effort;
use crate::error::{Error, Result};
use crate::providers::{self, ChatRequest, ProviderSpec};
use crate::registry::{Model, Provider, Registry};
use crate::safety;
use crate::session::{Session, Stored};
use crate::tools::{self, ApproveFn, TodoItem, Tool, ToolCtx};
use crate::types::{AgentEvent, ApprovalRequest, Effort, Message, Role, Usage};

pub struct Agent {
    cfg: Config,
    provider: Provider,
    model: Model,
    spec: ProviderSpec,
    pub session: Session,
    messages: Vec<Message>,
    tools: Vec<Box<dyn Tool>>,
    events: UnboundedSender<AgentEvent>,
    approve: ApproveFn,
    http: reqwest::Client,
    todos: Arc<Mutex<Vec<TodoItem>>>,
    totals: Usage,
}

impl Agent {
    pub fn new(
        cfg: Config,
        registry: Registry,
        session: Session,
        events: UnboundedSender<AgentEvent>,
        approve: ApproveFn,
    ) -> Result<Agent> {
        let provider = registry
            .provider(&cfg.provider)
            .cloned()
            .ok_or_else(|| Error::Config(format!("unknown provider `{}`", cfg.provider)))?;
        let model = registry
            .model(&cfg.provider, &cfg.model)
            .cloned()
            .unwrap_or_else(|| Model {
                id: cfg.model.clone(),
                name: cfg.model.clone(),
                reasoning: false,
                efforts: Vec::new(),
                tool_call: true,
                context: 0,
                cost_in: 0.0,
                cost_out: 0.0,
            });
        let spec = ProviderSpec::resolve(&provider, &cfg)?;

        let mut messages = vec![Message::system(system_prompt(&cfg, &session))];
        messages.extend(session.messages());

        Ok(Agent {
            cfg,
            provider,
            model,
            spec,
            session,
            messages,
            tools: tools::all(),
            events,
            approve,
            http: reqwest::Client::new(),
            todos: Arc::new(Mutex::new(Vec::new())),
            totals: Usage::default(),
        })
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn totals(&self) -> Usage {
        self.totals
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Point the agent at a different provider/model or effort without losing
    /// the conversation (used by the model picker in both front ends).
    pub fn reconfigure(&mut self, cfg: &Config, registry: &Registry) -> Result<()> {
        let provider = registry
            .provider(&cfg.provider)
            .cloned()
            .ok_or_else(|| Error::Config(format!("unknown provider `{}`", cfg.provider)))?;
        let model = registry
            .model(&cfg.provider, &cfg.model)
            .cloned()
            .unwrap_or_else(|| Model {
                id: cfg.model.clone(),
                name: cfg.model.clone(),
                reasoning: false,
                efforts: Vec::new(),
                tool_call: true,
                context: 0,
                cost_in: 0.0,
                cost_out: 0.0,
            });
        self.spec = ProviderSpec::resolve(&provider, cfg)?;
        self.provider = provider;
        self.model = model;
        self.cfg = cfg.clone();
        if let Some(first) = self.messages.first_mut() {
            *first = Message::system(system_prompt(&self.cfg, &self.session));
        }
        Ok(())
    }

    fn emit(&self, event: AgentEvent) {
        let _ = self.events.send(event);
    }

    fn ctx(&self) -> ToolCtx {
        ToolCtx {
            cwd: self.session.meta.cwd.clone().into(),
            root: self.session.meta.cwd.clone().into(),
            http: self.http.clone(),
            todos: self.todos.clone(),
        }
    }

    pub async fn run_turn(&mut self, text: &str, cancel: CancellationToken) -> Result<()> {
        let user = Message::user(text);
        self.messages.push(user.clone());
        self.session.append(&Stored::User {
            text: text.to_string(),
        })?;

        if self.session.meta.title == "New session" {
            let title: String = text.chars().take(64).collect();
            if !title.trim().is_empty() {
                let _ = self.session.apply_title(title.trim());
            }
        }

        let schemas = tools::schemas(&self.tools);
        let mut turn = 0u32;

        while turn < self.cfg.max_turns.max(1) {
            if cancel.is_cancelled() {
                self.emit(AgentEvent::Done);
                return Err(Error::Cancelled);
            }
            let resolved = match self.cfg.effort {
                Effort::Auto => effort::auto_level(text, turn),
                other => other,
            };
            let effective = if self.model.reasoning {
                Effort::from_efforts(&self.model.effort_values(), resolved)
            } else {
                Effort::Auto
            };
            self.emit(AgentEvent::Effort(effective));

            let request = ChatRequest {
                model: self.model.id.clone(),
                messages: self.messages.clone(),
                tools: schemas.clone(),
                effort: effective,
                reasoning: self.model.reasoning,
                max_tokens: 8192,
            };

            let tx = self.events.clone();
            let mut on_delta = |delta: crate::types::Delta| {
                if let Some(t) = delta.text {
                    let _ = tx.send(AgentEvent::Text(t));
                }
                if let Some(r) = delta.reasoning {
                    let _ = tx.send(AgentEvent::Reasoning(r));
                }
            };

            let outcome = providers::stream(&self.spec, &request, &cancel, &mut on_delta).await;

            let outcome = match outcome {
                Ok(o) => o,
                Err(Error::Cancelled) => {
                    self.emit(AgentEvent::Done);
                    return Err(Error::Cancelled);
                }
                Err(e) => {
                    self.emit(AgentEvent::Error(e.to_string()));
                    return Err(e);
                }
            };

            self.totals.add(outcome.usage);
            let _ = self.session.append(&Stored::Usage {
                input: outcome.usage.input,
                output: outcome.usage.output,
            });
            self.session.add_usage(outcome.usage);
            let _ = self.events.send(AgentEvent::Usage(self.totals));

            let assistant = Message {
                role: Role::Assistant,
                content: if outcome.text.is_empty() {
                    None
                } else {
                    Some(outcome.text.clone())
                },
                tool_calls: outcome.tool_calls.clone(),
                reasoning: if outcome.reasoning.is_empty() {
                    None
                } else {
                    Some(outcome.reasoning.clone())
                },
                ..Default::default()
            };
            self.messages.push(assistant);
            let _ = self.session.append(&Stored::Assistant {
                text: outcome.text,
                reasoning: if outcome.reasoning.is_empty() {
                    None
                } else {
                    Some(outcome.reasoning)
                },
                tool_calls: outcome.tool_calls.clone(),
            });

            if outcome.tool_calls.is_empty() {
                break;
            }

            for call in outcome.tool_calls {
                if cancel.is_cancelled() {
                    self.emit(AgentEvent::Done);
                    return Err(Error::Cancelled);
                }
                let args: Value = serde_json::from_str(&call.arguments)
                    .unwrap_or_else(|_| Value::Object(Default::default()));
                let tool = self.tools.iter().find(|t| t.name() == call.name);
                let risk = tool
                    .map(|t| t.risk(&args))
                    .unwrap_or(crate::types::Risk::Low);
                let summary = format!("{}: {}", call.name, safety::summarize_args(&args));
                self.emit(AgentEvent::ToolStart {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    args: args.clone(),
                    risk,
                });

                let needs_approval = risk != crate::types::Risk::Low
                    && (!self.cfg.auto_approve || risk == crate::types::Risk::High);
                let (ok, output, duration_ms) = if needs_approval {
                    let request = ApprovalRequest {
                        id: call.id.clone(),
                        tool: call.name.clone(),
                        summary,
                        args: args.clone(),
                        risk,
                    };
                    self.emit(AgentEvent::Approval(request.clone()));
                    let approved = (self.approve)(request).await;
                    if approved {
                        timed_tool(&self.tools, &call.name, &args, &self.ctx()).await
                    } else {
                        (
                            false,
                            "Denied by the user. Ask before retrying or choose another approach."
                                .into(),
                            0,
                        )
                    }
                } else {
                    timed_tool(&self.tools, &call.name, &args, &self.ctx()).await
                };

                self.emit(AgentEvent::ToolResult {
                    id: call.id.clone(),
                    ok,
                    output: output.clone(),
                    duration_ms,
                });
                let result =
                    Message::tool_result(call.id.clone(), call.name.clone(), output.clone());
                self.messages.push(result);
                let _ = self.session.append(&Stored::Tool {
                    id: call.id,
                    name: call.name,
                    args,
                    output,
                    ok,
                    risk,
                });
            }
            turn += 1;
        }

        if turn >= self.cfg.max_turns.max(1) {
            self.emit(AgentEvent::Error(format!(
                "stopped after {} tool rounds; send a follow-up to continue",
                self.cfg.max_turns
            )));
        }
        self.emit(AgentEvent::Done);
        Ok(())
    }
}

async fn timed_tool(
    tools_list: &[Box<dyn Tool>],
    name: &str,
    args: &Value,
    ctx: &ToolCtx,
) -> (bool, String, u64) {
    let started = std::time::Instant::now();
    let (ok, output) = run_tool(tools_list, name, args, ctx).await;
    (ok, output, started.elapsed().as_millis() as u64)
}

async fn run_tool(
    tools_list: &[Box<dyn Tool>],
    name: &str,
    args: &Value,
    ctx: &ToolCtx,
) -> (bool, String) {
    let Some(tool) = tools_list.iter().find(|t| t.name() == name) else {
        return (false, format!("unknown tool `{name}`"));
    };
    match tokio::time::timeout(Duration::from_secs(180), tool.run(args, ctx)).await {
        Ok(Ok(out)) => (true, out),
        Ok(Err(e)) => (false, e.to_string()),
        Err(_) => (false, "tool timed out after 180s".into()),
    }
}

fn system_prompt(cfg: &Config, session: &Session) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let date = chrono::Local::now().format("%Y-%m-%d");
    format!(
        "{}\n\nEnvironment:\n- workspace: {}\n- platform: {os} {arch}\n- date: {date}\n- model: {} ({})\nRun commands as the current user; privilege escalation is blocked.",
        cfg.system_prompt(),
        session.meta.cwd,
        session.meta.model,
        session.meta.provider,
    )
}
