use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use termagent_core::session::Session;
use termagent_core::tools::ApproveFn;
use termagent_core::types::AgentEvent;
use termagent_core::{Agent, Config, Effort, Registry};
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
        println!(
            "models.dev: {} providers, {} models",
            registry.providers.len(),
            registry.total_models()
        );
        return Ok(());
    }
    if registry.providers.is_empty() {
        return Err(termagent_core::Error::Registry(
            "no providers available; run termagent --refresh".into(),
        ));
    }

    if cli.list_models {
        let configured = registry.configured(&cfg);
        if configured.is_empty() {
            println!("no provider configured yet; run `termagent --onboard`");
            println!(
                "registry: {} providers, {} models",
                registry.providers.len(),
                registry.total_models()
            );
            return Ok(());
        }
        for provider in configured {
            println!(
                "{} ({}) · {} models",
                provider.name,
                provider.id,
                provider.models.len()
            );
            for model in provider.models.iter().take(20) {
                println!("  {}/{}", provider.id, model.id);
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
    termagent_tui::run(cfg, registry, cwd)
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
                        print!("{text}");
                        let _ = std::io::stdout().flush();
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
        println!();
        result
    })
}
