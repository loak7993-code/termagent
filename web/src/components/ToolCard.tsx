import { useState } from "react";
import { CaretDown, Check, CircleNotch, X } from "@phosphor-icons/react";
import type { ToolItem } from "@/lib/termagent";

const RISK_LABEL: Record<ToolItem["risk"], string> = {
  low: "low risk",
  medium: "medium risk",
  high: "high risk",
};

export function ToolCard({ item }: { item: ToolItem }) {
  const [open, setOpen] = useState(false);
  const lines = item.output ? item.output.split("\n") : [];
  const long = lines.length > 10;
  const shown = open || !long ? item.output : lines.slice(0, 8).join("\n");

  return (
    <div className="rounded-[10px] bg-white/[0.025] px-3.5 py-3">
      <div className="flex min-w-0 items-center gap-2.5">
        {item.running ? (
          <CircleNotch size={13} weight="bold" className="shrink-0 animate-spin text-brand" />
        ) : item.ok ? (
          <Check size={13} weight="bold" className="shrink-0 text-ok" />
        ) : (
          <X size={13} weight="bold" className="shrink-0 text-danger" />
        )}
        <span className="mono shrink-0 text-[12px] font-medium text-ink">{item.name}</span>
        <span className="mono min-w-0 flex-1 truncate text-[11.5px] text-dim">
          {item.summary}
        </span>
        {item.risk !== "low" && (
          <span
            className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium ${
              item.risk === "high"
                ? "bg-danger/12 text-danger"
                : "bg-warn/12 text-warn"
            }`}
          >
            {RISK_LABEL[item.risk]}
          </span>
        )}
        {!item.running && item.elapsed ? (
          <span className="mono shrink-0 text-[10.5px] text-faint">{item.elapsed} ms</span>
        ) : null}
      </div>

      {item.output && (
        <div className="mt-2.5">
          <pre
            className={`mono max-h-72 overflow-auto whitespace-pre-wrap rounded-md bg-black/35 px-3 py-2 text-[11.5px] leading-5 ${
              item.ok ? "text-dim" : "text-danger/90"
            }`}
          >
            {shown}
            {long && !open ? "\n…" : ""}
          </pre>
          {long && (
            <button
              type="button"
              onClick={() => setOpen((value) => !value)}
              className="mt-1.5 inline-flex items-center gap-1 text-[11px] text-faint transition-colors hover:text-ink"
            >
              <CaretDown
                size={11}
                weight="bold"
                className={open ? "rotate-180" : undefined}
              />
              {open ? "Collapse output" : `Show ${lines.length - 8} more lines`}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
