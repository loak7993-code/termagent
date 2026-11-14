import type { ReactNode } from "react";
import { CodeBlock } from "@aicss/react/code-block";

type Part =
  | { type: "text"; text: string }
  | { type: "code"; lang: string; code: string };

function parse(text: string): Part[] {
  const parts: Part[] = [];
  const fence = /```([\w+#.-]*)\n([\s\S]*?)```/g;
  let last = 0;
  let match: RegExpExecArray | null;
  while ((match = fence.exec(text)) !== null) {
    if (match.index > last) {
      parts.push({ type: "text", text: text.slice(last, match.index) });
    }
    parts.push({ type: "code", lang: match[1] || "text", code: match[2].replace(/\n$/, "") });
    last = match.index + match[0].length;
  }
  if (last < text.length) {
    parts.push({ type: "text", text: text.slice(last) });
  }
  return parts;
}

function inline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /(`[^`\n]+`|\*\*[^*\n]+\*\*)/g;
  let last = 0;
  let key = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > last) nodes.push(text.slice(last, match.index));
    const token = match[0];
    if (token.startsWith("`")) {
      nodes.push(
        <code
          key={`c${key++}`}
          className="mono rounded bg-white/[0.06] px-1.5 py-0.5 text-[0.92em] text-ink"
        >
          {token.slice(1, -1)}
        </code>,
      );
    } else {
      nodes.push(
        <strong key={`b${key++}`} className="font-semibold text-ink">
          {token.slice(2, -2)}
        </strong>,
      );
    }
    last = match.index + token.length;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

export function Markdown({ text }: { text: string }) {
  const parts = parse(text);
  return (
    <div className="space-y-3">
      {parts.map((part, index) =>
        part.type === "code" ? (
          <div key={index} className="overflow-hidden rounded-md border border-line/60">
            <CodeBlock lang={part.lang} code={part.code} />
          </div>
        ) : (
          part.text.trim() && (
            <p
              key={index}
              className="whitespace-pre-wrap text-[14.5px] leading-[1.72] text-ink/95"
            >
              {inline(part.text.trim())}
            </p>
          )
        ),
      )}
    </div>
  );
}
