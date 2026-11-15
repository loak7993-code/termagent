import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowClockwise,
  Check,
  CircleNotch,
  MagnifyingGlass,
  Plus,
} from "@phosphor-icons/react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EffortPicker } from "@/components/EffortPicker";
import { compact, price, orderProviders, type Effort, type ModelInfo, type Snapshot } from "@/lib/termagent";
import { api } from "@/lib/termagent";

interface ModelDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  snapshot: Snapshot;
  onUse: (provider: string, model: string, effort: Effort) => void;
  onRefreshRegistry: () => Promise<void>;
  onSetupProvider: (providerId: string) => void;
}

export function ModelDialog({
  open,
  onOpenChange,
  snapshot,
  onUse,
  onRefreshRegistry,
  onSetupProvider,
}: ModelDialogProps) {
  const [providerId, setProviderId] = useState(snapshot.provider);
  const [providerQuery, setProviderQuery] = useState("");
  const [modelQuery, setModelQuery] = useState("");
  const [models, setModels] = useState<Record<string, ModelInfo[]>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<string | null>(snapshot.model);
  const [effort, setEffort] = useState<Effort>(snapshot.effort);
  const [refreshing, setRefreshing] = useState(false);
  const [mobilePane, setMobilePane] = useState<"providers" | "models">("models");

  useEffect(() => {
    if (!open) return;
    setProviderId(snapshot.provider);
    setDraft(snapshot.model);
    setEffort(snapshot.effort);
    setModelQuery("");
    setProviderQuery("");
    setError(null);
    setMobilePane("models");
  }, [open, snapshot.provider, snapshot.model, snapshot.effort]);

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
    if (!open || !providerId) return;
    if (!models[providerId]) void load(providerId);
  }, [open, providerId, models, load]);

  const providers = useMemo(() => {
    const needle = providerQuery.trim().toLowerCase();
    const ordered = orderProviders(snapshot.providers, snapshot.provider);
    if (!needle) return ordered;
    return ordered.filter((provider) =>
      `${provider.name} ${provider.id}`.toLowerCase().includes(needle),
    );
  }, [providerQuery, snapshot.providers, snapshot.provider]);

  const provider = snapshot.providers.find((p) => p.id === providerId);
  const providerModels = useMemo(() => models[providerId] ?? [], [models, providerId]);
  const filteredModels = useMemo(() => {
    const needle = modelQuery.trim().toLowerCase();
    if (!needle) return providerModels;
    return providerModels.filter((model) =>
      `${model.name} ${model.id}`.toLowerCase().includes(needle),
    );
  }, [modelQuery, providerModels]);

  const draftModel = providerModels.find((model) => model.id === draft) ?? null;

  const apply = () => {
    if (!draft || !providerId) return;
    onUse(providerId, draft, effort);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(640px,88vh)] w-[min(980px,calc(100vw-2rem))] max-w-[980px] flex-col gap-0 overflow-hidden p-0 sm:max-w-[980px]">
        <DialogHeader className="flex-row items-center justify-between gap-4 space-y-0 border-b border-line px-5 py-4">
          <div>
            <DialogTitle className="text-[15px]">Models</DialogTitle>
            <DialogDescription className="mono mt-0.5 text-[10.5px] text-faint">
              {snapshot.registry.providers}{" "}
              {snapshot.registry.providers === 1 ? "provider" : "providers"} ·{" "}
              {snapshot.registry.models.toLocaleString()} models · {snapshot.registry.source}
            </DialogDescription>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1.5 rounded-[6px] px-2 text-[11.5px] text-dim hover:text-ink"
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
            >
              {refreshing ? (
                <CircleNotch size={13} weight="bold" className="animate-spin" />
              ) : (
                <ArrowClockwise size={13} weight="bold" />
              )}
              Refresh
            </Button>
          </div>
        </DialogHeader>

        {error && (
          <div className="border-b border-line bg-danger/10 px-5 py-2 text-[12px] text-danger">
            {error}
          </div>
        )}

        <div className="flex gap-1 border-b border-line px-3 py-2 md:hidden">
          {(["providers", "models"] as const).map((pane) => (
            <button
              key={pane}
              type="button"
              onClick={() => setMobilePane(pane)}
              aria-pressed={mobilePane === pane}
              className={`rounded-[6px] px-3 py-1.5 text-[11.5px] font-medium capitalize transition-colors ${
                mobilePane === pane ? "bg-white/[0.08] text-ink" : "text-dim hover:text-ink"
              }`}
            >
              {pane}
            </button>
          ))}
        </div>
        <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[310px_1fr]">
          <div
            className={`min-h-0 flex-col border-b border-line md:flex md:border-b-0 md:border-r ${
              mobilePane === "providers" ? "flex" : "hidden"
            }`}
          >
            <div className="relative px-3 py-3">
              <MagnifyingGlass
                size={14}
                weight="bold"
                className="pointer-events-none absolute left-6 top-1/2 -translate-y-1/2 text-faint"
              />
              <Input
                value={providerQuery}
                onChange={(event) => setProviderQuery(event.target.value)}
                placeholder="Filter providers"
                aria-label="Filter providers"
                className="h-8 border-0 bg-white/[0.04] pl-8 text-[12px] focus-visible:ring-2 focus-visible:ring-brand/60"
              />
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              {providers.map((entry) => {
                const active = entry.id === providerId;
                return (
                  <button
                    key={entry.id}
                    type="button"
                    onClick={() => {
                      setProviderId(entry.id);
                      setDraft(null);
                      setModelQuery("");
                      setEffort("auto");
                      setMobilePane("models");
                    }}
                    className={`mb-0.5 flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left transition-colors ${
                      active ? "bg-white/[0.07]" : "hover:bg-white/[0.04]"
                    }`}
                  >
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1.5">
                        <span className="truncate text-[12.5px] font-medium text-ink">
                          {entry.name}
                        </span>
                        {entry.envSet && (
                          <span
                            className="size-1.5 shrink-0 rounded-full bg-ok"
                            title="key found in your environment"
                          />
                        )}
                        {entry.local ? (
                          <span className="mono shrink-0 text-[9px] text-faint">local</span>
                        ) : (
                          entry.keySet &&
                          !entry.envSet && (
                            <Check size={11} weight="bold" className="shrink-0 text-ok" />
                          )
                        )}
                      </span>
                      <span className="mono block truncate text-[10px] text-faint">
                        {entry.id} · {entry.models} models
                      </span>
                    </span>
                  </button>
                );
              })}
              {providers.length === 0 && (
                <p className="px-2 py-6 text-center text-[11.5px] text-faint">
                  No provider matches that filter.
                </p>
              )}
            </div>
          </div>

          <div
            className={`min-h-0 flex-col md:flex ${
              mobilePane === "models" ? "flex" : "hidden"
            }`}
          >
            <div className="flex items-center gap-2 border-b border-line px-3 py-3">
              <div className="relative min-w-0 flex-1">
                <MagnifyingGlass
                  size={14}
                  weight="bold"
                  className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-faint"
                />
                <Input
                  value={modelQuery}
                  onChange={(event) => setModelQuery(event.target.value)}
                  placeholder={provider ? `Search ${provider.name} models` : "Search models"}
                  aria-label="Search models"
                  className="h-8 border-0 bg-white/[0.04] pl-8 text-[12px] focus-visible:ring-2 focus-visible:ring-brand/60"
                />
              </div>
              <span className="mono shrink-0 text-[10px] text-faint">
                {filteredModels.length} shown
              </span>
            </div>

            {provider && !provider.keySet && !provider.local && (
              <div className="flex items-center gap-2 border-b border-line bg-white/[0.02] px-4 py-2">
                <p className="text-[11.5px] text-dim">
                  No key for {provider.name} yet.
                </p>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 gap-1 rounded-[5px] px-1.5 text-[11px] text-brand hover:text-brand"
                  onClick={() => {
                    onOpenChange(false);
                    onSetupProvider(provider.id);
                  }}
                >
                  <Plus size={11} weight="bold" /> Add a key
                </Button>
              </div>
            )}

            <div className="min-h-0 flex-1 overflow-y-auto p-2">
              {loading && providerModels.length === 0 && (
                <div className="space-y-2 p-2">
                  {Array.from({ length: 8 }).map((_, index) => (
                    <div
                      key={index}
                      className="h-11 animate-pulse rounded-md bg-white/[0.03]"
                    />
                  ))}
                </div>
              )}
              {!loading && filteredModels.length === 0 && (
                <p className="px-2 py-8 text-center text-[12px] text-faint">
                  {modelQuery ? "No model matches that search." : "No tool-capable models for this provider."}
                </p>
              )}
              {filteredModels.map((model) => {
                const selected = draft === model.id;
                return (
                  <button
                    key={model.id}
                    type="button"
                    onClick={() => {
                      setDraft(model.id);
                      if (!model.reasoning) setEffort("auto");
                    }}
                    className={`model-row mb-0.5 flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left transition-colors ${
                      selected ? "bg-brand-soft ring-1 ring-brand/40" : "hover:bg-white/[0.04]"
                    }`}
                  >
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1.5">
                        <span className="truncate text-[12.5px] font-medium text-ink">
                          {model.name}
                        </span>
                        {model.reasoning && (
                          <span className="shrink-0 rounded-full border border-brand/35 px-1.5 py-px text-[9px] font-medium text-brand">
                            reasoning
                          </span>
                        )}
                      </span>
                      <span className="mono mt-0.5 flex flex-wrap items-center gap-x-2 text-[10px] text-faint">
                        <span className="truncate">{model.id}</span>
                        {model.context > 0 && <span>{compact(model.context)} ctx</span>}
                        <span>{price(model)}</span>
                      </span>
                    </span>
                    {selected && <Check size={14} weight="bold" className="shrink-0 text-brand" />}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="flex flex-col gap-3 border-t border-line px-5 py-3.5 md:flex-row md:items-end md:justify-between">
          <div className="min-w-0">
            <p className="mono truncate text-[11px] text-dim">
              {draftModel ? draftModel.name : "Pick a model"}
              {draftModel && draftModel.context > 0 ? ` · ${compact(draftModel.context)} ctx` : ""}
            </p>
            <div className="mt-2">
              <EffortPicker
                value={effort}
                model={draftModel}
                onChange={setEffort}
                disabled={!draftModel}
              />
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button
              variant="ghost"
              size="sm"
              className="h-8 rounded-[6px] px-3 text-[12px]"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button
              size="sm"
              disabled={
                !draftModel ||
                Boolean(provider && !provider.keySet && !provider.local) ||
                Boolean(provider?.requiresProject && !snapshot.project)
              }
              title={
                provider && !provider.keySet && !provider.local
                  ? "Add a key for this provider first"
                  : provider?.requiresProject && !snapshot.project
                    ? "Open provider settings and set the team/project first"
                    : undefined
              }
              onClick={apply}
              className="h-8 rounded-[6px] bg-brand px-3 text-[12px] font-medium text-white hover:bg-brand/90"
            >
              Use model
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
