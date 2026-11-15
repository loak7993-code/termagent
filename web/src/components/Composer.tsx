import { useEffect, useRef, useState } from "react";
import { ArrowUp, Cpu, Square } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { effortLabel, type Effort, type Snapshot } from "@/lib/termagent";

interface ComposerProps {
  snapshot: Snapshot;
  busy: boolean;
  connected: boolean;
  resolvedEffort: Effort | null;
  onSend: (text: string) => void;
  onStop: () => void;
  onOpenModels: () => void;
  onEffort: (effort: Effort) => void;
}

export function Composer({
  snapshot,
  busy,
  connected,
  resolvedEffort,
  onSend,
  onStop,
  onOpenModels,
  onEffort,
}: ComposerProps) {
  const [value, setValue] = useState("");
  const area = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const node = area.current;
    if (!node) return;
    node.style.height = "0px";
    node.style.height = `${Math.min(node.scrollHeight, 208)}px`;
  }, [value]);

  useEffect(() => {
    if (!busy) area.current?.focus();
  }, [busy]);

  const submit = () => {
    if (!value.trim() || busy || !connected) return;
    onSend(value);
    setValue("");
  };

  const modelLabel = snapshot.modelName ?? snapshot.model;
  const effortText =
    snapshot.effort === "auto"
      ? resolvedEffort && resolvedEffort !== "auto"
        ? `Auto → ${effortLabel(resolvedEffort)}`
        : "Auto"
      : effortLabel(snapshot.effort);

  const nextEffort = (): Effort => {
    const order: Effort[] = ["auto", "low", "medium", "high", "max"];
    const index = order.indexOf(snapshot.effort);
    return order[(index + 1) % order.length];
  };

  return (
    <div className="mx-auto w-full max-w-[880px] px-3 pb-4 pt-2 md:px-6 md:pb-6">
      <div className="rounded-[12px] bg-panel transition-colors focus-within:bg-panel-2">
        <textarea
          ref={area}
          rows={1}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              if (event.nativeEvent.isComposing) return;
              event.preventDefault();
              submit();
            }
          }}
          placeholder={
            connected
              ? "Ask termagent to read, edit or run"
              : "Reconnecting to termagent…"
          }
          aria-label="Message termagent"
          disabled={!connected}
          className="block w-full resize-none bg-transparent px-4 pt-3.5 text-[14.5px] leading-6 text-ink outline-none placeholder:text-faint disabled:opacity-60"
        />
        <div className="flex items-center gap-1.5 px-2.5 pb-2.5 pt-1">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onOpenModels}
            className="mono h-7 max-w-[46%] gap-1.5 rounded-[6px] px-2 text-[11.5px] font-normal text-dim hover:text-ink"
            title="Model picker"
          >
            <Cpu size={13} weight="bold" />
            <span className="truncate">{modelLabel}</span>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onEffort(nextEffort())}
            className="h-7 rounded-[6px] px-2 text-[11.5px] font-normal text-dim hover:text-ink"
            title="Cycle reasoning effort"
          >
            {effortText}
          </Button>
          <span className="ml-auto mono hidden text-[10.5px] text-faint sm:block">
            {snapshot.totals.input + snapshot.totals.output > 0
              ? `${snapshot.totals.input.toLocaleString()} in / ${snapshot.totals.output.toLocaleString()} out`
              : "local only"}
          </span>
          {busy ? (
            <Button
              type="button"
              size="icon"
              onClick={onStop}
              aria-label="Stop the turn"
              className="ml-1 size-8 rounded-[6px] bg-white/10 text-ink hover:bg-white/15"
            >
              <Square size={12} weight="fill" />
            </Button>
          ) : (
            <Button
              type="button"
              size="icon"
              onClick={submit}
              disabled={!value.trim() || !connected}
              aria-label="Send"
              className="ml-1 size-8 rounded-[6px] bg-brand text-white transition-transform hover:bg-brand/90 active:scale-[0.96] disabled:bg-white/10 disabled:text-faint"
            >
              <ArrowUp size={14} weight="bold" />
            </Button>
          )}
        </div>
      </div>
      <div className="mt-2 flex items-center justify-between px-1">
        <p className="text-[10.5px] text-faint">
          Enter to send · Shift+Enter for a new line · ⌘K models
        </p>
        <p className="mono hidden text-[10px] text-faint sm:block">
          {connected ? `127.0.0.1:${window.__TERMAGENT__?.port ?? 6033}` : "offline"}
        </p>
      </div>
    </div>
  );
}
