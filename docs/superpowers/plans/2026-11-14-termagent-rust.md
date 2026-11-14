# termagent 2.0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Ink/TypeScript TUI with a single Rust binary that runs a native ratatui coding agent and serves a local React chat UI on 127.0.0.1:6033, with models.dev-driven onboarding and model selection.

**Architecture:** Cargo workspace: `termagent-core` (config, registry, providers, agent loop, tools, sessions), `termagent-tui` (ratatui screens), `termagent-web` (axum + WebSocket + embedded Vite build), `termagent-cli` (binary, flags, one-shot). The agent emits events on an mpsc channel; approvals return over oneshot channels.

**Tech Stack:** Rust 1.99, tokio, ratatui 0.29, crossterm 0.28, axum 0.8, reqwest 0.12 (rustls), serde, clap 4, include_dir, flate2. Web: Vite 7, React 19, TypeScript, Tailwind v4, shadcn, @aicss/react 0.1.3, motion, @phosphor-icons/react.

**Spec:** `docs/superpowers/specs/2026-11-14-termagent-rust-design.md`

## Global Constraints

- Bind web server to 127.0.0.1 only; WebSocket requires `?token=` matching the per-boot token; no CORS headers.
- API keys: stored under `$TERMAGENT_CONFIG_DIR` (default `~/.config/termagent`) with mode 0600; never logged, never serialized to session files, never sent to the browser (masked only).
- Tests run with no network and no API keys; provider tests use a local mock SSE server.
- tool output truncated at 60k chars per call.
- No `sudo`, `su`, `doas`, `pkexec` execution; path jail for file tools (workspace + config dir); writes require approval unless auto-approve.
- Binary name `termagent`; default web port 6033; config file `config.json`; sessions `sessions/<id>.jsonl`.
- Accent `#3584e4`; canvas `#070708`; Geist Sans + Geist Mono self-hosted; Phosphor icons; no purple, no emojis, no em-dashes in copy.

## Review Focus

- First run with no network: bundled registry snapshot must still let the user pick a provider and model.
- Provider returns 401/429 mid-stream: inline error, no partial assistant message persisted, session remains usable.
- Approval requested while the web socket is gone: denied after 60s, never auto-approved.
- Tool path arguments like `../../etc/passwd`, absolute paths, symlinks: rejected or approval-gated, never silently written.
- Model chooser with 8k models across 300 providers: renders fast, filters by configured providers first, search does not freeze the UI.

---

### Task 1: Workspace, config, and registry

**Files:** `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/{lib,config,registry,types,error}.rs`, `crates/core/tests/registry.rs`, `scripts/gen_registry.mjs`, `crates/core/assets/registry.json.gz`

**Interfaces:**
- `Config { provider: String, model: String, effort: Effort, base_url: Option<String>, api_key: String, system: String, max_turns: u32, auto_approve: bool, theme: String }`
- `Config::load() -> Result<Config, Error>`, `Config::save(&self)`, `Config::path() -> PathBuf`, `Config::dir() -> PathBuf`
- `Registry { providers: Vec<Provider> }`, `Provider { id, name, env: Vec<String>, api: Option<String>, protocol: Protocol, models: Vec<Model> }`
- `Model { id, name, reasoning: bool, efforts: Vec<String>, tool_call: bool, context: u64, cost_in: f64, cost_out: f64 }`
- `Registry::load_cached() -> Registry` (cache -> bundled snapshot), `Registry::refresh() -> Result<Registry>`, `Registry::supported() -> Vec<Provider>`
- `Protocol { OpenAi, Anthropic, Gemini }`
- `DEFAULT_BASE_URLS: &[(&str, &str)]` for anthropic/openai/google/groq/mistral/xai/cerebras/cohere/perplexity/vercel.

