import type { ReactNode } from "react";
import { motion } from "motion/react";
import { WarningCircle } from "@phosphor-icons/react";
import { ThinkingState } from "@aicss/react/thinking-state";
import type { Item } from "@/lib/termagent";
import { Markdown } from "@/components/Markdown";
import { ReasoningBlock } from "@/components/ReasoningBlock";
import { ToolCard } from "@/components/ToolCard";

interface MessageListProps {
  items: Item[];
  modelName: string;
  streaming: boolean;
}

function Label({ children }: { children: ReactNode }) {
  return (
    <div className="mono mb-1.5 text-[10px] uppercase tracking-[0.14em] text-faint">
      {children}
    </div>
  );
}

export function MessageList({ items, modelName, streaming }: MessageListProps) {
  return (
    <div className="space-y-5">
      {items.map((item, index) => {
        const last = index === items.length - 1;
        const enter = {
          initial: { opacity: 0, y: 6 },
          animate: { opacity: 1, y: 0 },
          transition: { duration: 0.2, ease: [0.16, 1, 0.3, 1] as const },
        };

        if (item.kind === "user") {
          return (
            <motion.div key={item.id} {...enter} className="pt-2">
              <Label>You</Label>
              <p className="whitespace-pre-wrap text-[14.5px] leading-[1.65] text-ink">
                {item.text}
              </p>
            </motion.div>
          );
        }

        if (item.kind === "assistant") {
          return (
            <motion.div key={item.id} {...enter}>
              <Label>{modelName}</Label>
              <Markdown text={item.text} />
              {item.streaming && last && streaming && (
                <span className="ml-0.5 inline-block h-[1.05em] w-[7px] translate-y-[2px] animate-pulse bg-brand" />
              )}
            </motion.div>
          );
        }

        if (item.kind === "reasoning") {
          return (
            <motion.div key={item.id} {...enter}>
              <ReasoningBlock text={item.text} live={item.live} />
            </motion.div>
          );
        }

        if (item.kind === "tool") {
          return (
            <motion.div key={item.id} {...enter}>
              <ToolCard item={item} />
            </motion.div>
          );
        }

        if (item.kind === "error") {
          return (
            <motion.div
              key={item.id}
              {...enter}
              className="flex items-start gap-2.5 rounded-md bg-danger/10 px-3.5 py-3"
            >
              <WarningCircle size={15} weight="bold" className="mt-0.5 shrink-0 text-danger" />
              <p className="text-[13px] leading-5 text-ink/90">{item.text}</p>
            </motion.div>
          );
        }

        return (
          <motion.p key={item.id} {...enter} className="mono text-[11.5px] leading-5 text-faint">
            {item.text}
          </motion.p>
        );
      })}
      {streaming && items[items.length - 1]?.kind === "user" && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className="flex items-center gap-2"
        >
          <ThinkingState />
        </motion.div>
      )}
    </div>
  );
}
