// todo tool: per-cwd plan tracking. Port of tools/todo.py.

import { ok, register, type ToolContext } from "./base.js";

const VALID = new Set(["pending", "in_progress", "completed", "cancelled"]);
const PRIORITY = new Set(["high", "medium", "low"]);

const state = new Map<string, Array<Record<string, unknown>>>();

function store(ctx: ToolContext): Array<Record<string, unknown>> {
  let s = state.get(ctx.cwd);
  if (!s) {
    s = [];
    state.set(ctx.cwd, s);
  }
  return s;
}

function render(items: Array<Record<string, unknown>>): string {
  if (items.length === 0) return "(no todos)";
  const sym: Record<string, string> = {
    pending: "[ ]",
    in_progress: "[~]",
    completed: "[x]",
    cancelled: "[-]",
  };
  const pr: Record<string, string> = { high: "!", medium: "", low: "?" };
  return items
    .map((t) => {
      const status = String(t.status);
      const priority = String(t.priority);
      const mark = pr[priority] ?? "";
      const suffix = mark ? ` ${mark}` : "";
      return `${sym[status] ?? "[ ]"} ${String(t.content)}${suffix}`;
    })
    .join("\n");
}

interface TodoInput {
  content?: unknown;
  status?: unknown;
  priority?: unknown;
}

export function runTodo(
  action: string,
  todos: TodoInput[] | null,
  ctx: ToolContext,
): { content: string; isError: boolean } {
  const s = store(ctx);
  const act = (action ?? "").trim();
  if (act === "list") return ok(render(s));
  if (act === "set") {
    if (!Array.isArray(todos)) return { content: "error: 'todos' must be a list", isError: true };
    s.length = 0;
    todos.forEach((item, i) => {
      if (typeof item !== "object" || item === null) return;
      const content = String(item.content ?? "").trim();
      if (!content) return;
      let status = String(item.status ?? "pending");
      if (!VALID.has(status)) status = "pending";
      let priority = String(item.priority ?? "medium");
      if (!PRIORITY.has(priority)) priority = "medium";
      s.push({ content, status, priority, order: i });
    });
    return ok(render(s));
  }
  return {
    content:
      "error: action must be 'list' or 'set'. Use 'set' with a full todos list " +
      "(content, status, priority) to replace the whole plan.",
    isError: true,
  };
}

register({
  name: "todo",
  description:
    "Track a task plan. action=list shows the plan; action=set replaces it with a full list of {content, status, priority}.",
  params: {
    type: "object",
    properties: {
      action: { type: "string", description: "'list' or 'set'." },
      todos: {
        type: "array",
        description:
          "For action=set: list of {content, status: pending|in_progress|completed|cancelled, priority: high|medium|low}.",
        items: { type: "object" },
      },
    },
    required: ["action"],
  },
  read_only: false,
  run: (a, ctx) =>
    runTodo(String(a.action ?? ""), (a.todos as TodoInput[] | undefined) ?? null, ctx),
});

export function clearTodos(cwd: string): void {
  state.delete(cwd);
}
