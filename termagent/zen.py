from __future__ import annotations

import httpx

ZEN_BASE = "https://opencode.ai/zen/v1"

# Models on OpenCode Zen served via the OpenAI-compatible /chat/completions
# endpoint, verified against the live /models catalog (Oct 2026).
#
# Two things changed upstream since 0.1.0 and are reflected here:
#   1. The old free models (north-mini-code-free, deepseek-v4-flash-free,
#      laguna-s-2.1-free, nemotron-3-ultra-free) no longer exist.
#   2. Zen's free tier is now gated server-side to the official OpenCode
#      client (403 FreeTierError for third-party user agents). Free models
#      are still listed — the client detects the 403 and explains options.
#
# Claude-*, Gemini-*, GPT-* and mistral models are intentionally excluded:
# Zen serves them via /messages or /responses, which this client does not
# implement.

_FREE_SUFFIX = "-free"
_FREE_EXACT = {"big-pickle", "space-bunny", "cat-jam"}

CHAT_MODEL_IDS: list[str] = [
    # ---- free-labeled ----
    "big-pickle",
    "ling-3.1-flash-free",
    "cat-jam-free",
    "mimo-v2.6-flash-free",
    "muse-spark-1.2-contributor-free",
    "muse-spark-1.3-contributor-free",
    "nemotron-3.5-lightning-free",
    "jev-1.13-free",
    "step-5-preview-free",
    "longcat-2.5-preview-free",
    "space-bunny-free",
    # ---- paid ----
    "jev-1.13",
    "deepseek-v4-flash",
    "deepseek-v4.1-flash",
    "deepseek-v4-pro",
    "glm-5",
    "glm-5.1",
    "glm-5.2",
    "glm-5.3",
    "glm-5.3-flash",
    "grok-4.5",
    "grok-4.6",
    "grok-4.7",
    "grok-build-0.1",
    "kimi-k2.5",
    "kimi-k2.6",
    "kimi-k2.7-code",
    "kimi-k3",
    "minimax-m2.5",
    "minimax-m2.7",
    "minimax-m3",
    "muse-spark-1.2",
    "muse-spark-1.3",
    "qwen3.5-plus",
    "qwen3.6-plus",
    "qwen3.8-flash",
    "qwen3.8-max",
]

_FAMILY_NAMES = {
    "glm": "GLM",
    "mimo": "MiMo",
    "muse": "Muse",
    "deepseek": "DeepSeek",
    "minimax": "MiniMax",
    "nemotron": "Nemotron",
    "longcat": "LongCat",
    "ling": "Ling",
    "jev": "JEV",
    "step": "Step",
}

_FALLBACK_LABELS = {
    "big-pickle": "Big Pickle (stealth)",
    "cat-jam-free": "Cat Jam",
    "space-bunny-free": "Space Bunny",
    "grok-build-0.1": "Grok Build 0.1",
    "kimi-k2.7-code": "Kimi K2.7 Code",
    "kimi-k3": "Kimi K3",
}

DEFAULT_FREE_MODEL = "big-pickle"


def is_free(model_id: str) -> bool:
    return model_id.endswith(_FREE_SUFFIX) or model_id in _FREE_EXACT


def label_for(model_id: str) -> str:
    if model_id in _FALLBACK_LABELS:
        base = _FALLBACK_LABELS[model_id]
    else:
        parts = model_id.split("-")
        if parts[0].startswith("qwen"):
            family = "Qwen" + parts[0][4:]
        else:
            family = _FAMILY_NAMES.get(parts[0], parts[0].capitalize())
        rest = " ".join(p.capitalize() for p in parts[1:])
        base = f"{family} {rest}".strip()
    return f"{base} (free)" if is_free(model_id) else base


class ZenModel:
    def __init__(self, id: str, label: str, free: bool):
        self.id = id
        self.label = label
        self.free = free


def catalog() -> list[ZenModel]:
    return [ZenModel(mid, label_for(mid), is_free(mid)) for mid in CHAT_MODEL_IDS]


def is_known(model_id: str) -> bool:
    return model_id in CHAT_MODEL_IDS


def fetch_available(base_url: str, api_key: str = "", timeout: float = 12.0) -> list[str]:
    """Fetch the live model list from the provider's /models endpoint.

    Returns model ids that are known chat/completions models (filtered), so the
    picker only shows models this client can actually drive. Falls back to the
    static catalog on any error.
    """
    url = base_url.rstrip("/") + "/models"
    headers = {}
    if api_key:
        headers["authorization"] = f"Bearer {api_key}"
    try:
        r = httpx.get(url, headers=headers, timeout=timeout)
        r.raise_for_status()
        data = r.json()
        ids = [m.get("id", "") for m in data.get("data", [])]
        known = [i for i in ids if is_known(i)]
        return known if known else list(CHAT_MODEL_IDS)
    except Exception:
        return list(CHAT_MODEL_IDS)
