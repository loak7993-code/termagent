from __future__ import annotations

import json
from dataclasses import dataclass

import httpx

ZEN_BASE = "https://opencode.ai/zen/v1"

# Models on OpenCode Zen served via the OpenAI-compatible /chat/completions
# endpoint. (Claude models use /messages and GPT-5.x use /responses, which this
# client does not implement — so they're intentionally excluded from this list.)
# `free` reflects Zen's pricing page.

@dataclass
class ZenModel:
    id: str
    label: str
    free: bool


CHAT_MODELS: list[ZenModel] = [
    # ---- Free ----
    ZenModel("north-mini-code-free", "North Mini Code (free, coding)", True),
    ZenModel("deepseek-v4-flash-free", "DeepSeek V4 Flash (free)", True),
    ZenModel("big-pickle", "Big Pickle (free, stealth)", True),
    ZenModel("mimo-v2.5-free", "MiMo-V2.5 (free)", True),
    ZenModel("laguna-s-2.1-free", "Laguna S 2.1 (free)", True),
    ZenModel("nemotron-3-ultra-free", "Nemotron 3 Ultra (free)", True),
    # ---- Paid ----
    ZenModel("kimi-k2.7-code", "Kimi K2.7 Code", False),
    ZenModel("kimi-k2.6", "Kimi K2.6", False),
    ZenModel("kimi-k2.5", "Kimi K2.5", False),
    ZenModel("glm-5.2", "GLM 5.2", False),
    ZenModel("glm-5.1", "GLM 5.1", False),
    ZenModel("glm-5", "GLM 5", False),
    ZenModel("deepseek-v4-pro", "DeepSeek V4 Pro", False),
    ZenModel("deepseek-v4-flash", "DeepSeek V4 Flash", False),
    ZenModel("grok-4.5", "Grok 4.5", False),
    ZenModel("grok-build-0.1", "Grok Build 0.1", False),
    ZenModel("minimax-m3", "MiniMax M3", False),
    ZenModel("minimax-m2.7", "MiniMax M2.7", False),
    ZenModel("minimax-m2.5", "MiniMax M2.5", False),
]

_FREE_IDS = {m.id for m in CHAT_MODELS if m.free}
_CHAT_IDS = {m.id for m in CHAT_MODELS}


def catalog() -> list[ZenModel]:
    return list(CHAT_MODELS)


def is_free(model_id: str) -> bool:
    return model_id in _FREE_IDS


def is_known(model_id: str) -> bool:
    return model_id in _CHAT_IDS


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
        known = [i for i in ids if i in _CHAT_IDS]
        return known if known else [m.id for m in CHAT_MODELS]
    except Exception:
        return [m.id for m in CHAT_MODELS]
