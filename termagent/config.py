from __future__ import annotations

import json
import os
from dataclasses import asdict, dataclass, field
from pathlib import Path

CONFIG_DIR_ENV = "TERMAGENT_CONFIG_DIR"
DEFAULT_CONFIG_DIR = Path.home() / ".config" / "termagent"
CONFIG_FILENAME = "config.json"


@dataclass
class Config:
    api_key: str = ""
    base_url: str = "https://opencode.ai/zen/v1"
    model: str = "north-mini-code-free"
    system: str = ""
    max_turns: int = 24
    auto_approve: bool = False
    history_lines: int = 2000
    tools: list[str] = field(default_factory=list)
    extra_headers: dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> dict:
        d = asdict(self)
        d.pop("api_key", None)
        d["has_api_key"] = bool(self.api_key)
        return d


def config_dir() -> Path:
    env = os.environ.get(CONFIG_DIR_ENV)
    if env:
        return Path(env).expanduser()
    return DEFAULT_CONFIG_DIR


def config_path() -> Path:
    return config_dir() / CONFIG_FILENAME


def default_system_prompt() -> str:
    return (
        "You are termagent, an interactive terminal coding agent running on the "
        "user's machine (often Termux on Android). You help with software "
        "engineering tasks by using tools.\n\n"
        "Operating principles:\n"
        "- Be concise. Do not add commentary the user did not ask for.\n"
        "- Use tools to inspect the environment before assuming anything.\n"
        "- Prefer reading files before editing them; never guess file contents.\n"
        "- When editing files, preserve existing style and conventions.\n"
        "- Do not add comments to code unless asked.\n"
        "- Do not commit, push, or run destructive commands unless the user asks.\n"
        "- When a task needs multiple steps, use the todo tool to track them.\n"
        "- Answer in plain text; use markdown sparingly since output is a terminal.\n"
    )


def load() -> Config:
    cfg = Config()
    path = config_path()
    if path.exists():
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
            known = {f for f in Config.__dataclass_fields__}
            for k, v in data.items():
                if k in known:
                    setattr(cfg, k, v)
        except Exception:
            pass

    # Environment overrides (highest priority for secrets/ephemeral use).
    if os.environ.get("OPENAI_API_KEY"):
        cfg.api_key = os.environ["OPENAI_API_KEY"]
    if os.environ.get("TERMAGENT_API_KEY"):
        cfg.api_key = os.environ["TERMAGENT_API_KEY"]
    if os.environ.get("OPENAI_BASE_URL"):
        cfg.base_url = os.environ["OPENAI_BASE_URL"]
    if os.environ.get("TERMAGENT_BASE_URL"):
        cfg.base_url = os.environ["TERMAGENT_BASE_URL"]
    if os.environ.get("OPENAI_MODEL"):
        cfg.model = os.environ["OPENAI_MODEL"]
    if os.environ.get("TERMAGENT_MODEL"):
        cfg.model = os.environ["TERMAGENT_MODEL"]
    if os.environ.get("TERMAGENT_AUTO_APPROVE"):
        cfg.auto_approve = os.environ["TERMAGENT_AUTO_APPROVE"].lower() in (
            "1",
            "true",
            "yes",
        )
    if os.environ.get("TERMAGENT_MAX_TURNS"):
        try:
            cfg.max_turns = int(os.environ["TERMAGENT_MAX_TURNS"])
        except ValueError:
            pass

    if not cfg.system:
        cfg.system = default_system_prompt()
    return cfg


def save(cfg: Config) -> None:
    path = config_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    d = asdict(cfg)
    path.write_text(json.dumps(d, indent=2), encoding="utf-8")
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass
