import { useCallback, useEffect, useMemo, useState } from "react";
import { motion } from "motion/react";
import {
  ArrowClockwise,
  CaretDown,
  CircleNotch,
  Check,
  Key,
  Play,
} from "@phosphor-icons/react";
import { Brand } from "@/components/Brand";
import { EffortPicker } from "@/components/EffortPicker";
import { Picker, type PickerItem } from "@/components/Picker";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  api,
  compact,
  orderProviders,
  POPULAR,
  type Effort,
  type ModelInfo,
  type Snapshot,
} from "@/lib/termagent";

interface OnboardingProps {
  snapshot: Snapshot;
  initialProvider?: string;
  onComplete: (input: {
    provider: string;
    model: string;
    api_key: string;
    effort: Effort;
  }) => Promise<void>;
  onClose?: () => void;
  onRefreshRegistry: () => Promise<void>;
}

function defaultProvider(snapshot: Snapshot, initial?: string): string {
  if (initial) return initial;
  if (snapshot.provider) return snapshot.provider;
  const ordered = orderProviders(snapshot.providers, initial);
  const find = (id: string) => snapshot.providers.find((entry) => entry.id === id);
  for (const id of POPULAR) {
    const provider = find(id);
    if (provider?.envSet) return id;
  }
  for (const id of POPULAR) {
    const provider = find(id);
    if (provider?.keySet) return id;
  }
  const withCredential = ordered.find((entry) => entry.envSet || entry.keySet);
  if (withCredential) return withCredential.id;
  for (const id of POPULAR) {
    if (find(id)) return id;
  }
  return ordered[0]?.id ?? "";
}

