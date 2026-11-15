use std::sync::mpsc::Receiver;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use termagent_core::session::{self, Session, SessionMeta, Stored};
use termagent_core::types::{AgentEvent, ApprovalRequest, Effort, Risk, Usage};
use termagent_core::{Config, Registry};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::input::TextInput;
use crate::onboarding::{effort_index, Action, Onboarding};
use crate::theme::{self, Palette};
use crate::worker::{Approvals, Command, TuiEvent};

pub enum Screen {
    Onboarding,
    Chat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Streaming,
    Waiting,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Live {
    None,
    Assistant,
    Reasoning,
}

pub struct ToolCard {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub risk: Risk,
    pub running: bool,
    pub ok: bool,
    pub output: String,
    pub elapsed_ms: u64,
}

pub enum Item {
    User(String),
    Assistant(String),
    Reasoning(String),
    Tool(ToolCard),
    Error(String),
    Info(String),
}

pub struct ChatState {
    pub items: Vec<Item>,
    pub live: Live,
    pub pending: Option<ApprovalRequest>,
    pub status: Status,
    pub usage: Usage,
    pub effort: Effort,
    pub resolved: Option<Effort>,
    pub session: Option<SessionMeta>,
    pub scroll_back: u16,
    pub notice: Option<(String, Instant)>,
}

impl ChatState {
    pub fn new(effort: Effort) -> Self {
        ChatState {
            items: Vec::new(),
            live: Live::None,
            pending: None,
            status: Status::Idle,
            usage: Usage::default(),
            effort,
            resolved: None,
            session: None,
            scroll_back: 0,
            notice: None,
        }
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    pub fn message(&mut self, item: Item) {
        self.items.push(item);
        self.scroll_back = 0;
    }
}

pub enum Pane {
    Providers,
    Models,
}

pub struct ModelPicker {
    pub pane: Pane,
    pub provider_sel: usize,
    pub model_sel: usize,
    pub provider_query: TextInput,
    pub model_query: TextInput,
    pub provider_offset: usize,
    pub model_offset: usize,
}

pub struct SessionPicker {
    pub items: Vec<SessionMeta>,
    pub sel: usize,
    pub offset: usize,
    pub confirm: Option<String>,
}

pub enum Overlay {
    None,
    Models(ModelPicker),
    Sessions(SessionPicker),
    Help,
}

pub struct App {
    pub cfg: Config,
    pub registry: Registry,
    pub screen: Screen,
    pub onboarding: Option<Onboarding>,
    pub onboarding_back: bool,
    pub chat: ChatState,
    pub input: TextInput,
    pub overlay: Overlay,
    pub palette: Palette,
    pub should_quit: bool,
    pub tick: usize,
    pub commands: UnboundedSender<Command>,
    pub approvals: Approvals,
    pub events: Receiver<TuiEvent>,
    pub cancel: Option<CancellationToken>,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
    pub theme_names: Vec<&'static str>,
    initial_session: Option<String>,
}

impl App {
    pub fn new(
        cfg: Config,
        registry: Registry,
        commands: UnboundedSender<Command>,
        approvals: Approvals,
        events: Receiver<TuiEvent>,
        force_onboarding: bool,
        initial_session: Option<String>,
    ) -> App {
        let palette = theme::get(&cfg.theme);
        let chat = ChatState::new(cfg.effort);
        let configured = cfg.is_configured(&registry) && !force_onboarding;
        let onboarding = if configured {
            None
        } else if force_onboarding {
            Some(Onboarding::fresh(&cfg, &registry))
        } else {
            Some(Onboarding::resume(&cfg, &registry))
        };
        let mut app = App {
            screen: if configured {
                Screen::Chat
            } else {
                Screen::Onboarding
            },
            onboarding,
            onboarding_back: false,
            chat,
            input: TextInput::default(),
            overlay: Overlay::None,
            palette,
            should_quit: false,
            tick: 0,
            cfg,
            registry,
            commands,
            approvals,
            events,
            cancel: None,
            history: Vec::new(),
            history_idx: None,
            theme_names: theme::names(),
            initial_session,
        };
        if configured {
            app.start_chat();
        }
        app
    }

