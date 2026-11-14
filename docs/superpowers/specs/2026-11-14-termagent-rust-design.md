# termagent 2.0: Rust rewrite, native TUI, local web chat

Status: approved direction (operator brief, 2026-11-14)

## Intent

The current build is an Ink/React TypeScript TUI aimed at Termux. It starts a Node
runtime, vendors a UI kit, hardcodes an OpenCode Zen catalog (including families the
client cannot speak to), has no first-run onboarding, and cannot show a model list
larger than 12. The operator wants a Rust rewrite with native terminal features, a
first-run provider/model picker fed by models.dev, a real coding-agent toolset that
never needs root, and a browser UI (`termagent --web`) with sessions in a sidebar and
a scrollable chooser over every configured provider's models.

Success: `cargo build --release` yields one `termagent` binary. First run asks for a
provider, a key, a model, and produces a working chat. `termagent` renders a native
TUI; `termagent --web` serves a dark React chat UI on 127.0.0.1:6033. The model list
comes from models.dev, so no model that cannot work is offered.

## Goals

- One static binary. Web assets are embedded; no CDN, no Node at runtime.
- Providers and models from https://models.dev/api.json (328 providers, ~8.4k models),
  cached under the config dir with a bundled snapshot for offline first run.
- Three protocol adapters: OpenAI Chat Completions (SSE), Anthropic Messages (SSE),
  Google Gemini generateContent (SSE). Everything else that models.dev lists with a
  base URL speaks OpenAI-compatible.
- Only working models are offered: protocol support is required, so Bedrock, Vertex,
  Azure and Copilot (cloud IAM, no key field) are excluded from onboarding.
- Onboarding in both surfaces: provider -> key -> model -> effort, one screen each.
  Keys stored 0600; env vars from the registry (`ANTHROPIC_API_KEY`, ...) are honored.
- Model chooser: every model of every configured provider, searchable, 20+ visible
  rows, virtualized scrolling, reasoning/effort badges, context and price shown.
- Effort control: Auto / low / medium / high / max where the model supports it.
  Auto is a per-message heuristic (length, code fences, task verbs, tool round-trips)
  mapped to each protocol's reasoning field.
- Real coding agent: read, write, edit, ls, glob, grep, bash, todo, webfetch.
  Approval gates with risk classes; workspace path jail; no privilege escalation.
- Sessions: JSONL under the config dir, resume, rename, delete, token totals.
- Web: sidebar of sessions, model dialog, streaming chat, inline approvals, usage.
  React + Vite + Tailwind v4 + shadcn components + `@aicss/react`, Bluma-style motion.

## Non-goals

- No accounts, OAuth, or multi-user server. The web UI is a single-user localhost tool.
- No image generation, speech, embeddings, or fine-tuning.
- No Tauri/Electron desktop shell.
- No Windows-specific testing (code is portable, verification is Linux).

## Anti-goals

- Fake data in production paths. Demo material, if any, is explicitly gated and labeled.
- Silent privilege elevation, sudo prompts, or writes outside the workspace without approval.
- Unbounded tool output or blocking work on the render thread.
- Purple, gradient slop, light mode by default.

## Constraints

- Rust 1.99 stable; tokio, axum 0.8, ratatui 0.29, crossterm.
- `cargo test` passes with no network and no API keys (mock SSE server in tests).
- Keys never reach the browser, logs, or session files. UI shows masked hints only.
- Web server binds 127.0.0.1 only; WebSocket requires a per-boot token and a
  same-origin check; CORS disabled.
- Built web assets committed, no source maps, so fresh clones build without Node.

## Approaches considered

1. **Full Rust core, native ratatui TUI, embedded React SPA** (picked). One binary,
   fast startup, no Node runtime, two first-class surfaces. Cost: two UIs to maintain.
2. Rust core with the existing Ink TUI kept. Reuses components but keeps Node and
   contradicts the native-TUI request.
3. Tauri desktop app. Heavier, not terminal-first, does not match the operator's usage.

`@termcn/ink` is not published on npm (404) and is React-based, so it cannot be used
from a Rust TUI; the TUI ships its own spinner, approval, and picker widgets. shadcn
is used on the web surface, `@aicss/react` (verified install) for chat primitives.

## Design

Workspace:

```
crates/core   config, registry, providers, agent loop, tools, sessions, effort
crates/tui    ratatui screens and widgets
crates/web    axum server, WebSocket protocol, embedded assets
crates/cli    binary dispatch, arg parsing, one-shot mode
web/          Vite + React + TS + Tailwind v4 source, builds to web/dist
```

Core types: `Config`, `Provider`, `Model`, `Effort`, `Session`, `Message`, `ToolCall`,
`Approval`, `AgentEvent`. The agent loop is an async state machine emitting events on
an mpsc channel; approvals travel back over oneshot channels. Rendering never blocks
on IO.

Registry: fetch `api.json`, keep providers with `api` or a curated base URL whose
protocol we implement, trim model fields to id/name/reasoning/reasoning_options/
tool_call/limit/cost/modalities, cache 24h, embed the same trimmed snapshot (gzip).

Endpoints: `GET /api/state`, `GET/POST /api/sessions`, `GET/PATCH/DELETE
/api/sessions/:id`, `GET /api/models`, `POST /api/config`, `POST /api/model`,
`POST /api/registry/refresh`, `GET /ws?token=`. WS events are tagged JSON.

Failure modes: offline first run uses the snapshot; provider errors surface inline
with retry; WS loss cancels the turn; an approval nobody answers is denied after 60s;
root is allowed but never escalated, with a one-time warning.

## Builder's calls

- Keep 8 curated themes instead of 17.
- Commit `web/dist` and the registry snapshot so fresh clones build with cargo only.
- Attach the web token via a `<meta>` tag and require it on the WebSocket.
- Use `@phosphor-icons/react` for authored icons; `@aicss/react` brings its own icons.
- Accent: GNOME blue `#3584e4`, dark canvas `#070708`, Geist Sans + Geist Mono self-hosted.
