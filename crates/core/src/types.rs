use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    #[default]
    Auto,
    Low,
    Medium,
    High,
    Max,
}

impl Effort {
    pub const ALL: [Effort; 5] = [
        Effort::Auto,
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::Max,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Effort::Auto => "Auto",
            Effort::Low => "Low",
            Effort::Medium => "Medium",
            Effort::High => "High",
            Effort::Max => "Max",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Effort::Auto => Effort::Low,
            Effort::Low => Effort::Medium,
            Effort::Medium => Effort::High,
            Effort::High => Effort::Max,
            Effort::Max => Effort::Auto,
        }
    }

    /// Wire value for OpenAI-style `reasoning_effort`.
    pub fn api_value(self) -> &'static str {
        match self {
            Effort::Auto | Effort::Medium => "medium",
            Effort::Low => "low",
            Effort::High => "high",
            Effort::Max => "max",
        }
    }

    /// Token budget used by Anthropic thinking and Gemini thinkingConfig.
    pub fn budget(self) -> u32 {
        match self {
            Effort::Auto | Effort::Low => 1024,
            Effort::Medium => 4096,
            Effort::High => 16384,
            Effort::Max => 32768,
        }
    }

    pub fn from_efforts(values: &[String], wanted: Effort) -> Effort {
        if values.is_empty() || wanted == Effort::Auto {
            return wanted;
        }
        let name = wanted.label().to_lowercase();
        if values.iter().any(|v| v.eq_ignore_ascii_case(&name)) {
            wanted
        } else if values.iter().any(|v| v.eq_ignore_ascii_case("high")) {
            Effort::High
        } else {
            Effort::Medium
        }
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Effort {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Effort::Auto),
            "low" | "minimal" => Ok(Effort::Low),
            "medium" | "med" | "default" => Ok(Effort::Medium),
            "high" => Ok(Effort::High),
            "max" => Ok(Effort::Max),
            other => Err(Error::Config(format!("unknown effort `{other}`"))),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    #[default]
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    #[default]
    Low,
    Medium,
    High,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Low => "low",
            Risk::Medium => "medium",
            Risk::High => "high",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON text, exactly as the model produced it.
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Message {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self {
        Message {
            role: Role::System,
            content: Some(text.into()),
            ..Default::default()
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: Some(text.into()),
            ..Default::default()
        }
    }

    pub fn tool_result(
        id: impl Into<String>,
        name: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        Message {
            role: Role::Tool,
            content: Some(output.into()),
            tool_call_id: Some(id.into()),
            name: Some(name.into()),
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

impl Usage {
    pub fn add(&mut self, other: Usage) {
        self.input += other.input;
        self.output += other.output;
    }

    pub fn cost(&self, model: &crate::registry::Model) -> f64 {
        (self.input as f64 / 1_000_000.0) * model.cost_in
            + (self.output as f64 / 1_000_000.0) * model.cost_out
    }
}

#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool: String,
    pub summary: String,
    pub args: serde_json::Value,
    pub risk: Risk,
}

#[derive(Clone, Debug)]
pub enum AgentEvent {
    Text(String),
    Reasoning(String),
    Effort(Effort),
    ToolStart {
        id: String,
        name: String,
        args: serde_json::Value,
        risk: Risk,
    },
    ToolResult {
        id: String,
        ok: bool,
        output: String,
        duration_ms: u64,
    },
    Approval(ApprovalRequest),
    Usage(Usage),
    Done,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct Delta {
    pub text: Option<String>,
    pub reasoning: Option<String>,
}
