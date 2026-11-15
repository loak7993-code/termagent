use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use termagent_core::registry::{Model, Protocol, Provider};
use termagent_core::session::Session;
use termagent_core::tools::ApproveFn;
use termagent_core::types::AgentEvent;
use termagent_core::{Agent, Config, Effort, Registry, VERSION};
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(
    name = "termagent",
    version,
    about = "Coding agent in your terminal and browser",
    long_about = "termagent runs a coding agent in a native terminal UI, or serves a local web chat on 127.0.0.1.\n\
                  Providers and models come from models.dev. Runs entirely in user space; no root required.",
    disable_help_subcommand = true
)]
struct Cli {
    /// Prompt to run once, then exit
    #[arg(value_name = "PROMPT", trailing_var_arg = true)]
    prompt: Vec<String>,

    /// Serve the browser chat on 127.0.0.1
    #[arg(long)]
    web: bool,

    /// Web port (default 6033)
    #[arg(long, default_value_t = 6033, value_parser = clap::value_parser!(u16).range(1024..=65535))]
    port: u16,

    /// Force the provider, key, model and effort flow
    #[arg(long)]
    onboard: bool,

    /// Provider id from models.dev
    #[arg(long)]
    provider: Option<String>,

    /// Model id
    #[arg(short, long)]
    model: Option<String>,

    /// Custom OpenAI-compatible base URL
    #[arg(short, long = "base-url")]
    base_url: Option<String>,

    /// Team/project for CoreWeave and W&B Inference (OpenAI-Project header)
    #[arg(long)]
    project: Option<String>,

    /// API key (prefer the provider's environment variable)
    #[arg(short = 'k', long = "api-key")]
    api_key: Option<String>,

    /// Effort: auto, low, medium, high, max
    #[arg(long)]
    effort: Option<String>,

    /// Workspace directory
    #[arg(short, long)]
    cwd: Option<PathBuf>,

    /// Resume a saved session
    #[arg(short, long)]
    session: Option<String>,

    /// Approve medium-risk tools without asking (high risk still asks)
    #[arg(short = 'a', long = "auto-approve")]
    auto_approve: bool,

    /// Print configured providers and their first models
    #[arg(long)]
    list_models: bool,

    /// Refresh the models.dev registry
    #[arg(long)]
    refresh: bool,

    /// Show provider, model, key source, and paths without running a turn
    #[arg(long)]
    status: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("termagent: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> termagent_core::Result<()> {
    if let Some(dir) = &cli.cwd {
        std::env::set_current_dir(dir).map_err(|e| {
            termagent_core::Error::Config(format!("cannot enter {}: {e}", dir.display()))
        })?;
    }
    let cwd = std::env::current_dir()?;

    let mut cfg = Config::load()?;
    if let Some(provider) = &cli.provider {
        cfg.provider = provider.clone();
    }
    if let Some(model) = &cli.model {
        cfg.model = model.clone();
    }
    if let Some(base) = &cli.base_url {
        cfg.base_url = Some(base.clone());
    }
    if let Some(project) = &cli.project {
        cfg.project = Some(project.trim().to_string());
    }
    if let Some(key) = &cli.api_key {
        cfg.api_key = key.clone();
    }
    if let Some(effort) = &cli.effort {
        cfg.effort = effort.parse::<Effort>()?;
    }
    if cli.auto_approve {
        cfg.auto_approve = true;
    }

    let mut registry = Registry::load_cached();
    if cli.refresh {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        registry = runtime.block_on(Registry::refresh())?;
        print_line(&format!(
            "models.dev: {} providers, {} models",
            registry.providers.len(),
            registry.total_models()
        ));
        return Ok(());
    }
    if registry.providers.is_empty() {
        return Err(termagent_core::Error::Registry(
            "no providers available; run termagent --refresh".into(),
        ));
    }

    // A bare --base-url gets a synthetic OpenAI-compatible provider so local
    // servers and gateways work without picking a registry entry first.
    if let Some(base) = cfg.base_url.clone() {
        if registry.provider(&cfg.provider).is_none() {
            if cfg.model.is_empty() {
                cfg.model = "default".into();
            }
            registry.providers.retain(|p| p.id != "custom");
            registry.providers.push(Provider {
                id: "custom".into(),
                name: "Custom endpoint".into(),
                env: Vec::new(),
                api: base,
                protocol: Protocol::OpenAi,
                models: vec![Model {
                    id: cfg.model.clone(),
                    name: cfg.model.clone(),
                    reasoning: false,
                    efforts: Vec::new(),
                    tool_call: true,
                    context: 0,
                    cost_in: 0.0,
                    cost_out: 0.0,
                }],
            });
            cfg.provider = "custom".into();
        }
    }

    if cli.status {
        return print_status(&cfg, &registry, &cwd);
    }

    if cli.list_models {
        let configured = registry.configured(&cfg);
        if configured.is_empty() {
            print_line("no provider configured yet; run `termagent --onboard`");
            print_line(&format!(
                "registry: {} providers, {} models",
                registry.providers.len(),
                registry.total_models()
            ));
            return Ok(());
        }
        for provider in configured {
            print_line(&format!(
                "{} ({}) · {} models",
                provider.name,
                provider.id,
                provider.models.len()
            ));
            for model in provider.models.iter().take(20) {
                print_line(&format!("  {}/{}", provider.id, model.id));
            }
        }
        return Ok(());
    }

    if cli.onboard {
        return termagent_tui::run_onboarding(cfg, registry, cwd);
    }
    if cli.web {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        return runtime.block_on(termagent_web::serve(cfg, registry, cli.port));
    }

    let prompt = cli.prompt.join(" ");
    if !prompt.trim().is_empty() {
        return one_shot(
            cfg,
            registry,
            cwd,
            prompt.trim(),
            cli.session,
            cli.auto_approve,
        );
    }
    termagent_tui::run(cfg, registry, cwd, cli.session)
}

fn one_shot(
    cfg: Config,
    registry: Registry,
    cwd: PathBuf,
    prompt: &str,
    session_id: Option<String>,
    auto_approve: bool,
) -> termagent_core::Result<()> {
    if !cfg.is_configured(&registry) {
        return Err(termagent_core::Error::NotConfigured);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let prompt = prompt.to_string();
    runtime.block_on(async move {
        let session = match session_id {
            Some(id) => Session::open(&id)?,
            None => Session::new(&cwd, &cfg.provider, &cfg.model, cfg.effort)?,
        };
        let (tx, mut rx) = unbounded_channel();
        let printer = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                match ev {
                    AgentEvent::Text(text) => {
                        use std::io::Write;
                        let mut out = std::io::stdout();
                        let _ = out.write_all(text.as_bytes());
                        let _ = out.flush();
                    }
                    AgentEvent::ToolStart { name, args, .. } => {
                        eprintln!(
                            "\n· {name}: {}",
                            termagent_core::safety::summarize_args(&args)
                        );
                    }
                    AgentEvent::ToolResult { ok, .. } => {
                        eprintln!("  {}", if ok { "ok" } else { "failed" });
                    }
                    AgentEvent::Error(message) => eprintln!("error: {message}"),
                    _ => {}
                }
            }
        });
        let approve: ApproveFn = Arc::new(move |req| {
            Box::pin(async move {
                if auto_approve {
                    true
                } else {
                    eprintln!("denied (non-interactive): {}", req.summary);
                    false
                }
            })
        });
        let mut agent = Agent::new(cfg, registry, session, tx, approve)?;
        let result = agent.run_turn(&prompt, CancellationToken::new()).await;
        drop(agent);
        let _ = printer.await;
        print_line("");
        result
    })
}

