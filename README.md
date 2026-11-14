# termagent

A coding agent that runs in your terminal and your browser. One Rust binary, one
workspace, no root. It reads files, edits them, searches, runs commands with your
approval, and keeps every session on disk.

```
termagent           native terminal UI
termagent --web     local chat UI on http://127.0.0.1:6033
```

On first run it asks for a provider, a key, a model, and a reasoning level. Providers
and models come from [models.dev](https://models.dev), so the list is current and only
models that support tool calling are offered. Keys stay on the machine.

![onboarding](docs/screenshots/onboarding.png)

## Features

- Native TUI on ratatui: streaming replies, tool cards, approval modal, model picker,
  session picker, eight themes, ASCII fallback.
- Browser chat served by the binary: session sidebar, searchable model chooser,
  inline approvals, token usage, mobile layout.
- Providers and models from models.dev: 309 providers and 7,958 tool-capable models
  in the bundled snapshot, refreshed on demand and cached for 24 hours.
- Three protocol adapters: OpenAI-compatible chat completions, Anthropic Messages,
  and Google Gemini. Every other provider in the registry speaks the OpenAI shape.
- Per-provider API keys, stored with mode 0600, or read from the provider's
  environment variable. One provider's key never authenticates another.
- Effort control: auto, low, medium, high, max. Auto picks a level per message and
  each protocol receives it in its own field (`reasoning_effort`, thinking budget,
  `thinkingBudget`).
- Tools: `read`, `write`, `edit`, `ls`, `glob`, `grep`, `bash`, `todo`, `webfetch`.
- Sessions as JSONL, resume by id, rename, delete, token totals.
- Runs in user space. Privilege escalation commands are refused, child processes
  never inherit API keys, and file tools cannot leave the workspace.

| Terminal | Model chooser |
|---|---|
| ![tui](docs/screenshots/tui.png) | ![models](docs/screenshots/models.png) |

| Chat | 
|---|
| ![chat](docs/screenshots/chat.png) |

## Quick start

Requires Rust 1.85 or newer. Node 20+ is only needed to rebuild the web client.

```sh
git clone https://github.com/loak7993-code/termagent
cd termagent
cargo build --release
./target/release/termagent
```

First run opens the onboarding wizard. Pick a provider, paste a key (or export the
provider's environment variable first), pick a model, pick effort, press enter.

Install into your user path:

```sh
./install.sh                 # installs to ~/.local/bin/termagent
# or
install -Dm755 target/release/termagent ~/.local/bin/termagent
```

On Termux: `pkg install rust git`, then the same build.

### Web mode

```sh
termagent --web              # http://127.0.0.1:6033
termagent --web --port 6044
```

The server binds loopback only. Every boot mints a new token, injected into the page,
required by the API and the WebSocket. Nothing is exposed to the network.

If the config is missing, the web UI opens the same onboarding flow. Keys never reach
the browser; the page only sees masked state.

### One-shot mode

```sh
termagent "run the tests and fix the first failure"
termagent --auto-approve "list the rust files"    # high-risk commands still ask
```

## CLI

| Flag | Purpose |
|---|---|
| `--web` | serve the browser UI instead of the TUI |
| `--port <n>` | web port, 1024-65535, default 6033 |
| `--onboard` | force the provider/key/model wizard |
| `-m, --model <id>` | override the model for this run |
| `--provider <id>` | override the provider |
| `-b, --base-url <url>` | custom OpenAI-compatible endpoint |
| `-k, --api-key <key>` | key for this run (prefer environment variables) |
| `--effort <level>` | auto, low, medium, high, max |
| `-c, --cwd <dir>` | workspace directory |
| `-s, --session <id>` | resume a stored session |
| `-a, --auto-approve` | skip medium-risk approvals (high risk still asks) |
| `--list-models` | print configured providers and their first models |
| `--refresh` | refresh the models.dev registry |

TUI keys: `enter` send, `esc` stop, `ctrl+k` models, `ctrl+s` sessions, `ctrl+e`
effort, `ctrl+t` theme, `ctrl+c` quit, `f1` help. Slash commands: `/model`,
`/effort`, `/sessions`, `/new`, `/theme`, `/config`, `/tools`, `/cwd`, `/help`,
`/exit`.

## Configuration

Files live in `$TERMAGENT_CONFIG_DIR`, default `~/.config/termagent`:

| Path | Contents |
|---|---|
| `config.json` | provider, model, effort, per-provider keys, theme, safety flags. Mode 0600. |
| `registry.json` | models.dev cache, refreshed every 24 hours or with `--refresh` |
| `sessions/<id>.jsonl` | one event log per session |

| Environment variable | Purpose |
|---|---|
| `TERMAGENT_CONFIG_DIR` | config and session directory |
| `TERMAGENT_PROVIDER` | provider id |
| `TERMAGENT_MODEL` | model id |
| `TERMAGENT_BASE_URL` | custom API base URL |
| `TERMAGENT_API_KEY` | key for the active provider |
| `TERMAGENT_EFFORT` | auto, low, medium, high, max |
| `TERMAGENT_THEME` | TUI theme name |
| `TERMAGENT_AUTO_APPROVE` | `1` enables auto-approve |
| `TERMAGENT_ASCII` | `1` uses ASCII borders and symbols |

Every provider's own variable from models.dev is honored, for example
`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `GROQ_API_KEY`.

Custom local endpoints work without a key when the base URL points at localhost
(LM Studio, Ollama, llama.cpp). Pass `-b http://127.0.0.1:1234/v1 -m <model>`.

## Safety model

- No privilege escalation. `sudo`, `su`, `doas`, `pkexec`, and `run0` are refused
  before a command runs. termagent itself never needs root.
- Every file path is resolved inside the workspace; `..` escapes and symlinks that
  point outside are rejected.
- Tools that change state ask first: `write`, `edit`, `bash`, `webfetch`. Destructive
  commands such as `rm -rf /`, `mkfs`, or `curl | sh` are classified high risk and ask
  even with `--auto-approve`.
- Child processes start without `*_API_KEY`, `*_TOKEN`, `*SECRET*`, `*PASSWORD*`,
  `AWS_*`, `AZURE_*`, `GOOGLE_APPLICATION*`, or `SSH_AUTH_SOCK`.
- Shell commands time out after 120 seconds and output is capped at 60k characters.
- The web server is loopback only, token gated, origin checked, and sends no CORS
  headers. Keys are never serialized into pages, WebSocket frames, or session files.
- No telemetry. Requests go only to the provider you configured.

## Build, test, develop

```sh
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

The web client builds separately and is embedded into the binary:

```sh
cd web
npm ci
npm run build          # writes web/dist, no source maps
cd ..
cargo build --release  # re-embeds web/dist
```

Regenerate the offline registry snapshot after a models.dev refresh:

```sh
node scripts/gen_registry.mjs            # fetches models.dev when no local snapshot
```

## Architecture

```
crates/core        agent loop, providers, tools, config, registry, sessions, safety
  providers/       openai.rs, anthropic.rs, gemini.rs, sse.rs
  tools/           read, write, edit, ls, glob, grep, bash, todo, webfetch
crates/tui         ratatui screens, key handling, worker thread, themes
crates/web         axum server, REST API, WebSocket protocol, embedded assets
crates/cli         binary, flags, one-shot mode
web                Vite + React client (Tailwind v4, shadcn, @aicss/react)
scripts            registry snapshot generator
docs/screenshots   verified captures at 390, 768, 1440
```

The TUI owns an agent task on a two-thread Tokio runtime and exchanges events over a
channel. The web server runs one conversation per process and broadcasts events over a
WebSocket. Both use the same `Agent`, tools, session log, and provider adapters.

## Known limitations

- One conversation per `--web` process. Multiple tabs share it.
- No context compaction; `/new` starts a clean window, `max_turns` caps tool rounds.
- Tool arguments stream only when a call completes, so the tool card appears after the
  model finishes composing it.
- Terminal composer is single-line; long prompts scroll horizontally.
- Bedrock, Vertex, Azure, and Copilot are excluded from onboarding because they need
  cloud IAM rather than one API key.
- The bundled registry snapshot is a point in time; use `--refresh` or the Refresh
  button for the newest models.
- Linux and Termux are verified. macOS is expected to work. Windows is untested.
- Screenshots in this README come from a scripted local provider used for UI
  verification; no third-party model keys were used.

## License

MIT. Built with ratatui, axum, and Tokio. Web client uses shadcn components and
`@aicss/react`. Registry data from models.dev.