- [ ] Write `scripts/gen_registry.mjs` (trim api.json: providers with api or curated base URL, models with tool_call, keep id/name/reasoning/reasoning_options/limit/cost; write gzip snapshot).
- [ ] Tests: config roundtrip writes 0600; registry from snapshot has >100 providers, >1000 models, no provider lacking a base URL; `supported()` excludes bedrock/vertex/azure/copilot.
- [ ] Implement modules; `cargo test -p termagent-core registry config` passes.

### Task 2: Providers and streaming

**Files:** `crates/core/src/providers/{mod,openai,anthropic,gemini,sse}.rs`, `crates/core/tests/providers.rs`

**Interfaces:**
- `ChatRequest { model, messages, tools, effort, temperature, max_tokens }`
- `ProviderClient::new(ProviderSpec)`, `async fn stream(&self, req, tx: mpsc::Sender<Delta>, cancel: CancellationToken) -> Result<Usage>`
- `Delta { text: Option<String>, reasoning: Option<String>, tool_calls: Vec<ToolCall> }`
- `Effort { Auto, Low, Medium, High, Max }` with `resolve(auto_ctx) -> Effort` and payload mapping per protocol.

- [ ] Tests against a local mock SSE server (tokio TcpListener): OpenAI text+tool_calls+usage; Anthropic content_block_delta + tool_use + thinking; Gemini candidates parts + usageMetadata; 401 surfaces `Error::Api`; cancel stops reading.
- [ ] Implement SSE parser shared by adapters; map effort: OpenAI `reasoning_effort`, Anthropic `thinking.budget_tokens` (low 1024 / med 4096 / high 16384 / max 32768), Gemini `thinkingConfig.thinkingBudget`.
- [ ] `cargo test -p termagent-core providers` passes.

### Task 3: Tools, approvals, safety

**Files:** `crates/core/src/tools/{mod,read,write,edit,ls,glob,grep,bash,todo,webfetch}.rs`, `crates/core/src/safety.rs`, `crates/core/tests/tools.rs`

**Interfaces:**
- `trait Tool { fn name(&self) -> &'static str; fn schema(&self) -> Value; fn risk(&self, args) -> Risk; fn run(&self, args, ctx) -> BoxFuture<Result<String>> }`
- `ToolCtx { cwd: PathBuf, approve: ApprovalGate, todos: Arc<Mutex<Vec<Todo>>>, http: reqwest::Client }`
- `ApprovalGate::ask(Request) -> bool` (64s timeout -> false), `Risk { Low, Medium, High }`
- `safety::resolve_path(cwd, path) -> Result<PathBuf>` rejects escapes outside workspace; `safety::is_privilege_escalation(cmd) -> bool`.

- [ ] Tests: read caps at 2000 lines; write creates parents and is refused outside jail; edit requires unique match; glob/grep match fixtures; bash strips secrets from env, denies `sudo`, times out, truncates output; todo roundtrip; path escapes (`../..`, absolute outside) rejected.
- [ ] Implement; `cargo test -p termagent-core tools` passes.

### Task 4: Agent loop, sessions, effort heuristic

**Files:** `crates/core/src/{agent,session,effort}.rs`, `crates/core/tests/agent.rs`

**Interfaces:**
- `Agent::new(cfg, registry, session, tx: mpsc::Sender<AgentEvent>)`, `async fn run_turn(user_text, cancel) -> Result<()>`
- `AgentEvent { Text, Reasoning, ToolStart, ToolArgs, ToolResult, Approval, Usage, Done, Error }`
- `Session::create(cwd, provider, model) -> Session`, `Session::append(&Event)`, `Session::load(id)`, `list()`, `delete(id)`, `rename(id, title)`
- `effort::auto_level(text, previous_tool_rounds) -> Effort`

- [ ] Tests: full loop against mock SSE server executes a read tool and feeds result back; usage accumulates; cancel mid-stream aborts; session JSONL has no api_key string; auto effort picks Low for short text, High for code fence or 400+ chars; resume loads messages.
- [ ] Implement; `cargo test -p termagent-core` passes.

### Task 5: CLI and one-shot mode

**Files:** `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/cli/tests/cli.rs`