export function Onboarding({
  snapshot,
  initialProvider,
  onComplete,
  onClose,
  onRefreshRegistry,
}: OnboardingProps) {
  const [providerId, setProviderId] = useState(() => defaultProvider(snapshot, initialProvider));
  const [key, setKey] = useState("");
  const [modelId, setModelId] = useState<string | null>(null);
  const [effort, setEffort] = useState<Effort>(snapshot.effort ?? "auto");
  const [models, setModels] = useState<Record<string, ModelInfo[]>>({});
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [providerPicker, setProviderPicker] = useState(false);
  const [modelPicker, setModelPicker] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const provider = snapshot.providers.find((entry) => entry.id === providerId);
  const providerModels = useMemo(() => models[providerId] ?? [], [models, providerId]);

  const load = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const data = await api<{ models: ModelInfo[] }>(
        `/api/models?provider=${encodeURIComponent(id)}`,
      );
      setModels((prev) => ({ ...prev, [id]: data.models }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "could not load models");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!providerId) return;
    if (!models[providerId]) void load(providerId);
  }, [providerId, models, load]);

  useEffect(() => {
    if (modelId && providerModels.some((model) => model.id === modelId)) return;
    if (snapshot.provider === providerId && providerModels.some((m) => m.id === snapshot.model)) {
      setModelId(snapshot.model);
    } else {
      setModelId(providerModels[0]?.id ?? null);
    }
  }, [providerModels, providerId, snapshot.provider, snapshot.model, modelId]);

  const model = providerModels.find((entry) => entry.id === modelId) ?? null;

  const providerItems: PickerItem[] = useMemo(
    () =>
      orderProviders(snapshot.providers, providerId).map((entry) => ({
        id: entry.id,
        name: entry.name,
        subtitle: entry.protocol === entry.id ? entry.id : `${entry.id} · ${entry.protocol}`,
        meta: (
          <span className="flex shrink-0 items-center gap-1.5">
            {entry.envSet && (
              <span className="mono text-[9.5px] text-ok" title="key found in your environment">
                env
              </span>
            )}
            {entry.keySet && <Check size={12} weight="bold" className="text-ok" />}
            <span className="mono text-[10px] text-faint">{entry.models}</span>
          </span>
        ),
      })),
    [snapshot.providers, providerId],
  );

  const modelItems: PickerItem[] = useMemo(
    () =>
      providerModels.map((entry) => ({
        id: entry.id,
        name: entry.name,
        subtitle: entry.id,
        meta: (
          <span className="flex shrink-0 items-center gap-2">
            {entry.reasoning && (
              <span className="rounded-full border border-brand/35 px-1.5 py-px text-[9px] text-brand">
                reasoning
              </span>
            )}
            <span className="mono text-[9.5px] text-faint">
              {entry.context > 0 ? compact(entry.context) : ""}
            </span>
          </span>
        ),
      })),
    [providerModels],
  );

  const keyOptional = Boolean(provider?.keySet || provider?.envSet || provider?.local);
  const canStart = Boolean(provider && model && !busy) && (key.trim().length > 0 || keyOptional);

  const submit = async () => {
    if (!provider || !model) return;
    setBusy(true);
    setError(null);
    try {
      await onComplete({
        provider: provider.id,
        model: model.id,
        api_key: key.trim(),
        effort,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "could not save the provider");
      setBusy(false);
    }
  };

  return (
    <div className="relative flex min-h-dvh items-center justify-center px-4 py-10">
      <div className="haze" />
      <motion.div
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
        className="relative z-10 w-full max-w-[600px] rounded-[14px] bg-panel p-6 shadow-[0_24px_80px_-32px_rgba(0,0,0,0.9)] md:p-8"
      >
        <div className="flex items-start justify-between">
          <Brand />
          {onClose && (
            <Button
              variant="ghost"
              size="sm"
              onClick={onClose}
              className="h-7 rounded-[6px] px-2 text-[11.5px] text-dim hover:text-ink"
            >
              Close
            </Button>
          )}
        </div>

        <div className="mt-7">
          <div className="mb-2 flex items-center gap-2">
            <span className="rounded-full bg-brand-soft px-2 py-0.5 text-[10px] font-medium text-brand">
              first run
            </span>
            <span className="mono text-[10px] uppercase tracking-[0.14em] text-faint">
              {snapshot.registry.providers} providers
            </span>
          </div>
          <h1 className="text-[26px] font-medium leading-tight tracking-tight text-ink">
            Connect a model
          </h1>
          <p className="mt-1.5 text-[13.5px] leading-6 text-dim">
            Pick any provider from models.dev. termagent uses it to read, edit and run code in
            this workspace.
          </p>
        </div>

        <div className="mt-6 space-y-4">
          <div>
            <label className="mb-1.5 block text-[11.5px] font-medium text-dim" htmlFor="provider">
              Provider
            </label>
            <button
              id="provider"
              type="button"
              onClick={() => setProviderPicker(true)}
              className="flex h-10 w-full items-center gap-3 rounded-md bg-white/[0.04] px-3 text-left transition-colors hover:bg-white/[0.06]"
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] text-ink">
                  {provider?.name ?? "Choose a provider"}
                </span>
                <span className="mono block truncate text-[10px] text-faint">
                  {provider ? `${provider.id} · ${provider.models} models` : "309 providers from models.dev"}
                </span>
              </span>
              {provider?.local ? (
                <span className="rounded-full bg-white/[0.06] px-2 py-0.5 text-[9.5px] text-dim">local</span>
              ) : provider?.envSet ? (
                <span className="rounded-full bg-ok/10 px-2 py-0.5 text-[9.5px] text-ok">env key</span>
              ) : provider?.keySet ? (
                <span className="rounded-full bg-white/[0.06] px-2 py-0.5 text-[9.5px] text-dim">key saved</span>
              ) : null}
              <CaretDown size={13} weight="bold" className="shrink-0 text-faint" />
            </button>
          </div>

          <div>
            <div className="mb-1.5 flex items-center justify-between">
              <label className="text-[11.5px] font-medium text-dim" htmlFor="api-key">
                API key
              </label>
              <span className="mono text-[10px] text-faint">
                {provider?.env?.[0] ?? "stored locally"}
              </span>
            </div>
            <div className="relative">
              <Key
                size={14}
                weight="bold"
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-faint"
              />
              <Input
                id="api-key"
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={key}
                onChange={(event) => setKey(event.target.value)}
                placeholder={
                  provider?.local
                    ? "Local endpoint, no key needed"
                    : keyOptional
                      ? "Key already available, leave empty"
                      : `Paste your ${provider?.name ?? ""} key`
                }
                className="h-10 rounded-md border-0 bg-white/[0.04] pl-9 text-[13px] focus-visible:ring-2 focus-visible:ring-brand/60"
              />
            </div>
            <p className="mt-1.5 text-[10.5px] leading-4 text-faint">
              {provider?.local
                ? "Talks to a server on this machine. No key is stored."
                : provider?.envSet
                  ? `${provider.env[0]} is set in your environment. Nothing is written to disk.`
                  : "Written 0600 to the termagent config on this machine. Never sent anywhere else."}
            </p>
          </div>

          <div>
            <label className="mb-1.5 block text-[11.5px] font-medium text-dim" htmlFor="model">
              Model
            </label>
            <button
              id="model"
              type="button"
              onClick={() => setModelPicker(true)}
              disabled={!provider}
              className="flex h-10 w-full items-center gap-3 rounded-md bg-white/[0.04] px-3 text-left transition-colors hover:bg-white/[0.06] disabled:opacity-50"
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] text-ink">
                  {model?.name ?? (loading ? "Loading models…" : "Choose a model")}
                </span>
                <span className="mono block truncate text-[10px] text-faint">
                  {model
                    ? `${model.id}${model.context ? ` · ${compact(model.context)} ctx` : ""}`
                    : "tool-capable models only"}
                </span>
              </span>
              {model?.reasoning && (
                <span className="hidden rounded-full border border-brand/35 px-1.5 py-px text-[9px] text-brand sm:block">
                  reasoning
                </span>
              )}
              <CaretDown size={13} weight="bold" className="shrink-0 text-faint" />
            </button>
          </div>

          <div>
            <p className="mb-1.5 text-[11.5px] font-medium text-dim">Reasoning effort</p>
            <EffortPicker value={effort} model={model ?? undefined} onChange={setEffort} />
          </div>

          {error && (
            <div className="rounded-md bg-danger/10 px-3 py-2 text-[12px] leading-5 text-danger">
              {error}
            </div>
          )}

          <Button
            type="button"
            onClick={submit}
            disabled={!canStart}
            className="h-10 w-full rounded-md bg-brand text-[13px] font-medium text-white hover:bg-brand/90 disabled:bg-white/[0.08] disabled:text-dim"
          >
            {busy ? (
              <CircleNotch size={15} weight="bold" className="animate-spin" />
            ) : (
              <Play size={13} weight="fill" />
            )}
            {busy ? "Starting…" : "Start session"}
          </Button>
        </div>

        <div className="mt-5 flex flex-wrap items-center justify-between gap-2 border-t border-line pt-4">
          <span className="mono text-[9.5px] text-faint">
            {snapshot.registry.source} · {snapshot.registry.models.toLocaleString()} models
          </span>
          <button
            type="button"
            onClick={() => {
              setRefreshing(true);
              onRefreshRegistry()
                .then(() => setModels({}))
                .catch((err: unknown) =>
                  setError(err instanceof Error ? err.message : "refresh failed"),
                )
                .finally(() => setRefreshing(false));
            }}
            disabled={refreshing}
            className="flex items-center gap-1 text-[10px] text-faint transition-colors hover:text-ink"
          >
            <ArrowClockwise
              size={11}
              weight="bold"
              className={refreshing ? "animate-spin" : undefined}
            />
            Refresh registry
          </button>
        </div>
      </motion.div>

      <Picker
        open={providerPicker}
        onOpenChange={setProviderPicker}
        title="Providers"
        description="Key status and model counts come from models.dev and your environment."
        items={providerItems}
        selected={providerId}
        placeholder="Search 309 providers"
        emptyLabel="No provider matches that search."
        onSelect={(id) => {
          setProviderId(id);
          setModelId(null);
          setKey("");
        }}
      />

      <Picker
        open={modelPicker}
        onOpenChange={setModelPicker}
        title={provider ? `${provider.name} models` : "Models"}
        description="Only models that support tool calling are listed."
        items={modelItems}
        selected={modelId ?? undefined}
        placeholder="Search models"
        emptyLabel="No model matches that search."
        onSelect={setModelId}
      />
    </div>
  );
}
