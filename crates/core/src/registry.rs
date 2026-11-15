use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::error::{Error, Result};

pub const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const SNAPSHOT: &[u8] = include_bytes!("../assets/registry.json.gz");
const SNAPSHOT_MAX_AGE_SECS: i64 = 24 * 60 * 60;

/// Providers models.dev lists without a base URL but whose public endpoint we know.
const BASE_URLS: &[(&str, &str)] = &[
    ("anthropic", "https://api.anthropic.com/v1"),
    ("openai", "https://api.openai.com/v1"),
    ("google", "https://generativelanguage.googleapis.com/v1beta"),
    ("groq", "https://api.groq.com/openai/v1"),
    ("mistral", "https://api.mistral.ai/v1"),
    ("xai", "https://api.x.ai/v1"),
    ("cerebras", "https://api.cerebras.ai/v1"),
    ("cohere", "https://api.cohere.com/compatibility/v1"),
    ("perplexity", "https://api.perplexity.ai"),
    ("vercel", "https://ai-gateway.vercel.sh/v1"),
    ("ollama", "http://127.0.0.1:11434/v1"),
];

/// Cloud IAM providers need credentials we cannot collect as a single key.
const EXCLUDED: &[&str] = &[
    "amazon-bedrock",
    "google-vertex",
    "azure",
    "github-copilot",
    "google-vertex-anthropic",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[serde(rename = "openai")]
    OpenAi,
    Anthropic,
    Gemini,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Protocol::OpenAi => "openai",
            Protocol::Anthropic => "anthropic",
            Protocol::Gemini => "gemini",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub efforts: Vec<String>,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub context: u64,
    #[serde(default)]
    pub cost_in: f64,
    #[serde(default)]
    pub cost_out: f64,
}

