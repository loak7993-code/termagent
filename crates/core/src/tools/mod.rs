use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::Result;
use crate::types::{ApprovalRequest, Risk};

pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod read;
pub mod todo;
pub mod webfetch;
pub mod write;

/// Front-end supplied approval handler. Resolves false on timeout or refusal.
pub type ApproveFn = Arc<dyn Fn(ApprovalRequest) -> BoxFuture<'static, bool> + Send + Sync>;

#[derive(Clone)]
pub struct ToolCtx {
    pub cwd: PathBuf,
    pub root: PathBuf,
    pub http: reqwest::Client,
    pub todos: Arc<Mutex<Vec<TodoItem>>>,
}

impl ToolCtx {
    pub fn new(root: PathBuf) -> Self {
        let cwd = root.clone();
        ToolCtx {
            cwd,
            root,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(25))
                .user_agent(format!("termagent/{}", crate::VERSION))
                .build()
                .unwrap_or_default(),
            todos: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Todo,
    Doing,
    Done,
}

impl TodoStatus {
    pub fn parse(s: &str) -> TodoStatus {
        match s.to_ascii_lowercase().as_str() {
            "doing" | "in_progress" | "active" => TodoStatus::Doing,
            "done" | "completed" => TodoStatus::Done,
            _ => TodoStatus::Todo,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TodoStatus::Todo => "todo",
            TodoStatus::Doing => "doing",
            TodoStatus::Done => "done",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;
    fn risk(&self, _args: &Value) -> Risk {
        Risk::Low
    }
    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>>;
}

pub fn all() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(read::Read),
        Box::new(write::Write),
        Box::new(edit::Edit),
        Box::new(ls::Ls),
        Box::new(glob::Glob),
        Box::new(grep::Grep),
        Box::new(bash::Bash),
        Box::new(todo::Todo),
        Box::new(webfetch::WebFetch),
    ]
}

/// OpenAI function-calling schemas for every registered tool.
pub fn schemas(tools: &[Box<dyn Tool>]) -> Value {
    Value::Array(
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.schema(),
                    }
                })
            })
            .collect(),
    )
}

pub fn string_arg(args: &Value, key: &str) -> std::result::Result<String, crate::Error> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| crate::Error::Tool(format!("missing `{key}`")))
}

pub fn bool_arg(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}
