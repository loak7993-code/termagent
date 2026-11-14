pub mod agent;
pub mod config;
pub mod effort;
pub mod error;
pub mod providers;
pub mod registry;
pub mod safety;
pub mod session;
pub mod tools;
pub mod types;

pub use agent::Agent;
pub use config::Config;
pub use error::{Error, Result};
pub use registry::{Model, Protocol, Provider, Registry};
pub use session::Session;
pub use types::{AgentEvent, ApprovalRequest, Effort, Message, Risk, Role, ToolCall, Usage};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