impl Model {
    /// Empty means the model exposes a reasoning toggle, not effort levels.
    pub fn effort_values(&self) -> Vec<String> {
        self.efforts.clone()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub env: Vec<String>,
    pub api: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub models: Vec<Model>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Registry {
    #[serde(default)]
    pub fetched_at: i64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub providers: Vec<Provider>,
}

impl Registry {
    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn model(&self, provider: &str, model: &str) -> Option<&Model> {
        self.provider(provider)
            .and_then(|p| p.models.iter().find(|m| m.id == model))
    }

    pub fn total_models(&self) -> usize {
        self.providers.iter().map(|p| p.models.len()).sum()
    }

    pub fn is_stale(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.fetched_at == 0 || now - self.fetched_at > SNAPSHOT_MAX_AGE_SECS
    }

    /// Cache, then bundled snapshot. Never fails: a missing cache is normal.
    pub fn load_cached() -> Registry {
        if let Ok(raw) = std::fs::read(Config::registry_cache()) {
            if let Ok(reg) = serde_json::from_slice::<Registry>(&raw) {
                if !reg.providers.is_empty() {
                    return reg;
                }
            }
        }
        Self::snapshot().unwrap_or_default()
    }

    pub fn snapshot() -> Result<Registry> {
        let mut decoder = flate2::read::GzDecoder::new(SNAPSHOT);
        let mut json = String::new();
        decoder
            .read_to_string(&mut json)
            .map_err(|e| Error::Registry(format!("snapshot: {e}")))?;
        let mut reg: Registry = serde_json::from_str(&json)?;
        if reg.source.is_empty() {
            reg.source = "bundled".into();
        }
        Ok(reg)
    }

    pub async fn refresh() -> Result<Registry> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .user_agent(format!("termagent/{}", crate::VERSION))
            .build()
            .map_err(|e| Error::Http(e.to_string()))?;
        let resp = client
            .get(MODELS_DEV_URL)
            .send()
            .await
            .map_err(|e| Error::Http(format!("models.dev: {e}")))?;
        if !resp.status().is_success() {
            return Err(Error::api(
                resp.status().as_u16(),
                "models.dev rejected the request",
            ));
        }
        let body = resp.text().await.map_err(|e| Error::Http(e.to_string()))?;
        let mut reg = Self::from_models_dev(&body)?;
        reg.fetched_at = chrono::Utc::now().timestamp();
        reg.source = "models.dev".into();
        reg.write_cache();
        Ok(reg)
    }

    fn write_cache(&self) {
        let dir = Config::dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let path = Config::registry_cache();
        let tmp = dir.join("registry.json.tmp");
        if let Ok(bytes) = serde_json::to_vec(self) {
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    /// Turn the models.dev document into the trimmed shape termagent uses.
    pub fn from_models_dev(body: &str) -> Result<Registry> {
        let root: Value = serde_json::from_str(body)?;
        let obj = root
            .as_object()
            .ok_or_else(|| Error::Registry("models.dev payload is not an object".into()))?;

        let mut providers = Vec::new();
        for (id, raw) in obj {
            if EXCLUDED.contains(&id.as_str()) {
                continue;
            }
            let api = raw
                .get("api")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    BASE_URLS
                        .iter()
                        .find(|(pid, _)| *pid == id)
                        .map(|(_, url)| (*url).to_string())
                });
            let Some(api) = api else { continue };

            let protocol = match id.as_str() {
                "anthropic" => Protocol::Anthropic,
                "google" => Protocol::Gemini,
                _ => Protocol::OpenAi,
            };

            let mut models = Vec::new();
            if let Some(map) = raw.get("models").and_then(Value::as_object) {
                for (mid, m) in map {
                    if m.get("tool_call").and_then(Value::as_bool) != Some(true) {
                        continue;
                    }
                    let outputs_text = m
                        .pointer("/modalities/output")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().any(|v| v.as_str() == Some("text")))
                        .unwrap_or(true);
                    if !outputs_text {
                        continue;
                    }
                    let efforts = m
                        .get("reasoning_options")
                        .and_then(Value::as_array)
                        .and_then(|opts| {
                            opts.iter()
                                .find(|o| o.get("type").and_then(Value::as_str) == Some("effort"))
                                .and_then(|o| o.get("values"))
                                .and_then(Value::as_array)
                                .map(|vals| {
                                    vals.iter()
                                        .filter_map(Value::as_str)
                                        .map(str::to_string)
                                        .collect::<Vec<_>>()
                                })
                        })
                        .unwrap_or_default();
                    models.push(Model {
                        id: mid.clone(),
                        name: m
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or(mid)
                            .to_string(),
                        reasoning: m.get("reasoning").and_then(Value::as_bool).unwrap_or(false),
                        efforts,
                        tool_call: true,
                        context: m
                            .pointer("/limit/context")
                            .and_then(Value::as_u64)
                            .unwrap_or(0),
                        cost_in: m
                            .pointer("/cost/input")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                        cost_out: m
                            .pointer("/cost/output")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                    });
                }
            }
            if models.is_empty() {
                continue;
            }

            providers.push(Provider {
                id: id.clone(),
                name: raw
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_string(),
                env: raw
                    .get("env")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                api,
                protocol,
                models,
            });
        }

        providers.sort_by_key(|p| p.name.to_lowercase());
        Ok(Registry {
            fetched_at: 0,
            source: "models.dev".into(),
            providers,
        })
    }

    /// Providers whose key is available (config, env, or local endpoint), first.
    pub fn for_config(&self, cfg: &Config) -> Vec<Provider> {
        let mut configured = Vec::new();
        let mut rest = Vec::new();
        for p in &self.providers {
            if cfg.key_for(p).is_some() {
                configured.push(p.clone());
            } else {
                rest.push(p.clone());
            }
        }
        configured.extend(rest);
        configured
    }

    pub fn configured(&self, cfg: &Config) -> Vec<Provider> {
        self.providers
            .iter()
            .filter(|p| cfg.key_for(p).is_some())
            .cloned()
            .collect()
    }

    /// Compact search index: one row per model, provider id + name included.
    pub fn model_index(&self) -> Vec<IndexRow> {
        let mut rows = Vec::with_capacity(self.total_models());
        for p in &self.providers {
            for m in &p.models {
                rows.push(IndexRow {
                    provider: p.id.clone(),
                    provider_name: p.name.clone(),
                    id: m.id.clone(),
                    name: m.name.clone(),
                    reasoning: m.reasoning,
                    context: m.context,
                    cost_in: m.cost_in,
                    cost_out: m.cost_out,
                });
            }
        }
        rows
    }

    pub fn group_counts(&self) -> BTreeMap<String, usize> {
        self.providers
            .iter()
            .map(|p| (p.id.clone(), p.models.len()))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexRow {
    pub provider: String,
    pub provider_name: String,
    pub id: String,
    pub name: String,
    pub reasoning: bool,
    pub context: u64,
    pub cost_in: f64,
    pub cost_out: f64,
}
