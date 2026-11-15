import { useState } from "react";
import { Cpu, GearSix, List, PencilSimple } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { effortLabel, type Effort, type Snapshot } from "@/lib/termagent";

interface TopBarProps {
  snapshot: Snapshot;
  connected: boolean;
  busy: boolean;
  resolvedEffort: Effort | null;
  title: string;
  onOpenSidebar: () => void;
  onOpenModels: () => void;
  onOpenOnboarding: () => void;
  onRename: (title: string) => void;
}

export function TopBar({
  snapshot,
  connected,
  busy,
  resolvedEffort,
  title,
  onOpenSidebar,
  onOpenModels,
  onOpenOnboarding,
  onRename,
}: TopBarProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(title);

  const effortText =
    snapshot.effort === "auto"
      ? resolvedEffort && resolvedEffort !== "auto"
        ? `Auto → ${effortLabel(resolvedEffort)}`
        : "Auto"
      : effortLabel(snapshot.effort);

  const modelLabel = snapshot.modelName ?? snapshot.model;

  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b border-line/70 px-3 md:px-5">
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onOpenSidebar}
        aria-label="Open sessions"
        className="md:hidden"
      >
        <List size={16} weight="bold" />
      </Button>

      <div className="min-w-0 flex-1">
        {editing ? (
          <input
            autoFocus
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onBlur={() => {
              if (draft.trim()) onRename(draft.trim());
              setEditing(false);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              } else if (event.key === "Escape") {
                setDraft(title);
                setEditing(false);
              }
            }}
            className="h-8 w-full max-w-[420px] rounded-[6px] bg-white/[0.06] px-2 text-[13px] text-ink outline-none ring-1 ring-brand/40"
            aria-label="Session title"
          />
        ) : (
          <button
            type="button"
            onDoubleClick={() => {
              setDraft(title);
              setEditing(true);
            }}
            title="Double click to rename"
            className="group flex max-w-full items-center gap-1.5 truncate rounded px-1 py-0.5 text-left text-[13px] font-medium text-ink/90 hover:text-ink"
          >
            <span className="truncate">{title}</span>
            <PencilSimple
              size={11}
              weight="bold"
              className="shrink-0 text-faint opacity-0 transition-opacity group-hover:opacity-100"
            />
          </button>
        )}
      </div>

      <div className="flex items-center gap-1.5">
        <span
          className="mr-1 flex items-center gap-1.5"
          title={connected ? "connected" : "reconnecting"}
        >
          <span
            className={`size-1.5 rounded-full ${
              connected ? (busy ? "animate-pulse bg-brand" : "bg-ok") : "bg-warn"
            }`}
          />
          <span className="mono hidden text-[10px] text-faint sm:block">
            {connected ? (busy ? "working" : "ready") : "offline"}
          </span>
        </span>

        <Button
          variant="ghost"
          size="sm"
          onClick={onOpenModels}
          className="mono hidden h-7 max-w-[220px] gap-1.5 rounded-[6px] px-2 text-[11px] font-normal text-dim hover:text-ink sm:flex"
          title="Model picker"
        >
          <Cpu size={13} weight="bold" />
          <span className="truncate">{modelLabel}</span>
        </Button>
        <span className="mono hidden rounded-full bg-white/[0.05] px-2.5 py-1 text-[10.5px] text-dim md:block">
          {effortText}
        </span>
        <span className="mono hidden text-[10.5px] text-faint lg:block">
          {snapshot.totals.input + snapshot.totals.output > 0
            ? `${snapshot.totals.input.toLocaleString()} in / ${snapshot.totals.output.toLocaleString()} out`
            : ""}
        </span>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onOpenOnboarding}
          aria-label="Providers and keys"
          title="Providers and keys"
        >
          <GearSix size={15} weight="bold" />
        </Button>
      </div>
    </header>
  );
}