    fn start_chat(&mut self) {
        self.chat = ChatState::new(self.cfg.effort);
        self.screen = Screen::Chat;
        let session = self.initial_session.take();
        let _ = self
            .commands
            .send(Command::Start(self.cfg.clone(), session));
    }

    pub fn poll_events(&mut self) {
        while let Ok(ev) = self.events.try_recv() {
            self.handle_tui_event(ev);
        }
    }

    fn handle_tui_event(&mut self, ev: TuiEvent) {
        match ev {
            TuiEvent::Agent(agent) => self.handle_agent_event(agent),
            TuiEvent::Session(meta) => {
                let changed = self
                    .chat
                    .session
                    .as_ref()
                    .map(|current| current.id != meta.id)
                    .unwrap_or(true);
                self.chat.session = Some(meta.clone());
                if changed || self.chat.items.is_empty() {
                    self.hydrate(&meta);
                }
            }
            TuiEvent::Info(msg) => {
                self.chat.message(Item::Info(msg));
            }
        }
    }

    fn hydrate(&mut self, meta: &SessionMeta) {
        let Ok(session) = Session::open(&meta.id) else {
            return;
        };
        let events = session.events;
        if events.is_empty() {
            return;
        }
        self.chat.items.clear();
        self.chat.live = Live::None;
        self.chat.usage = termagent_core::types::Usage::default();
        for event in events {
            match event {
                Stored::User { text } => self.chat.items.push(Item::User(text)),
                Stored::Assistant {
                    text,
                    reasoning,
                    tool_calls,
                } => {
                    if let Some(reasoning) = reasoning {
                        if !reasoning.trim().is_empty() {
                            self.chat.items.push(Item::Reasoning(reasoning));
                        }
                    }
                    if !text.trim().is_empty() {
                        self.chat.items.push(Item::Assistant(text));
                    }
                    let _ = tool_calls;
                }
                Stored::Tool {
                    id,
                    name,
                    args,
                    output,
                    ok,
                    risk,
                } => self.chat.items.push(Item::Tool(ToolCard {
                    id,
                    name,
                    summary: termagent_core::safety::summarize_args(&args),
                    risk,
                    running: false,
                    ok,
                    output,
                    elapsed_ms: 0,
                })),
                Stored::Usage { input, output } => {
                    self.chat.usage.input += input;
                    self.chat.usage.output += output;
                }
                Stored::Meta(_) | Stored::Title { .. } => {}
            }
        }
        self.chat.scroll_back = 0;
    }

