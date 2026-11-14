import { ApprovalCard } from "@aicss/react/approval-card";
import type { Approval } from "@/lib/termagent";

interface ApprovalPanelProps {
  approval: Approval;
  cwd: string;
  onAnswer: (allow: boolean) => void;
}

export function ApprovalPanel({ approval, cwd, onAnswer }: ApprovalPanelProps) {
  const risk =
    approval.risk === "high"
      ? "High risk"
      : approval.risk === "medium"
        ? "Medium risk"
        : "Low risk";
  const command = approval.summary.replace(
    new RegExp(`^${approval.tool}:\\s*`),
    "",
  );
  return (
    <div className="mx-auto w-full max-w-[880px] px-3 pt-2 md:px-6">
      <div className="mb-2 flex items-center gap-2">
        <span
          className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
            approval.risk === "high"
              ? "bg-danger/12 text-danger"
              : approval.risk === "medium"
                ? "bg-warn/12 text-warn"
                : "bg-white/[0.06] text-dim"
          }`}
        >
          {risk}
        </span>
        <span className="mono text-[10.5px] text-faint">
          {approval.tool} needs your approval
        </span>
      </div>
      <ApprovalCard
        variant="command"
        title={`Run ${approval.tool}?`}
        command={command}
        cwd={cwd}
        approveLabel="Run"
        rejectLabel="Deny"
        onApprove={() => onAnswer(true)}
        onReject={() => onAnswer(false)}
      />
    </div>
  );
}
