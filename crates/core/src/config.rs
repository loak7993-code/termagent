use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::registry::{Provider, Registry};
use crate::types::Effort;

pub const CONFIG_DIR_ENV: &str = "TERMAGENT_CONFIG_DIR";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: Effort,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Legacy single key, kept for -k and old config files.
    #[serde(default)]
    pub api_key: String,
    /// Per-provider keys, the normal storage path.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
    #[serde(default)]
    pub system: String,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_max_turns() -> u32 {
    24
}

fn default_theme() -> String {
    "opencode".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            provider: String::new(),
            model: String::new(),
            effort: Effort::Auto,
            base_url: None,
            api_key: String::new(),
            keys: BTreeMap::new(),
            system: String::new(),
            max_turns: default_max_turns(),
            auto_approve: false,
            theme: default_theme(),
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        if let Ok(dir) = env::var(CONFIG_DIR_ENV) {
            if !dir.trim().is_empty() {
                return PathBuf::from(dir);
            }
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("termagent")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn sessions_dir() -> PathBuf {
        Self::dir().join("sessions")
    }

    pub fn registry_cache() -> PathBuf {
        Self::dir().join("registry.json")
    }

    pub fn load() -> Result<Config> {
        let path = Self::path();
        let mut cfg = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw)
                .map_err(|e| Error::Config(format!("{}: {e}", path.display())))?
        } else {
            Config::default()
        };
        cfg.apply_env();
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::dir();
        fs::create_dir_all(&dir)?;
        let path = Self::path();
        let tmp = dir.join("config.json.tmp");
        let body = serde_json::to_vec_pretty(self)?;
        fs::write(&tmp, body)?;
        restrict(&tmp)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn apply_env(&mut self) {
        if let Ok(v) = env::var("TERMAGENT_PROVIDER") {
            if !v.is_empty() {
                self.provider = v;
            }
        }
        if let Ok(v) = env::var("TERMAGENT_MODEL") {
            if !v.is_empty() {
                self.model = v;
            }
        }
        if let Ok(v) = env::var("TERMAGENT_BASE_URL") {
            if !v.is_empty() {
                self.base_url = Some(v);
            }
        }
        if let Ok(v) = env::var("TERMAGENT_EFFORT") {
            if let Ok(e) = v.parse::<Effort>() {
                self.effort = e;
            }
        }
        if let Ok(v) = env::var("TERMAGENT_THEME") {
            if !v.is_empty() {
                self.theme = v;
            }
        }
        if let Ok(v) = env::var("TERMAGENT_AUTO_APPROVE") {
            self.auto_approve = matches!(v.as_str(), "1" | "true" | "yes");
        }
    }

    pub fn is_configured(&self, registry: &Registry) -> bool {
        if self.provider.is_empty() || self.model.is_empty() {
            return false;
        }
        match registry.provider(&self.provider) {
            Some(p) => self.key_for(p).is_some(),
            None => !self.api_key.is_empty(),
        }
    }

    /// A real credential for this provider: stored key, the legacy key for the
    /// active provider, or one of the provider's environment variables.
    pub fn direct_key(&self, provider: &Provider) -> Option<String> {
        if let Some(key) = self.keys.get(&provider.id) {
            if !key.trim().is_empty() {
                return Some(key.clone());
            }
        }
        if self.provider == provider.id && !self.api_key.is_empty() {
            return Some(self.api_key.clone());
        }
        if self.provider == provider.id {
            if let Ok(v) = env::var("TERMAGENT_API_KEY") {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        for var in &provider.env {
            if let Ok(v) = env::var(var) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Key used for requests. Local servers need no real credential.
    pub fn key_for(&self, provider: &Provider) -> Option<String> {
        self.direct_key(provider).or_else(|| {
            if provider.api.contains("127.0.0.1") || provider.api.contains("localhost") {
                Some("local".to_string())
            } else {
                None
            }
        })
    }

    pub fn system_prompt(&self) -> String {
        if !self.system.trim().is_empty() {
            return self.system.clone();
        }
        default_system_prompt()
    }
}

pub fn default_system_prompt() -> String {
    [
        "You are termagent, a coding agent running in the user's terminal and browser.",
        "You work inside one workspace directory and never assume network or root access.",
        "",
        "Rules:",
        "- Inspect before acting: read files, list directories, search instead of guessing.",
        "- Prefer the smallest edit that solves the task. Match the surrounding style.",
        "- Never claim a command succeeded; run it and report what happened.",
        "- Never use sudo, su, doas or pkexec. Everything you need is in user space.",
        "- Do not write outside the workspace.",
        "- Keep answers short. The user sees a terminal, not a document.",
        "- Use the todo tool when a task has more than three steps.",
        "- When you finish, state what changed and what you verified.",
    ]
    .join("\n")
}

#[cfg(unix)]
fn restrict(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_path: &std::path::Path) -> Result<()> {
    Ok(())
}