    fn handle_agent_event(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::Text(text) => {
                self.chat.status = Status::Streaming;
                match self.chat.items.last_mut() {
                    Some(Item::Assistant(buf)) if self.chat.live == Live::Assistant => {
                        buf.push_str(&text)
                    }
                    _ => {
                        self.chat.message(Item::Assistant(text));
                        self.chat.live = Live::Assistant;
                    }
                }
            }
            AgentEvent::Reasoning(text) => match self.chat.items.last_mut() {
                Some(Item::Reasoning(buf)) if self.chat.live == Live::Reasoning => {
                    buf.push_str(&text)
                }
                _ => {
                    self.chat.message(Item::Reasoning(text));
                    self.chat.live = Live::Reasoning;
                }
            },
            AgentEvent::Effort(effort) => {
                self.chat.resolved = Some(effort);
            }
            AgentEvent::ToolStart {
                id,
                name,
                args,
                risk,
            } => {
                self.chat.live = Live::None;
                self.chat.message(Item::Tool(ToolCard {
                    id,
                    name,
                    summary: termagent_core::safety::summarize_args(&args),
                    risk,
                    running: true,
                    ok: true,
                    output: String::new(),
                    elapsed_ms: 0,
                }));
            }
            AgentEvent::ToolResult {
                id,
                ok,
                output,
                duration_ms,
            } => {
                if let Some(Item::Tool(card)) = self
                    .chat
                    .items
                    .iter_mut()
                    .rev()
                    .find(|i| matches!(i, Item::Tool(c) if c.id == id))
                {
                    card.running = false;
                    card.ok = ok;
                    card.elapsed_ms = duration_ms;
                    card.output = output;
                }
            }
            AgentEvent::Approval(req) => {
                self.chat.pending = Some(req);
                self.chat.status = Status::Waiting;
            }
            AgentEvent::Usage(usage) => self.chat.usage = usage,
            AgentEvent::Done => {
                self.chat.status = Status::Idle;
                self.chat.live = Live::None;
                if self.chat.pending.is_some() {
                    self.chat.pending = None;
                }
                self.cancel = None;
            }
            AgentEvent::Error(msg) => {
                self.chat.status = Status::Idle;
                self.chat.live = Live::None;
                self.chat.pending = None;
                self.chat.message(Item::Error(msg));
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if self.chat.pending.is_some() {
            self.handle_approval_key(key);
            return;
        }
        if ctrl && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if ctrl && key.code == KeyCode::Char('t') {
            self.cycle_theme();
            return;
        }
        if ctrl && key.code == KeyCode::Char('e') {
            self.cycle_effort();
            return;
        }
        if ctrl && key.code == KeyCode::Char('k') {
            self.open_models();
            return;
        }
        if ctrl && key.code == KeyCode::Char('s') {
            self.open_sessions();
            return;
        }
        if ctrl && key.code == KeyCode::F(1) {
            self.overlay = Overlay::Help;
            return;
        }

        match std::mem::replace(&mut self.overlay, Overlay::None) {
            Overlay::Models(mut picker) => {
                if self.handle_model_picker(&mut picker, key) {
                    self.overlay = Overlay::Models(picker);
                } else {
                    self.overlay = Overlay::None;
                }
                return;
            }
            Overlay::Sessions(mut picker) => {
                if self.handle_session_picker(&mut picker, key) {
                    self.overlay = Overlay::Sessions(picker);
                } else {
                    self.overlay = Overlay::None;
                }
                return;
            }
            Overlay::Help => {
                if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                    self.overlay = Overlay::None;
                } else {
                    self.overlay = Overlay::Help;
                }
                return;
            }
            Overlay::None => {}
        }

        if matches!(self.screen, Screen::Onboarding) {
            self.handle_onboarding_key(key);
            return;
        }

        match key.code {
            KeyCode::Esc => {
                if self.chat.status == Status::Streaming {
                    if let Some(token) = &self.cancel {
                        token.cancel();
                        self.chat.notice("stopping…");
                    }
                } else {
                    self.input.clear();
                }
            }
            KeyCode::Enter => {
                if self.chat.status != Status::Idle {
                    return;
                }
                let text = self.input.value();
                if text.trim().is_empty() {
                    return;
                }
                self.input.clear();
                self.history_idx = None;
                self.history.push(text.clone());
                if self.handle_slash(text.trim()) {
                    return;
                }
                self.send_turn(text.trim().to_string());
            }
            KeyCode::Char(c) if !ctrl && !alt => self.input.insert(c),
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Delete => self.input.delete(),
            KeyCode::Left => self.input.left(),
            KeyCode::Right => self.input.right(),
            KeyCode::Home => self.input.home(),
            KeyCode::End => self.input.end(),
            KeyCode::Up => {
                if self.chat.scroll_back < 5000 {
                    self.chat.scroll_back += 3;
                }
            }
            KeyCode::Down => {
                self.chat.scroll_back = self.chat.scroll_back.saturating_sub(3);
            }
            KeyCode::PageUp => self.chat.scroll_back = self.chat.scroll_back.saturating_add(12),
            KeyCode::PageDown => self.chat.scroll_back = self.chat.scroll_back.saturating_sub(12),
            KeyCode::Tab => self.chat.scroll_back = 0,
            KeyCode::F(1) => self.overlay = Overlay::Help,
            _ => {}
        }
    }

