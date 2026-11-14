use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::types::{Effort, Message, Risk, Role, ToolCall, Usage};

pub const META_KIND: &str = "meta";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub created: i64,
    pub cwd: String,
    pub provider: String,
    pub model: String,
    pub effort: Effort,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Totals {
    pub input: u64,
    pub output: u64,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub meta: SessionMeta,
    pub path: PathBuf,
    pub events: Vec<Stored>,
    pub totals: Totals,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Stored {
    Meta(SessionMeta),
    Title {
        title: String,
    },
    User {
        text: String,
    },
    Assistant {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        id: String,
        name: String,
        args: serde_json::Value,
        output: String,
        ok: bool,
        risk: Risk,
    },
    Usage {
        input: u64,
        output: u64,
    },
}

impl Session {
    pub fn new(cwd: &Path, provider: &str, model: &str, effort: Effort) -> Result<Session> {
        let dir = Config::sessions_dir();
        fs::create_dir_all(&dir)?;
        let now = chrono::Local::now();
        let id = format!(
            "{}-{}",
            now.format("%Y%m%d-%H%M%S"),
            &uuid::Uuid::new_v4().simple().to_string()[..6]
        );
        let meta = SessionMeta {
            id: id.clone(),
            title: "New session".into(),
            created: chrono::Utc::now().timestamp(),
            cwd: cwd.display().to_string(),
            provider: provider.into(),
            model: model.into(),
            effort,
        };
        let path = dir.join(format!("{id}.jsonl"));
        let session = Session {
            meta,
            path,
            events: Vec::new(),
            totals: Totals::default(),
        };
        session.append(&Stored::Meta(session.meta.clone()))?;
        Ok(session)
    }

    pub fn open(id: &str) -> Result<Session> {
        if !valid_id(id) {
            return Err(Error::Config("invalid session id".into()));
        }
        let path = Config::sessions_dir().join(format!("{id}.jsonl"));
        Self::from_path(path)
    }

    fn from_path(path: PathBuf) -> Result<Session> {
        let raw = fs::read_to_string(&path)?;
        let mut meta: Option<SessionMeta> = None;
        let mut totals = Totals::default();
        let mut title = String::new();
        let mut stored = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Stored>(line) else {
                continue;
            };
            match &ev {
                Stored::Meta(m) => meta = Some(m.clone()),
                Stored::Title { title: t } => title = t.clone(),
                Stored::Usage { input, output } => {
                    totals.input += input;
                    totals.output += output;
                }
                _ => {}
            }
            stored.push(ev);
        }
        let mut meta = meta.ok_or_else(|| Error::Config("session has no meta line".into()))?;
        if !title.is_empty() {
            meta.title = title;
        }
        Ok(Session {
            meta,
            path,
            events: stored,
            totals,
        })
    }

    pub fn append(&self, ev: &Stored) -> Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let mut line = serde_json::to_string(ev)?;
        line.push('\n');
        f.write_all(line.as_bytes())?;
        Ok(())
    }

    /// Writes meta + every event to a fresh file (used by tests and imports).
    pub fn rewrite(&self) -> Result<()> {
        let mut body = String::new();
        for ev in &self.events {
            body.push_str(&serde_json::to_string(ev)?);
            body.push('\n');
        }
        fs::write(&self.path, body)?;
        Ok(())
    }

    pub fn messages(&self) -> Vec<Message> {
        let mut out = Vec::new();
        for ev in &self.events {
            match ev {
                Stored::User { text } => out.push(Message::user(text.clone())),
                Stored::Assistant {
                    text,
                    reasoning,
                    tool_calls,
                } => out.push(Message {
                    role: Role::Assistant,
                    content: if text.is_empty() {
                        None
                    } else {
                        Some(text.clone())
                    },
                    tool_calls: tool_calls.clone(),
                    reasoning: reasoning.clone(),
                    ..Default::default()
                }),
                Stored::Tool {
                    id, name, output, ..
                } => out.push(Message::tool_result(
                    id.clone(),
                    name.clone(),
                    output.clone(),
                )),
                _ => {}
            }
        }
        out
    }

    pub fn apply_title(&mut self, title: &str) -> Result<()> {
        self.meta.title = title.to_string();
        self.events.push(Stored::Title {
            title: title.to_string(),
        });
        self.append(&Stored::Title {
            title: title.to_string(),
        })
    }

    pub fn add_usage(&mut self, usage: Usage) {
        self.totals.input += usage.input;
        self.totals.output += usage.output;
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn list() -> Vec<SessionMeta> {
    let dir = Config::sessions_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_suffix(".jsonl") else {
            continue;
        };
        if !valid_id(id) {
            continue;
        }
        if let Ok(session) = Session::open(id) {
            out.push(session.meta);
        }
    }
    out.sort_by_key(|m| std::cmp::Reverse(m.created));
    out
}

pub fn delete(id: &str) -> Result<()> {
    if !valid_id(id) {
        return Err(Error::Config("invalid session id".into()));
    }
    fs::remove_file(Config::sessions_dir().join(format!("{id}.jsonl")))?;
    Ok(())
}

pub fn rename(id: &str, title: &str) -> Result<()> {
    let mut session = Session::open(id)?;
    session.apply_title(title.trim())?;
    Ok(())
}
