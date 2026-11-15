#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config: {0}")]
    Config(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(String),
    #[error("api {status}: {message}")]
    Api { status: u16, message: String },
    #[error("registry: {0}")]
    Registry(String),
    #[error("tool: {0}")]
    Tool(String),
    #[error("approval denied")]
    Denied,
    #[error("cancelled")]
    Cancelled,
    #[error("not configured: run `termagent --onboard` or set a provider key")]
    NotConfigured,
    #[error("{0}")]
    Terminal(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn api(status: u16, body: impl Into<String>) -> Self {
        Error::Api {
            status,
            message: friendly_error(&body.into()),
        }
    }
}

/// Pull the human message out of provider JSON before truncating.
fn friendly_error(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        let message = value
            .pointer("/error/message")
            .or_else(|| value.get("message"))
            .and_then(serde_json::Value::as_str);
        if let Some(message) = message {
            let kind = value
                .pointer("/error/type")
                .or_else(|| value.pointer("/error/code"))
                .and_then(serde_json::Value::as_str);
            return match kind {
                Some(kind) => format!("{kind}: {message}"),
                None => message.to_string(),
            };
        }
    }
    truncate_error(body)
}

fn truncate_error(body: &str) -> String {
    // Providers often wrap the useful message in JSON. Keep the raw body short
    // and readable instead of dumping kilobytes into the UI.
    let compact: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 400 {
        compact
    } else {
        let mut s: String = compact.chars().take(400).collect();
        s.push('…');
        s
    }
}

pub type Result<T> = std::result::Result<T, Error>;
