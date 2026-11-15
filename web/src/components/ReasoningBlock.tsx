import { useEffect, useState } from "react";
import { ThinkingState } from "@aicss/react/thinking-state";
import { CaretDown } from "@phosphor-icons/react";

export function ReasoningBlock({ text, live }: { text: string; live: boolean }) {
  const [open, setOpen] = useState(live);
  useEffect(() => {
    if (live) setOpen(true);
  }, [live]);
  return (
    <div className="overflow-hidden rounded-md bg-white/[0.02]">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        className="flex w-full items-center gap-2 px-3 py-2 text-left text-[11.5px] text-dim transition-colors hover:bg-white/[0.03]"
      >
        {live ? <ThinkingState /> : <span className="text-dim">Reasoning</span>}
        <span className="mono text-[10.5px] text-faint">
          {text.length.toLocaleString()} chars
        </span>
        <CaretDown
          size={12}
          weight="bold"
          className={`ml-auto text-faint transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>
      {open && (
        <pre className="mono max-h-72 overflow-auto whitespace-pre-wrap border-t border-line/60 px-3 py-2.5 text-[11.5px] leading-5 text-dim/85">
          {text}
        </pre>
      )}
    </div>
  );
}
