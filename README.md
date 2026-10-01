t.me/destinedtt
# termagent

A tiny terminal AI coding agent — like `opencode`, `codex`, and `claude code`, but
written in Python so it installs and runs cleanly on **Termux (Android)** as well as
any Linux/macOS shell.

It speaks the **OpenAI-compatible Chat Completions API** (with tool/function calling
and streaming), so it works with [OpenCode Zen](https://opencode.ai/zen) (default,
**free, no auth**), OpenAI, OpenRouter, Together, Groq, any
`vLLM`/`llama.cpp`/`Ollama`/`LM Studio` OpenAI endpoint, etc.

## Features

- Interactive REPL with **streaming** markdown rendering (via `rich`)
- **Tool use**: `bash`, `read`, `write`, `edit`, `ls`, `glob`, `grep`, `todo`
- **Human-in-the-loop approval** for shell commands and file writes (toggle with
  `/approve` or `-a`)
- **`/models` picker** — list & switch models live (free Zen models need no auth)
- **Session save/resume** (`/save`, `/load`, `/sessions`)
- Slash commands: `/help /clear /models /model /config /approve /cwd /tools /sys /save /load`
- One-shot mode: `termagent "explain this repo"`
- Minimal dependencies: just `httpx` + `rich` (prompt_toolkit is optional)
- Pure Python — no native compilation, ideal for Termux

## Install on Termux

```sh
pkg update && pkg install -y python python-pip git
pip install httpx rich           # required (or use the [input] extra for nicer input)
pip install prompt_toolkit       # optional: arrow-key history + multiline editing
```

Then either clone this repo and run in place, or install it:

```sh
git clone <this-repo> && cd termagent
pip install -e .                 # installs the `termagent` command
```

## Works out of the box (free, no API key)

By default termagent points at **OpenCode Zen** and uses `north-mini-code-free` — a
free coding model that needs **no authentication**. Just run it:

```sh
termagent            # starts immediately, no key needed
```

Want a different free model? Inside the REPL type `/models` and pick one:

```
> /models
Free models (no auth needed)
   1  north-mini-code-free       North Mini Code (free, coding) <— current
   2  deepseek-v4-flash-free     DeepSeek V4 Flash (free)
   3  big-pickle                 Big Pickle (free, stealth)
   4  mimo-v2.5-free             MiMo-V2.5 (free)
   5  laguna-s-2.1-free          Laguna S 2.1 (free)
   6  nemotron-3-ultra-free      Nemotron 3 Ultra (free)
...
Pick a model (number or id, blank to cancel): 2
switched to deepseek-v4-flash-free free
```

The Zen free models (`big-pickle`, `deepseek-v4-flash-free`, `mimo-v2.5-free`,
`laguna-s-2.1-free`, `north-mini-code-free`, `nemotron-3-ultra-free`) all speak the
OpenAI `/chat/completions` format with tool calling — no signup, no key, no credits.

## Configure (optional — only for paid / other providers)

Pick **one** of these:

```sh
# 1) Environment variables (also work in ~/.bashrc / Termux)
export TERMAGENT_API_KEY="sk-..."          # or OPENAI_API_KEY
export TERMAGENT_BASE_URL="https://api.openai.com/v1"   # or OPENAI_BASE_URL
export TERMAGENT_MODEL="gpt-4o-mini"       # or OPENAI_MODEL

# 2) Interactive config file (saved to ~/.config/termagent/config.json)
termagent --config
```

### Pointing at other providers

```sh
# OpenCode Zen paid models (needs a Zen API key from opencode.ai/auth)
termagent -b https://opencode.ai/zen/v1 -k $ZEN_KEY -m glm-5.2

# OpenRouter (hundreds of models)
termagent -b https://openrouter.ai/api/v1 -k $OPENROUTER_KEY -m "anthropic/claude-3.5-sonnet"

# Local Ollama (OpenAI-compat endpoint)
termagent -b http://localhost:11434/v1 -k dummy -m llama3.1

# Local LM Studio / llama.cpp server
termagent -b http://localhost:1234/v1 -k dummy -m my-model
```

## Usage

```sh
termagent                      # interactive REPL in current dir
termagent "list the python files"   # one-shot, then exit
termagent -c /path/to/project       # set working directory
termagent -a                        # auto-approve all tool calls (risky)
termagent -s 20260721-214103-cc34ae # resume a saved session
```

Inside the REPL:

```
> find any TODO comments in this repo
> /models        list & switch models (free Zen models need no auth)
> /model glm-5.2 switch model mid-session
> /approve        toggle auto-approve
> /save           save the conversation
> /exit
```

Type `/paste` (or a blank line in paste mode) to enter multi-line input; end with a
line containing only `.`.

## How it works

`termagent` sends your messages plus a system prompt and a JSON-schema description
of its tools to the chat-completions endpoint with `stream: true`. As the model
streams tokens, they're rendered live. When it emits `tool_calls`, termagent runs
each tool locally (asking for approval unless auto-approve is on), appends the
results, and loops back to the model — exactly like opencode/codex/claude-code, just
smaller.

The agent loop, tools, and streaming client are all in a few hundred lines of plain
Python under `termagent/` — easy to read and hack on.

## Layout

```
termagent/
  cli.py        argparse entry, slash commands (/models, /model, ...), main loop, one-shot
  agent.py      the tool-calling loop
  client.py     OpenAI-compatible streaming client (SSE + tool-call accumulation)
  config.py     env + JSON config (defaults: OpenCode Zen, north-mini-code-free)
  zen.py        OpenCode Zen model catalog + live /models fetch
  session.py    JSONL conversation save/resume
  tui.py        rich-based terminal UI + approval prompts
  tools/
    base.py     Tool registry + ToolContext
    bash.py read.py write.py edit.py ls.py glob.py grep.py todo.py
```

## Requirements

- Python 3.9+
- `httpx`, `rich` (`prompt_toolkit` optional)
- A model that supports OpenAI-style tool/function calling (most current chat models
  do; small local models may not).

## License

MIT
