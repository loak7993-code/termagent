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
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn api(status: u16, body: impl Into<String>) -> Self {
        Error::Api {
            status,
            message: truncate_error(&body.into()),
        }
    }
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
