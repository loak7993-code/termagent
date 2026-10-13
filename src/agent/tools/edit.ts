// edit tool: exact text replacement with approval gate. Port of tools/edit.py.

import { readFileSync, writeFileSync } from "node:fs";

import { err, ok, register, type ToolContext } from "./base.js";
import { resolve } from "./read.js";

function unifiedDiff(oldText: string, newText: string, path: string, maxLines = 60): string {
  const a = oldText.split("\n");
  const b = newText.split("\n");
  // classic LCS diff, unified-format output
  const n = a.length;
  const m = b.length;
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? (lcs[i + 1]?.[j + 1] ?? 0) + 1 : Math.max(lcs[i + 1]?.[j] ?? 0, lcs[i]?.[j + 1] ?? 0);
    }
  }
  const hunks: string[] = [];
  let i = 0;
  let j = 0;
  const ctx = 3;
  const changes: Array<{ t: "-" | "+" | " "; line: string }> = [];
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      changes.push({ t: " ", line: a[i] });
      i++;
      j++;
    } else if ((lcs[i + 1]?.[j] ?? 0) >= (lcs[i]?.[j + 1] ?? 0)) {
      changes.push({ t: "-", line: a[i] });
      i++;
    } else {
      changes.push({ t: "+", line: b[j] });
      j++;
    }
  }
  while (i < n) {
    changes.push({ t: "-", line: a[i] });
    i++;
  }
  while (j < m) {
    changes.push({ t: "+", line: b[j] });
    j++;
  }
  if (!changes.some((c) => c.t !== " ")) return "";
  hunks.push(`--- a/${path}`, `+++ b/${path}`);
  // group changes into hunks with context
  let printed = 0;
  let k = 0;
  while (k < changes.length && printed < maxLines) {
    if (changes[k].t === " ") {
      k++;
      continue;
    }
    const start = Math.max(0, k - ctx);
    const end = Math.min(changes.length, k + ctx + 1);
    let s = k;
    while (s > 0 && changes[s - 1].t !== " ") s--;
    let e = k;
    while (e < changes.length && changes[e].t !== " ") e++;
    const hunkStart = Math.max(0, s - ctx);
    const hunkEnd = Math.min(changes.length, e + ctx);
    const slice = changes.slice(hunkStart, hunkEnd);
    const oldCount = slice.filter((c) => c.t !== "+").length;
    const newCount = slice.filter((c) => c.t !== "-").length;
    hunks.push(`@@ -${hunkStart + 1},${oldCount} +${hunkStart + 1},${newCount} @@`);
    for (const c of slice) {
      hunks.push(c.t + c.line);
      printed++;
      if (printed >= maxLines) {
        hunks.push("… (diff truncated)");
        break;
      }
    }
    k = hunkEnd;
  }
  return hunks.join("\n");
}

export function runEdit(
  path: string,
  oldText: string,
  newText: string,
  replaceAll: boolean,
  ctx: ToolContext,
): Promise<{ content: string; isError: boolean }> {
  if (!path) return Promise.resolve(err("error: path is required"));
  if (oldText === newText) {
    return Promise.resolve(err("error: old and new text are identical"));
  }
  const p = resolve(ctx, path);
  let text: string;
  try {
    text = readFileSync(p, "utf8");
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ENOENT") return Promise.resolve(err(`error: file not found: ${p}`));
    return Promise.resolve(err(`error: ${(e as Error).message}`));
  }

  const count = text.split(oldText).length - 1;
  if (count === 0) {
    return Promise.resolve(
      err(
        "error: oldString not found. Re-read the file; it may have changed or " +
          "use different whitespace/line endings.",
      ),
    );
  }
  if (count > 1 && !replaceAll) {
    return Promise.resolve(
      err(
        `error: oldString matches ${count} locations. Provide more context to ` +
          "make it unique, or set replace_all=true.",
      ),
    );
  }

  const newWhole = replaceAll
    ? text.split(oldText).join(newText)
    : text.replace(oldText, newText);

  const rel = path;
  const diff = unifiedDiff(text, newWhole, rel);

  const apply = (): { content: string; isError: boolean } => {
    try {
      writeFileSync(p, newWhole, "utf8");
    } catch (e) {
      return err(`error: ${(e as Error).message}`);
    }
    if (ctx.emit) {
      ctx.emit(`edit ${p} (${count} replacement${count === 1 ? "" : "s"})`);
    }
    const summary = `edited ${p} (${count} replacement${count === 1 ? "" : "s"})`;
    return ok(diff ? `${summary}\n${diff}` : summary);
  };

  if (ctx.approve) {
    const plural = count === 1 ? "" : "es";
    return Promise.resolve(ctx.approve("edit", `${p} (${count} match${plural})`)).then(
      (approved) => {
        if (!approved) return err("Edit not approved by user.");
        return apply();
      },
    );
  }
  return Promise.resolve(apply());
}

register({
  name: "edit",
  description:
    "Replace exact text in a file. oldString must be unique unless replace_all is true. Fails if oldString is absent or ambiguous.",
  params: {
    type: "object",
    properties: {
      path: { type: "string", description: "File path." },
      old: { type: "string", description: "Exact text to replace." },
      new: { type: "string", description: "Replacement text." },
      replace_all: { type: "boolean", description: "Replace all matches." },
    },
    required: ["path", "old", "new"],
  },
  read_only: false,
  run: (a, ctx) =>
    runEdit(
      String(a.path ?? ""),
      String(a.old ?? ""),
      String(a.new ?? ""),
      Boolean(a.replace_all),
      ctx,
    ),
});
