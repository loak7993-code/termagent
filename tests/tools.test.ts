import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { createToolContext, get, allTools, schemas, parseArgs } from "../src/agent/tools/base.js";
import "../src/agent/tools/index.js";
import { runRead, resolve } from "../src/agent/tools/read.js";
import { runLs } from "../src/agent/tools/ls.js";
import { runGlob } from "../src/agent/tools/glob.js";
import { runGrep } from "../src/agent/tools/grep.js";
import { runEdit } from "../src/agent/tools/edit.js";
import { runWrite } from "../src/agent/tools/write.js";
import { runTodo, clearTodos } from "../src/agent/tools/todo.js";

const dir = mkdtempSync(join(tmpdir(), "termagent-tools-"));
const ctx = createToolContext({ cwd: dir });

writeFileSync(join(dir, "hello.txt"), "alpha\nbeta\ngamma\n");
mkdirSync(join(dir, "sub"));
writeFileSync(join(dir, "sub", "deep.txt"), "needle in deep\n");

describe("tool registry", () => {
  it("registers all 8 tools", () => {
    expect(allTools().map((t) => t.name).sort()).toEqual(
      ["bash", "edit", "glob", "grep", "ls", "read", "todo", "write"].sort(),
    );
    expect(schemas().length).toBe(8);
    expect(get("read")?.read_only).toBe(true);
    expect(get("bash")?.read_only).toBe(false);
  });

  it("parses tool args from JSON strings", () => {
    expect(parseArgs('{"command": "ls"}')).toEqual({ command: "ls" });
    expect(parseArgs("")).toEqual({});
    expect(parseArgs("not json")).toEqual({ _raw: "not json" });
  });
});

describe("read", () => {
  it("numbers lines and pages with offset/limit", () => {
    const r = runRead("hello.txt", 0, 0, ctx);
    expect(r.content).toContain("1: alpha");
    expect(r.content).toContain("3: gamma");
    const paged = runRead("hello.txt", 2, 1, ctx);
    expect(paged.content).toContain("2: beta");
    expect(paged.content).not.toContain("1: alpha");
  });

  it("errors on missing files and directories", () => {
    expect(runRead("nope.txt", 1, 10, ctx).isError).toBe(true);
    expect(runRead("sub", 1, 10, ctx).isError).toBe(true);
  });

  it("resolves relative paths against cwd and ~", () => {
    expect(resolve(ctx, "hello.txt")).toBe(join(dir, "hello.txt"));
  });
});

describe("ls", () => {
  it("lists directories and stats files", () => {
    const listing = runLs(".", ctx);
    expect(listing.content).toContain("sub/");
    expect(listing.content).toContain("hello.txt");
    const file = runLs("hello.txt", ctx);
    expect(file.content).toContain("hello.txt");
    expect(file.content).toMatch(/^-rw-/);
  });
});

describe("glob", () => {
  it("matches recursively and skips .git", () => {
    const r = runGlob("**/*.txt", "", ctx);
    expect(r.content).toContain("hello.txt");
    expect(r.content).toContain("deep.txt");
    const none = runGlob("*.xyz", "", ctx);
    expect(none.content).toBe("(no matches)");
  });
});

describe("grep", () => {
  it("finds matches with path:line format", () => {
    const r = runGrep("needle", "", "", ctx);
    expect(r.content).toContain("needle in deep");
    expect(r.content).toMatch(/deep\.txt:1:/);
  });

  it("filters by include globs", () => {
    const r = runGrep("alpha", "", "*.txt", ctx);
    expect(r.content).toContain("alpha");
    const excluded = runGrep("alpha", "", "*.md", ctx);
    expect(excluded.content).toBe("(no matches)");
  });
});

describe("write + edit", () => {
  it("creates and then edits files with diffs in the result", async () => {
    const w = await runWrite("notes.md", "alpha\nalpha\nzeta\n", ctx);
    expect(w.isError).toBe(false);
    expect(w.content).toContain("created");

    const ambiguous = await runEdit("notes.md", "alpha", "beta", false, ctx);
    expect(ambiguous.isError).toBe(true);
    expect(ambiguous.content).toContain("matches 2 locations");

    const replaceAll = await runEdit("notes.md", "alpha", "beta", true, ctx);
    expect(replaceAll.isError).toBe(false);
    expect(replaceAll.content).toContain("+beta");

    const unique = await runEdit("notes.md", "zeta", "eta", false, ctx);
    expect(unique.isError).toBe(false);
    expect(unique.content).toContain("-zeta");
    expect(unique.content).toContain("+eta");
  });

  it("respects the approval gate", async () => {
    let approved = false;
    const denyCtx = createToolContext({ cwd: dir, approve: () => approved });
    const denied = await runWrite("blocked.txt", "x", denyCtx);
    expect(denied.isError).toBe(true);
    expect(denied.content).toContain("not approved");
    approved = true;
    const allowed = await runWrite("blocked.txt", "x", denyCtx);
    expect(allowed.isError).toBe(false);
  });
});

describe("todo", () => {
  it("sets, renders, and isolates per cwd", () => {
    const r = runTodo(
      "set",
      [
        { content: "first", status: "in_progress", priority: "high" },
        { content: "second", status: "pending", priority: "low" },
      ],
      ctx,
    );
    expect(r.content).toContain("[~] first !");
    expect(r.content).toContain("[ ] second ?");
    clearTodos(dir);
    expect(runTodo("list", null, ctx).content).toBe("(no todos)");
  });
});

afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

import { afterAll } from "vitest";
