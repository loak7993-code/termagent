use termagent_core::config::Config;
use termagent_core::registry::{Model, Provider, Registry};
use termagent_core::types::Effort;

use crate::input::TextInput;

pub const POPULAR: &[&str] = &[
    "anthropic",
    "openai",
    "google",
    "openrouter",
    "opencode",
    "deepseek",
    "groq",
    "mistral",
    "xai",
    "alibaba",
    "moonshotai",
    "zhipuai",
    "cerebras",
    "lmstudio",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Provider,
    Key,
    Model,
    Effort,
}

pub enum Action {
    Saved(Config),
    Cancel,
}

pub struct Onboarding {
    pub step: Step,
    pub query: TextInput,
    pub key: TextInput,
    pub sel: usize,
    pub offset: usize,
    pub provider: Option<Provider>,
    pub model: Option<Model>,
    pub effort_sel: usize,
    pub env_found: Option<String>,
    pub notice: Option<String>,
    pub key_value: String,
    pub reconfigure: bool,
}

impl Onboarding {
    pub fn new(_cfg: &Config) -> Self {
        Onboarding {
            step: Step::Provider,
            query: TextInput::default(),
            key: TextInput::default(),
            sel: 0,
            offset: 0,
            provider: None,
            model: None,
            effort_sel: 0,
            env_found: None,
            notice: None,
            key_value: String::new(),
            reconfigure: false,
        }
    }

    pub fn resume(cfg: &Config, registry: &Registry) -> Self {
        let mut ob = Self::new(cfg);
        let Some(provider) = registry.provider(&cfg.provider) else {
            return ob;
        };
        ob.provider = Some(provider.clone());
        ob.env_found = detected_env(provider);
        if cfg.key_for(provider).is_none() {
            ob.step = Step::Key;
            return ob;
        }
        if let Some(model) = provider.models.iter().find(|m| m.id == cfg.model) {
            ob.model = Some(model.clone());
            ob.effort_sel = effort_index(cfg.effort);
            ob.step = Step::Effort;
        } else {
            ob.step = Step::Model;
        }
        ob
    }

    /// Start at the provider list for an explicit `--onboard`, even when a
    /// half-configured file exists, with the current provider preselected.
    pub fn fresh(cfg: &Config, registry: &Registry) -> Self {
        let mut ob = Self::new(cfg);
        ob.reconfigure = true;
        if let Some(provider) = registry.provider(&cfg.provider) {
            ob.provider = Some(provider.clone());
            let list = ob.providers(registry);
            ob.sel = list
                .iter()
                .position(|entry| entry.id == provider.id)
                .unwrap_or(0);
        }
        ob
    }

