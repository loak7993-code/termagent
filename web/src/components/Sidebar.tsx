import { useState } from "react";
import {
  ChatCircleText,
  Cpu,
  DotsThree,
  GearSix,
  PencilSimple,
  Plus,
  Trash,
  X,
} from "@phosphor-icons/react";
import { motion } from "motion/react";
import { Brand } from "@/components/Brand";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { groupDay, timeAgo, type SessionMeta, type Snapshot } from "@/lib/termagent";

interface SidebarProps {
  snapshot: Snapshot;
  activeId: string | null;
  onNew: () => void;
  onSelect: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onDelete: (id: string) => Promise<void>;
  onOpenModels: () => void;
  onOpenOnboarding: () => void;
  onClose?: () => void;
}

export function Sidebar({
  snapshot,
  activeId,
  onNew,
  onSelect,
  onRename,
  onDelete,
  onOpenModels,
  onOpenOnboarding,
  onClose,
}: SidebarProps) {
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [confirming, setConfirming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const groups: Array<{ label: string; sessions: SessionMeta[] }> = [];
  for (const session of snapshot.sessions) {
    const label = groupDay(session.created);
    const group = groups.find((entry) => entry.label === label);
    if (group) group.sessions.push(session);
    else groups.push({ label, sessions: [session] });
  }

  const modelLabel = snapshot.modelName ?? snapshot.model;
  const provider = snapshot.providers.find((p) => p.id === snapshot.provider);

  return (
    <aside className="flex h-dvh w-[276px] shrink-0 flex-col border-r border-line/70 bg-[#0a0a0c]">
      <div className="flex items-center justify-between px-4 pb-2 pt-4">
        <Brand />
        {onClose && (
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onClose}
            aria-label="Close sidebar"
            className="md:hidden"
          >
            <X size={15} weight="bold" />
          </Button>
        )}
      </div>

      <div className="px-3 pb-3 pt-2">
        <Button
          onClick={onNew}
          className="h-9 w-full justify-start gap-2 rounded-md bg-brand px-3 text-[13px] font-medium text-white hover:bg-brand/90 active:scale-[0.99]"
        >
          <Plus size={14} weight="bold" />
          New session
        </Button>
      </div>

      <nav className="min-h-0 flex-1 overflow-y-auto px-2 pb-2" aria-label="Sessions">
        {snapshot.sessions.length === 0 && (
          <div className="mx-2 mt-2 rounded-md bg-white/[0.025] px-3 py-4">
            <ChatCircleText size={16} weight="bold" className="mb-2 text-faint" />
            <p className="text-[12px] leading-5 text-dim">
              No sessions yet. Send the first prompt to start one.
            </p>
          </div>
        )}
        {groups.map((group) => (
          <div key={group.label} className="mb-3">
            <p className="mono px-2 py-1.5 text-[9.5px] uppercase tracking-[0.16em] text-faint">
              {group.label}
            </p>
            {group.sessions.map((session) => {
              const active = session.id === activeId;
              return (
                <div key={session.id} className="group/session relative">
                  {editing === session.id ? (
                    <div className="px-1 py-0.5">
                      <input
                        autoFocus
                        value={draft}
                        onChange={(event) => setDraft(event.target.value)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            onRename(session.id, draft);
                            setEditing(null);
                          } else if (event.key === "Escape") {
                            setEditing(null);
                          }
                        }}
                        onBlur={() => {
                          if (draft.trim()) onRename(session.id, draft);
                          setEditing(null);
                        }}
                        className="h-8 w-full rounded-[6px] bg-white/[0.06] px-2 text-[12.5px] text-ink outline-none ring-1 ring-brand/40"
                        aria-label="Session title"
                      />
                    </div>
                  ) : (
                    <button
                      type="button"
                      onClick={() => {
                        onSelect(session.id);
                        onClose?.();
                      }}
                      className={`flex w-full flex-col items-start gap-0.5 rounded-md px-2.5 py-2 text-left transition-colors ${
                        active
                          ? "bg-white/[0.07]"
                          : "hover:bg-white/[0.04]"
                      }`}
                    >
                      <span className="flex w-full items-center gap-1.5">
                        <span
                          className={`truncate text-[12.5px] ${
                            active ? "text-ink" : "text-ink/80"
                          }`}
                        >
                          {session.title || "New session"}
                        </span>
                        {active && (
                          <span className="ml-auto size-1.5 shrink-0 rounded-full bg-brand" />
                        )}
                      </span>
                      <span className="mono w-full truncate text-[10px] text-faint">
                        {session.model} · {timeAgo(session.created)}
                      </span>
                    </button>
                  )}

                  {confirming === session.id ? (
                    <div className="mt-1 flex items-center gap-1.5 rounded-md bg-danger/10 px-2 py-1.5">
                      <span className="text-[10.5px] text-danger">Delete this session?</span>
                      <button
                        type="button"
                        onClick={() => {
                          setConfirming(null);
                          onDelete(session.id).catch((err: unknown) =>
                            setError(err instanceof Error ? err.message : "delete failed"),
                          );
                        }}
                        className="ml-auto rounded-[5px] bg-danger px-1.5 py-0.5 text-[10px] font-medium text-white"
                      >
                        Delete
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirming(null)}
                        className="rounded-[5px] px-1.5 py-0.5 text-[10px] text-dim hover:text-ink"
                      >
                        Keep
                      </button>
                    </div>
                  ) : (
                    editing !== session.id && (
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <button
                            type="button"
                            aria-label="Session actions"
                            className="absolute right-1.5 top-1.5 grid size-6 place-items-center rounded-[5px] text-faint opacity-100 transition-opacity hover:bg-white/[0.08] hover:text-ink focus-visible:opacity-100 md:opacity-0 md:group-hover/session:opacity-100"
                          >
                            <DotsThree size={16} weight="bold" />
                          </button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end" side="bottom" className="w-40">
                          <DropdownMenuItem
                            onClick={() => {
                              setDraft(session.title);
                              setEditing(session.id);
                            }}
                          >
                            <PencilSimple size={13} weight="bold" /> Rename
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            variant="destructive"
                            onClick={() => setConfirming(session.id)}
                          >
                            <Trash size={13} weight="bold" /> Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    )
                  )}
                </div>
              );
            })}
          </div>
        ))}
        {error && <p className="px-2 py-1 text-[11px] text-danger">{error}</p>}
      </nav>

      <motion.footer
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.15 }}
        className="border-t border-line/70 p-3"
      >
        <button
          type="button"
          onClick={onOpenModels}
          className="flex w-full items-center gap-2.5 rounded-md bg-white/[0.035] px-2.5 py-2 text-left transition-colors hover:bg-white/[0.06]"
          aria-label="Choose a model"
        >
          <Cpu size={15} weight="bold" className="shrink-0 text-brand" />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[12px] font-medium text-ink">{modelLabel}</span>
            <span className="mono block truncate text-[10px] text-faint">
              {provider?.name ?? snapshot.provider} · {snapshot.effort}
            </span>
          </span>
        </button>
        <div className="mt-2 flex items-center justify-between px-1">
          <span className="mono text-[9.5px] text-faint">
            v{snapshot.version} · models.dev
          </span>
          <button
            type="button"
            onClick={onOpenOnboarding}
            title="Providers and keys"
            aria-label="Providers and keys"
            className="grid size-6 place-items-center rounded-[5px] text-faint transition-colors hover:bg-white/[0.06] hover:text-ink"
          >
            <GearSix size={14} weight="bold" />
          </button>
        </div>
      </motion.footer>
    </aside>
  );
}