/// Writer that tolerates a closed pipe (`termagent --list-models | head`).
fn print_line(line: &str) {
    use std::io::Write;
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{line}");
}

fn print_status(
    cfg: &Config,
    registry: &Registry,
    cwd: &std::path::Path,
) -> termagent_core::Result<()> {
    print_line(&format!("termagent {VERSION}"));
    print_line(&format!("config: {}", Config::path().display()));
    print_line(&format!("workspace: {}", cwd.display()));

    let provider = registry.provider(&cfg.provider);
    match provider {
        Some(provider) => {
            print_line(&format!("provider: {} ({})", provider.name, provider.id));
            print_line(&format!(
                "endpoint: {}",
                cfg.base_url.as_deref().unwrap_or(&provider.api)
            ));
            print_line(&format!("key: {}", key_source(cfg, provider)));
        }
        None => print_line("provider: (not configured)"),
    }
    print_line(&format!(
        "model: {}",
        if cfg.model.is_empty() {
            "(not configured)"
        } else {
            &cfg.model
        }
    ));
    print_line(&format!("effort: {}", cfg.effort.label().to_lowercase()));
    print_line(&format!(
        "auto-approve: {}",
        if cfg.auto_approve { "on" } else { "off" }
    ));
    print_line(&format!(
        "registry: {} · {} providers · {} models",
        registry.source,
        registry.providers.len(),
        registry.total_models()
    ));
    Ok(())
}

fn key_source(cfg: &Config, provider: &termagent_core::Provider) -> String {
    if let Some(key) = cfg.keys.get(&provider.id) {
        if !key.trim().is_empty() {
            return format!("configured (config, {})", mask_key(key));
        }
    }
    if cfg.provider == provider.id && !cfg.api_key.is_empty() {
        return format!("configured (config, legacy, {})", mask_key(&cfg.api_key));
    }
    if cfg.provider == provider.id {
        if let Ok(value) = std::env::var("TERMAGENT_API_KEY") {
            if !value.is_empty() {
                return format!("configured (TERMAGENT_API_KEY, {})", mask_key(&value));
            }
        }
    }
    for var in &provider.env {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() {
                return format!("configured ({var}, {})", mask_key(&value));
            }
        }
    }
    if provider.api.contains("127.0.0.1") || provider.api.contains("localhost") {
        return "not needed (local endpoint)".into();
    }
    "missing".into()
}

fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.trim().chars().collect();
    if chars.len() <= 8 {
        return "set (short)".into();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}...{tail}")
}