    fn handle_approval_key(&mut self, key: KeyEvent) {
        let Some(req) = self.chat.pending.clone() else {
            return;
        };
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.approvals.answer(&req.id, true);
                self.chat.pending = None;
                self.chat.status = Status::Streaming;
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.cfg.auto_approve = true;
                let _ = self.cfg.save();
                self.approvals.answer(&req.id, true);
                self.chat.pending = None;
                self.chat.status = Status::Streaming;
                self.chat.notice("auto-approve on for medium risks");
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.approvals.answer(&req.id, false);
                self.chat.pending = None;
                self.chat.status = Status::Streaming;
            }
            _ => {}
        }
    }

    fn send_turn(&mut self, text: String) {
        self.chat.message(Item::User(text.clone()));
        self.chat.status = Status::Streaming;
        self.chat.live = Live::None;
        let token = CancellationToken::new();
        self.cancel = Some(token.clone());
        let _ = self.commands.send(Command::Turn(text, token));
    }

    fn handle_onboarding_key(&mut self, key: KeyEvent) {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        let action = onboarding.handle_key(key, &self.cfg, &self.registry);
        match action {
            Some(Action::Saved(cfg)) => {
                self.cfg = cfg;
                self.palette = theme::get(&self.cfg.theme);
                self.onboarding = None;
                self.onboarding_back = false;
                self.start_chat();
            }
            Some(Action::Cancel) => {
                if self.onboarding_back {
                    self.onboarding = None;
                    self.onboarding_back = false;
                    self.screen = Screen::Chat;
                } else {
                    self.should_quit = true;
                }
            }
            None => {}
        }
    }

    fn open_models(&mut self) {
        if !matches!(self.screen, Screen::Chat) {
            return;
        }
        let providers = self.registry.for_config(&self.cfg);
        let provider_sel = providers
            .iter()
            .position(|p| p.id == self.cfg.provider)
            .unwrap_or(0);
        let mut picker = ModelPicker {
            pane: if providers.is_empty() {
                Pane::Providers
            } else {
                Pane::Models
            },
            provider_sel,
            model_sel: 0,
            provider_query: TextInput::default(),
            model_query: TextInput::default(),
            provider_offset: 0,
            model_offset: 0,
        };
        if let Some(provider) = providers.get(picker.provider_sel) {
            picker.model_sel = provider
                .models
                .iter()
                .position(|m| m.id == self.cfg.model)
                .unwrap_or(0);
        }
        self.overlay = Overlay::Models(picker);
    }

    fn open_sessions(&mut self) {
        if !matches!(self.screen, Screen::Chat) {
            return;
        }
        let items = session::list();
        self.overlay = Overlay::Sessions(SessionPicker {
            items,
            sel: 0,
            offset: 0,
            confirm: None,
        });
    }