**Interfaces:** `termagent [prompt] [--web] [--port 6033] [--onboard] [--model] [--provider] [--base-url] [--effort] [--cwd] [--session] [-a] [--version] [--help]`

- [ ] Tests: `--version` prints `termagent 2.0.0`; unknown flag exits 2; `--web --port 0` is rejected (fixed range 1024-65535); one-shot without config prints the onboarding hint and exits 1.
- [ ] Implement dispatch before TUI/web crates compile; `cargo test -p termagent-cli` passes.

### Task 6: TUI

**Files:** `crates/tui/Cargo.toml`, `crates/tui/src/{lib,app,theme,widgets/*,screens/*}.rs`, `crates/tui/tests/render.rs`

**Interfaces:**
- `tui::run(cfg, registry) -> Result<()>`; onboarding when `cfg.is_configured() == false`.
- Widgets: `spinner(tick) -> &'static str`, `MessageList`, `Composer`, `ApprovalModal`, `ModelPicker`, `SessionPicker`, `Header`, `Footer`, `hint`.
- Keymap: Enter send, Alt+Enter newline, Esc cancel, Ctrl+K models, Ctrl+S sessions, Ctrl+E effort, Ctrl+T theme, Ctrl+C quit twice, F1 help.

- [ ] Tests with ratatui TestBackend: header shows provider/model/effort; streaming text renders and wraps; approval modal blocks send; model picker scrolls 20+ rows and filters; ASCII fallback.
- [ ] Implement screens; `cargo test -p termagent-tui` passes.

### Task 7: Web server

**Files:** `crates/web/Cargo.toml`, `crates/web/src/{lib,api,ws,assets}.rs`, `crates/web/tests/api.rs`, `crates/web/build.rs`

**Interfaces:** routes from the spec; WS message schemas:
client `{"type":"message","text"} | {"type":"approve","id","allow"} | {"type":"cancel"} | {"type":"set_model","provider","model","effort"} | {"type":"new_session"} | {"type":"load_session","id"}`
server `{"type":"text","delta"} | {"type":"reasoning","delta"} | {"type":"tool","id","name","args","risk"} | {"type":"tool_result","id","ok","output"} | {"type":"approval","id","name","args","risk"} | {"type":"usage","in","out"} | {"type":"done"} | {"type":"error","message"} | {"type":"state",...}`

- [ ] Tests with axum oneshot: `/api/state` masks keys and lists configured providers; unauthorized WS rejected; token WS accepted; chat turn streams text and done against mock provider; approval event roundtrip; sessions CRUD; path traversal on session id rejected.
- [ ] Implement; `cargo test -p termagent-web` passes.

### Task 8: Web UI

**Files:** `web/` (Vite app), `crates/web/assets` embed path.

- [ ] `npm create vite` scaffold, Tailwind v4, shadcn init + add components (button dialog input tooltip badge scroll-area skeleton command dropdown-menu separator), Geist fonts installed locally, `@aicss/react` wired for streaming text, prompt input, approval card, thinking state, code block.
- [ ] Screens: onboarding (provider select from `/api/state`, key, model, effort), chat with sidebar sessions, model dialog (search across all models, 20+ rows virtualized, effort segmented control including Auto), inline approvals, usage footer, empty state, error banner, mobile drawer sidebar.
- [ ] Verify: `npm run build` clean, no sourcemaps, dist copied to `crates/web/assets`? (embed `../../web/dist`).

### Task 9: Verification and ship

- [ ] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo build --release`.
- [ ] Run TUI in tmux, capture panes at 120x36; screenshot web at 390/768/1440 with velox-automation; analyze and fix defects; re-shoot.
- [ ] Seed a sample session for screenshots only via a documented script, clearly labeled; no demo provider in prod paths.
- [ ] Rewrite README (verified commands), CHANGELOG 2.0.0, install.sh (user-space, no sudo), remove TS sources and npm TUI deps.
- [ ] Fresh checkout test: clone to /tmp, `cargo build --release`, run `--version`, start `--web`, fetch `/api/state`.