    pub fn providers<'a>(&self, registry: &'a Registry) -> Vec<&'a Provider> {
        let query = self.query.value().to_lowercase();
        let mut list: Vec<&Provider> = registry
            .providers
            .iter()
            .filter(|p| {
                query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.id.to_lowercase().contains(&query)
                    || p.models
                        .iter()
                        .any(|m| m.id.to_lowercase().contains(&query))
            })
            .collect();
        list.sort_by_key(|p| {
            POPULAR
                .iter()
                .position(|id| *id == p.id)
                .unwrap_or(usize::MAX)
        });
        list
    }

    pub fn models(&self) -> Vec<&Model> {
        let Some(provider) = &self.provider else {
            return Vec::new();
        };
        let query = self.query.value().to_lowercase();
        provider
            .models
            .iter()
            .filter(|m| {
                query.is_empty()
                    || m.name.to_lowercase().contains(&query)
                    || m.id.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn move_sel(&mut self, delta: i32, len: usize) {
        if len == 0 {
            self.sel = 0;
            return;
        }
        let len_i = len as i32;
        let next = (self.sel as i32 + delta).rem_euclid(len_i);
        self.sel = next as usize;
    }

    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        cfg: &Config,
        registry: &Registry,
    ) -> Option<Action> {
        use crossterm::event::{KeyCode, KeyModifiers};
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match self.step {
            Step::Provider => {
                let list_len = self.providers(registry).len();
                match key.code {
                    KeyCode::Esc => return Some(Action::Cancel),
                    KeyCode::Up => self.move_sel(-1, list_len),
                    KeyCode::Down => self.move_sel(1, list_len),
                    KeyCode::PageUp => self.move_sel(-10, list_len),
                    KeyCode::PageDown => self.move_sel(10, list_len),
                    KeyCode::Enter => {
                        let list = self.providers(registry);
                        if let Some(p) = list.get(self.sel).cloned() {
                            self.provider = Some(p.clone());
                            self.env_found = p
                                .env
                                .iter()
                                .find(|v| std::env::var(v).map(|s| !s.is_empty()).unwrap_or(false))
                                .cloned();
                            self.query.clear();
                            self.sel = 0;
                            self.step = Step::Key;
                        }
                    }
                    KeyCode::Backspace => {
                        self.query.backspace();
                        self.sel = 0;
                    }
                    KeyCode::Char(c) if !ctrl => {
                        self.query.insert(c);
                        self.sel = 0;
                    }
                    _ => {}
                }
                None
            }
            Step::Key => {
                match key.code {
                    KeyCode::Esc => {
                        self.step = Step::Provider;
                        self.key.clear();
                        self.key_value.clear();
                    }
                    KeyCode::Enter => {
                        let typed = self.key.value();
                        let has_env = self.env_found.is_some();
                        let local = self
                            .provider
                            .as_ref()
                            .map(|p| p.api.contains("127.0.0.1") || p.api.contains("localhost"))
                            .unwrap_or(false);
                        if typed.trim().is_empty() && !has_env && !local {
                            self.notice = Some(format!(
                                "paste a key{}",
                                self.provider
                                    .as_ref()
                                    .and_then(|p| p.env.first())
                                    .map(|v| format!(" (or set {v})"))
                                    .unwrap_or_default()
                            ));
                            return None;
                        }
                        self.key_value = typed.trim().to_string();
                        self.key.clear();
                        self.query.clear();
                        self.sel = 0;
                        self.notice = None;
                        self.step = Step::Model;
                    }
                    KeyCode::Backspace => self.key.backspace(),
                    KeyCode::Delete => self.key.delete(),
                    KeyCode::Left => self.key.left(),
                    KeyCode::Right => self.key.right(),
                    KeyCode::Home => self.key.home(),
                    KeyCode::End => self.key.end(),
                    KeyCode::Char('v') if ctrl => {}
                    KeyCode::Char(c) if !ctrl => self.key.insert(c),
                    _ => {}
                }
                None
            }
            Step::Model => {
                let list_len = self.models().len();
                match key.code {
                    KeyCode::Esc => {
                        self.step = Step::Key;
                        self.query.clear();
                        self.sel = 0;
                    }
                    KeyCode::Up => self.move_sel(-1, list_len),
                    KeyCode::Down => self.move_sel(1, list_len),
                    KeyCode::PageUp => self.move_sel(-10, list_len),
                    KeyCode::PageDown => self.move_sel(10, list_len),
                    KeyCode::Enter => {
                        let models = self.models();
                        if let Some(m) = models.get(self.sel).copied().cloned() {
                            self.model = Some(m);
                            self.effort_sel = 0;
                            self.query.clear();
                            self.sel = 0;
                            self.step = Step::Effort;
                        }
                    }
                    KeyCode::Backspace => {
                        self.query.backspace();
                        self.sel = 0;
                    }
                    KeyCode::Char(c) if !ctrl => {
                        self.query.insert(c);
                        self.sel = 0;
                    }
                    _ => {}
                }
                None
            }
            Step::Effort => {
                let options = self.effort_options();
                match key.code {
                    KeyCode::Esc => {
                        self.step = Step::Model;
                        self.sel = 0;
                    }
                    KeyCode::Up => {
                        self.effort_sel = self.effort_sel.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        self.effort_sel =
                            (self.effort_sel + 1).min(options.len().saturating_sub(1));
                    }
                    KeyCode::Enter => {
                        return self.finish(cfg);
                    }
                    _ => {}
                }
                None
            }
        }
    }

    pub fn effort_options(&self) -> Vec<Effort> {
        match &self.model {
            Some(m) if m.reasoning => Effort::ALL.to_vec(),
            _ => vec![Effort::Auto],
        }
    }

    pub fn chosen_effort(&self) -> Effort {
        self.effort_options()
            .get(self.effort_sel)
            .copied()
            .unwrap_or(Effort::Auto)
    }

    fn finish(&self, base: &Config) -> Option<Action> {
        let provider = self.provider.as_ref()?;
        let model = self.model.as_ref()?;
        let mut cfg = base.clone();
        cfg.provider = provider.id.clone();
        cfg.model = model.id.clone();
        cfg.effort = self.chosen_effort();
        if !self.key_value.is_empty() {
            cfg.keys.insert(provider.id.clone(), self.key_value.clone());
        }
        if cfg.save().is_err() {
            return None;
        }
        Some(Action::Saved(cfg))
    }
}

fn detected_env(provider: &Provider) -> Option<String> {
    provider
        .env
        .iter()
        .find(|var| std::env::var(var).map(|v| !v.is_empty()).unwrap_or(false))
        .cloned()
}

pub fn effort_index(effort: Effort) -> usize {
    Effort::ALL.iter().position(|e| *e == effort).unwrap_or(0)
}
