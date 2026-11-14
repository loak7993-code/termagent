use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::Result;
use crate::tools::{TodoItem, TodoStatus, Tool, ToolCtx};

pub struct Todo;

impl Tool for Todo {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn description(&self) -> &'static str {
        "Read or replace the task list. Send items with status todo|doing|done; send nothing to read it."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {"type": "string"},
                            "status": {"type": "string", "enum": ["todo", "doing", "done"]}
                        },
                        "required": ["content"]
                    }
                }
            }
        })
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            {
                let mut list = ctx
                    .todos
                    .lock()
                    .map_err(|_| crate::Error::Tool("todo lock poisoned".into()))?;
                if let Some(items) = args.get("items").and_then(Value::as_array) {
                    list.clear();
                    for item in items {
                        let content = item
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if content.is_empty() {
                            continue;
                        }
                        let status = item
                            .get("status")
                            .and_then(Value::as_str)
                            .map(TodoStatus::parse)
                            .unwrap_or(TodoStatus::Todo);
                        list.push(TodoItem { content, status });
                    }
                }
                if list.is_empty() {
                    return Ok("todo list is empty".into());
                }
                let mut out = String::new();
                for item in list.iter() {
                    let mark = match item.status {
                        TodoStatus::Todo => "[ ]",
                        TodoStatus::Doing => "[~]",
                        TodoStatus::Done => "[x]",
                    };
                    out.push_str(&format!("{mark} {}\n", item.content));
                }
                Ok(out)
            }
        })
    }
}