    fn handle_model_picker(&mut self, picker: &mut ModelPicker, key: KeyEvent) -> bool {
        let providers = self.registry.for_config(&self.cfg);
        match key.code {
            KeyCode::Esc => false,
            KeyCode::Tab => {
                picker.pane = match picker.pane {
                    Pane::Providers => Pane::Models,
                    Pane::Models => Pane::Providers,
                };
                true
            }
            KeyCode::Up => {
                if matches!(picker.pane, Pane::Providers) {
                    picker.provider_sel = picker.provider_sel.saturating_sub(1);
                } else {
                    picker.model_sel = picker.model_sel.saturating_sub(1);
                }
                true
            }
            KeyCode::Down => {
                if matches!(picker.pane, Pane::Providers) {
                    picker.provider_sel =
                        (picker.provider_sel + 1).min(providers.len().saturating_sub(1));
                } else {
                    let len = self.picker_model_list(picker).len();
                    picker.model_sel = (picker.model_sel + 1).min(len.saturating_sub(1));
                }
                true
            }
            KeyCode::PageUp | KeyCode::PageDown => true,
            KeyCode::Enter => {
                if matches!(picker.pane, Pane::Providers) {
                    picker.model_sel = 0;
                    picker.model_query.clear();
                    picker.pane = Pane::Models;
                    true
                } else {
                    let models = self.picker_model_list(picker);
                    let model = models.get(picker.model_sel).cloned();
                    let provider = providers.get(picker.provider_sel).map(|p| p.id.clone());
                    if let (Some(model), Some(provider)) = (model, provider) {
                        if self.cfg.provider != provider {
                            if provider != "custom" {
                                self.cfg.base_url = None;
                            }
                            self.cfg.api_key.clear();
                        }
                        self.cfg.provider = provider;
                        self.cfg.model = model.id.clone();
                        if !model.reasoning {
                            self.cfg.effort = Effort::Auto;
                        }
                        let _ = self.cfg.save();
                        let _ = self.commands.send(Command::Reconfigure);
                        self.chat.notice(format!("model: {}", model.name));
                    }
                    false
                }
            }
            KeyCode::Backspace => {
                if matches!(picker.pane, Pane::Providers) {
                    picker.provider_query.backspace();
                    picker.provider_sel = 0;
                } else {
                    picker.model_query.backspace();
                    picker.model_sel = 0;
                }
                true
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if matches!(picker.pane, Pane::Providers) {
                    picker.provider_query.insert(c);
                    picker.provider_sel = 0;
                } else {
                    picker.model_query.insert(c);
                    picker.model_sel = 0;
                }
                true
            }
            _ => true,
        }
    }

