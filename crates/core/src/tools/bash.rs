use std::process::Stdio;
use std::time::Duration;

use futures_util::future::BoxFuture;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::safety;
use crate::tools::{string_arg, Tool, ToolCtx};
use crate::types::Risk;

pub struct Bash;

const MAX_OUTPUT: usize = 60_000;
const TIMEOUT_SECS: u64 = 120;

impl Tool for Bash {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Run a shell command in the workspace. No root, no privilege escalation. Returns stdout, stderr and the exit code."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "timeout": {"type": "integer", "description": "Seconds, capped at 120."}
            },
            "required": ["command"]
        })
    }

    fn risk(&self, args: &Value) -> Risk {
        let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
        if safety::command_is_high_risk(cmd) {
            Risk::High
        } else {
            Risk::Medium
        }
    }

    fn run<'a>(&'a self, args: &'a Value, ctx: &'a ToolCtx) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let command = string_arg(args, "command")?;
            if command.trim().is_empty() {
                return Err(Error::Tool("empty command".into()));
            }
            if safety::is_privilege_escalation(&command) {
                return Err(Error::Tool(
                    "privilege escalation is disabled; run it as a normal user".into(),
                ));
            }
            let seconds = args
                .get("timeout")
                .and_then(Value::as_u64)
                .unwrap_or(TIMEOUT_SECS)
                .clamp(1, TIMEOUT_SECS);

            let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
            let mut cmd = tokio::process::Command::new(shell);
            cmd.arg("-c")
                .arg(&command)
                .current_dir(&ctx.cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            safety::scrub_env(&mut cmd);

            let child = cmd
                .spawn()
                .map_err(|e| Error::Tool(format!("spawn failed: {e}")))?;
            let result =
                tokio::time::timeout(Duration::from_secs(seconds), child.wait_with_output()).await;
            let output = match result {
                Ok(Ok(out)) => out,
                Ok(Err(e)) => return Err(Error::Tool(format!("command failed: {e}"))),
                Err(_) => {
                    return Err(Error::Tool(format!(
                        "timed out after {seconds}s; the process was killed"
                    )))
                }
            };

            let mut body = String::new();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stdout.trim().is_empty() {
                body.push_str(stdout.trim_end());
            }
            if !stderr.trim().is_empty() {
                if !body.is_empty() {
                    body.push_str("\n--- stderr ---\n");
                }
                body.push_str(stderr.trim_end());
            }
            if body.chars().count() > MAX_OUTPUT {
                let mut cut: String = body.chars().take(MAX_OUTPUT).collect();
                cut.push_str(&format!("\n… truncated, {} bytes total", body.len()));
                body = cut;
            }
            if body.is_empty() {
                body = "(no output)".into();
            }
            let code = output.status.code().unwrap_or(1);
            if !output.status.success() {
                body.push_str(&format!("\n[exit {code}]"));
            }
            Ok(body)
        })
    }
}
