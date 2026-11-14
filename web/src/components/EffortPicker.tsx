import { EFFORTS, type Effort, type ModelInfo } from "@/lib/termagent";

interface EffortPickerProps {
  value: Effort;
  model?: ModelInfo | null;
  onChange: (effort: Effort) => void;
  disabled?: boolean;
  resolved?: Effort | null;
}

export function EffortPicker({ value, model, onChange, disabled, resolved }: EffortPickerProps) {
  const allowed = model?.efforts?.length
    ? new Set(model.efforts.map((e) => e.toLowerCase()))
    : new Set(["low", "medium", "high"]);
  const reasoning = model?.reasoning ?? true;

  return (
    <div className="space-y-1.5">
      <div
        className="inline-flex rounded-md bg-white/[0.04] p-0.5"
        role="group"
        aria-label="Reasoning effort"
      >
        {EFFORTS.map((effort) => {
          const needsModel = effort !== "auto";
          const isDisabled =
            disabled || (needsModel && (!reasoning || !allowed.has(effort)));
          const active = effort === value;
          return (
            <button
              key={effort}
              type="button"
              disabled={isDisabled}
              aria-pressed={active}
              title={
                effort === "auto"
                  ? "termagent picks the level per message"
                  : isDisabled
                    ? "this model does not expose that level"
                    : undefined
              }
              onClick={() => onChange(effort)}
              className={`rounded-[6px] px-2.5 py-1 text-[11.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-35 ${
                active
                  ? "bg-white/[0.09] text-ink"
                  : "text-dim hover:text-ink hover:bg-white/[0.04]"
              }`}
            >
              {effort === "auto" ? "Auto" : effort.charAt(0).toUpperCase() + effort.slice(1)}
            </button>
          );
        })}
      </div>
      <p className="text-[11px] leading-4 text-faint">
        {value === "auto"
          ? `Auto picks low, medium or high per message${resolved ? ` (now ${resolved})` : ""}.`
          : "Sent with every request. Higher levels cost more tokens."}
      </p>
    </div>
  );
}