    pub fn picker_model_list(&self, picker: &ModelPicker) -> Vec<termagent_core::Model> {
        let providers = self.registry.for_config(&self.cfg);
        let Some(provider) = providers.get(picker.provider_sel) else {
            return Vec::new();
        };
        let query = picker.model_query.value().to_lowercase();
        provider
            .models
            .iter()
            .filter(|m| {
                query.is_empty()
                    || m.name.to_lowercase().contains(&query)
                    || m.id.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    pub fn picker_provider_list(&self, picker: &ModelPicker) -> Vec<termagent_core::Provider> {
        let query = picker.provider_query.value().to_lowercase();
        self.registry
            .for_config(&self.cfg)
            .into_iter()
            .filter(|p| {
                query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.id.to_lowercase().contains(&query)
                    || p.models
                        .iter()
                        .any(|m| m.id.to_lowercase().contains(&query))
            })
            .collect()
    }

    fn handle_session_picker(&mut self, picker: &mut SessionPicker, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => false,
            KeyCode::Up => {
                picker.sel = picker.sel.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                picker.sel = (picker.sel + 1).min(picker.items.len().saturating_sub(1));
                true
            }
            KeyCode::Enter => {
                if let Some(meta) = picker.items.get(picker.sel) {
                    let id = meta.id.clone();
                    let _ = self.commands.send(Command::LoadSession(id));
                    self.chat = ChatState::new(self.cfg.effort);
                    self.chat.notice(format!("loading {}", meta.title));
                }
                false
            }
            KeyCode::Char('n') => {
                self.chat = ChatState::new(self.cfg.effort);
                let _ = self.commands.send(Command::NewSession);
                false
            }
            KeyCode::Char('d') => {
                if let Some(meta) = picker.items.get(picker.sel) {
                    if picker.confirm.as_deref() == Some(meta.id.as_str()) {
                        let _ = session::delete(&meta.id);
                        picker.items = session::list();
                        picker.sel = picker.sel.saturating_sub(1);
                        picker.confirm = None;
                    } else {
                        picker.confirm = Some(meta.id.clone());
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn cycle_theme(&mut self) {
        let next = theme::next(self.palette.name);
        self.palette = next;
        self.cfg.theme = next.name.to_string();
        let _ = self.cfg.save();
        self.chat.notice(format!("theme: {}", next.name));
    }

    fn cycle_effort(&mut self) {
        self.cfg.effort = self.cfg.effort.cycle();
        self.chat.effort = self.cfg.effort;
        let _ = self.cfg.save();
        let _ = self.commands.send(Command::Reconfigure);
        self.chat
            .notice(format!("effort: {}", self.cfg.effort.label()));
    }

    fn handle_slash(&mut self, line: &str) -> bool {
        if !line.starts_with('/') {
            return false;
        }
        let mut parts = line.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim();
        match cmd {
            "/help" | "/?" => self.overlay = Overlay::Help,
            "/model" | "/models" => self.open_models(),
            "/sessions" => self.open_sessions(),
            "/new" | "/clear" => {
                self.chat = ChatState::new(self.cfg.effort);
                let _ = self.commands.send(Command::NewSession);
                self.chat.notice("new session");
            }
            "/effort" if arg.is_empty() => self.cycle_effort(),
            "/effort" => match arg.parse::<Effort>() {
                Ok(effort) => {
                    self.cfg.effort = effort;
                    let _ = self.cfg.save();
                    self.chat.effort = effort;
                    let _ = self.commands.send(Command::Reconfigure);
                    self.chat.notice(format!("effort: {}", effort.label()));
                }
                Err(e) => self.chat.notice(e.to_string()),
            },
            "/theme" if arg.is_empty() => self.cycle_theme(),
            "/theme" => {
                if self.theme_names.contains(&arg) {
                    self.palette = theme::get(arg);
                    self.cfg.theme = arg.to_string();
                    let _ = self.cfg.save();
                    self.chat.notice(format!("theme: {arg}"));
                } else {
                    self.chat.notice(format!("unknown theme `{arg}`"));
                }
            }
            "/config" | "/onboard" => {
                self.onboarding = Some(Onboarding::fresh(&self.cfg, &self.registry));
                self.onboarding_back = true;
                self.screen = Screen::Onboarding;
            }
            "/key" => {
                self.onboarding = Some(Onboarding::key_only(&self.cfg, &self.registry));
                self.onboarding_back = true;
                self.screen = Screen::Onboarding;
            }
            "/tools" => {
                let names: Vec<&str> = termagent_core::tools::all()
                    .iter()
                    .map(|t| t.name())
                    .collect();
                self.chat
                    .message(Item::Info(format!("tools: {}", names.join(", "))));
            }
            "/cwd" => {
                if let Some(meta) = &self.chat.session {
                    self.chat
                        .message(Item::Info(format!("workspace: {}", meta.cwd)));
                }
            }
            "/compact" => {
                self.chat.message(Item::Info(
                    "context lives in the session file; start /new for a clean window".into(),
                ));
            }
            "/exit" | "/quit" => self.should_quit = true,
            other => {
                let hint = "commands: /model /effort /key /sessions /new /theme /config /tools /cwd /help /exit";
                self.chat
                    .message(Item::Info(format!("unknown command {other}. {hint}")));
            }
        }
        true
    }

    pub fn selected_effort_index(&self) -> usize {
        effort_index(self.cfg.effort)
    }
}

impl App {
    pub fn paste(&mut self, text: &str) {
        if matches!(self.screen, Screen::Chat)
            && self.overlay_is_none()
            && self.chat.pending.is_none()
        {
            self.input.insert_str(text);
        }
    }

    fn overlay_is_none(&self) -> bool {
        matches!(self.overlay, Overlay::None)
    }

    pub fn theme_names_slice(&self) -> &[&'static str] {
        &self.theme_names
    }
}
