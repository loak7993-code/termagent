import { TerminalWindow } from "@phosphor-icons/react";

export function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <div className="flex items-center gap-2.5">
      <span className="grid size-8 place-items-center rounded-md bg-brand text-white">
        <TerminalWindow size={17} weight="bold" aria-hidden />
      </span>
      <span className="flex items-baseline gap-2">
        <span className="text-[15px] font-semibold tracking-tight text-ink">termagent</span>
        {!compact && (
          <span className="mono text-[10px] uppercase tracking-[0.12em] text-faint">2.0</span>
        )}
      </span>
    </div>
  );
}
