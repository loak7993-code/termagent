// Builds crates/core/assets/registry.json.gz from https://models.dev/api.json.
// Dev-only: the Rust build embeds the snapshot so a fresh clone works offline.
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { gzipSync } from "node:zlib";

const SRC = process.env.MODELS_JSON ?? "/tmp/opencode/models.json";
const OUT = new URL("../crates/core/assets/registry.json.gz", import.meta.url);

// Providers with a known API base and a protocol adapter we implement.
const BASE_URLS = {
  anthropic: "https://api.anthropic.com/v1",
  openai: "https://api.openai.com/v1",
  google: "https://generativelanguage.googleapis.com/v1beta",
  groq: "https://api.groq.com/openai/v1",
  mistral: "https://api.mistral.ai/v1",
  xai: "https://api.x.ai/v1",
  cerebras: "https://api.cerebras.ai/v1",
  cohere: "https://api.cohere.com/compatibility/v1",
  perplexity: "https://api.perplexity.ai",
  vercel: "https://ai-gateway.vercel.sh/v1",
  ollama: "http://127.0.0.1:11434/v1",
};

// Cloud IAM providers need credentials we cannot collect as a single key.
const EXCLUDED = new Set([
  "amazon-bedrock",
  "google-vertex",
  "azure",
  "github-copilot",
  "google-vertex-anthropic",
]);

let source;
if (existsSync(SRC)) {
  source = readFileSync(SRC, "utf8");
} else {
  const res = await fetch("https://models.dev/api.json", {
    headers: { "user-agent": "termagent-registry-build" },
  });
  if (!res.ok) throw new Error(`models.dev: ${res.status}`);
  source = await res.text();
}
const raw = JSON.parse(source);
const providers = [];

for (const [id, p] of Object.entries(raw)) {
  if (EXCLUDED.has(id)) continue;
  const api = p.api ?? BASE_URLS[id];
  if (!api) continue;

  const protocol = id === "anthropic" ? "anthropic" : id === "google" ? "gemini" : "openai";
  const models = [];
  for (const [mid, m] of Object.entries(p.models ?? {})) {
    if (!m.tool_call) continue;
    const out = m.modalities?.output ?? ["text"];
    if (!out.includes("text")) continue;
    const offers = Array.isArray(m.reasoning_options) ? m.reasoning_options : [];
    const effort = offers.find((o) => o.type === "effort");
    models.push({
      id: mid,
      name: m.name ?? mid,
      reasoning: !!m.reasoning,
      efforts: effort?.values ?? [],
      tool_call: true,
      context: m.limit?.context ?? 0,
      cost_in: m.cost?.input ?? 0,
      cost_out: m.cost?.output ?? 0,
    });
  }
  if (models.length === 0) continue;

  providers.push({
    id,
    name: p.name ?? id,
    env: p.env ?? [],
    api,
    protocol,
    models,
  });
}

providers.sort((a, b) => a.name.localeCompare(b.name));
const registry = { fetched_at: 0, source: "bundled", providers };
const json = JSON.stringify(registry);
mkdirSync(new URL("../crates/core/assets/", import.meta.url), { recursive: true });
writeFileSync(OUT, gzipSync(json, { level: 9 }));

const totalModels = providers.reduce((n, p) => n + p.models.length, 0);
console.log(
  `providers=${providers.length} models=${totalModels} json=${(json.length / 1024).toFixed(0)}KB gz=${(gzipSync(json).length / 1024).toFixed(0)}KB`,
);
