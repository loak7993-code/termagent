// OpenCode Zen model catalog + live /models fetch. Port of zen.py (Oct 2026 catalog).
//
// Zen serves Claude-*/Gemini-*/GPT-* families via /messages or /responses,
// which this client does not implement, so they are excluded. The free tier
// is gated server-side to the official OpenCode client (403 FreeTierError);
// the client detects that and surfaces actionable options.

const FREE_SUFFIX = "-free";
const FREE_EXACT = new Set(["big-pickle", "space-bunny", "cat-jam"]);

export const CHAT_MODEL_IDS: string[] = [
  // free-labeled
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
  // paid
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
];

const FAMILY_NAMES: Record<string, string> = {
  glm: "GLM",
  mimo: "MiMo",
  muse: "Muse",
  deepseek: "DeepSeek",
  minimax: "MiniMax",
  nemotron: "Nemotron",
  longcat: "LongCat",
  ling: "Ling",
  jev: "JEV",
  step: "Step",
};

const FALLBACK_LABELS: Record<string, string> = {
  "big-pickle": "Big Pickle (stealth)",
  "cat-jam-free": "Cat Jam",
  "space-bunny-free": "Space Bunny",
  "grok-build-0.1": "Grok Build 0.1",
  "kimi-k2.7-code": "Kimi K2.7 Code",
  "kimi-k3": "Kimi K3",
};

export const DEFAULT_FREE_MODEL = "big-pickle";

export interface ZenModel {
  id: string;
  label: string;
  free: boolean;
}

export function isFree(modelId: string): boolean {
  return modelId.endsWith(FREE_SUFFIX) || FREE_EXACT.has(modelId);
}

const capitalize = (s: string): string =>
  s.length === 0 ? s : s[0].toUpperCase() + s.slice(1);

export function labelFor(modelId: string): string {
  if (modelId in FALLBACK_LABELS) {
    return FALLBACK_LABELS[modelId]; // already carries its own descriptor
  }
  // strip the -free suffix so labels read "X (free)", not "X Free (free)"
  const stem = modelId.endsWith(FREE_SUFFIX) ? modelId.slice(0, -FREE_SUFFIX.length) : modelId;
  const parts = stem.split("-");
  let family = parts[0] ?? modelId;
  if (family.startsWith("qwen")) {
    family = "Qwen" + family.slice(4);
  } else {
    family = FAMILY_NAMES[family] ?? capitalize(family);
  }
  const base = [family, ...parts.slice(1).map(capitalize)].join(" ").trim();
  return isFree(modelId) ? `${base} (free)` : base;
}

export function catalog(): ZenModel[] {
  return CHAT_MODEL_IDS.map((id) => ({ id, label: labelFor(id), free: isFree(id) }));
}

export function isKnown(modelId: string): boolean {
  return CHAT_MODEL_IDS.includes(modelId);
}

export async function fetchAvailable(
  baseUrl: string,
  apiKey = "",
  timeoutMs = 12_000,
): Promise<string[]> {
  const url = baseUrl.replace(/\/+$/, "") + "/models";
  const headers: Record<string, string> = {};
  if (apiKey) headers.authorization = `Bearer ${apiKey}`;
  try {
    const resp = await fetch(url, {
      headers,
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!resp.ok) return CHAT_MODEL_IDS;
    const data = (await resp.json()) as { data?: Array<{ id?: string }> };
    const ids = (data.data ?? []).map((m) => m.id ?? "");
    const known = ids.filter((i) => isKnown(i));
    return known.length > 0 ? known : CHAT_MODEL_IDS;
  } catch {
    return CHAT_MODEL_IDS;
  }
}
