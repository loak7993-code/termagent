// Full-screen Ink app: app-shell layout, chat feed, composer, overlays.
// All chrome comes from vendored termcn components.

import React, { useCallback, useEffect, useRef, useState } from "react";
import { Box, Text, useApp, useInput, useStdout } from "ink";

import { AppShell } from "../components/ui/app-shell.js";
import { Badge } from "../components/ui/badge.js";
import { Markdown } from "../components/ui/markdown.js";
import { ChatMessage } from "../components/ui/chat-message.js";
import { ToolCall } from "../components/ui/tool-call.js";
import { ToolApproval } from "../components/ui/tool-approval.js";
import { ModelSelector } from "../components/ui/model-selector.js";
import { TokenUsage } from "../components/ui/token-usage.js";
import { ThinkingBlock } from "../components/ui/thinking-block.js";
import { DiffView } from "../components/ui/diff-view.js";
import { Panel } from "../components/ui/panel.js";
import { StatusMessage } from "../components/ui/status-message.js";
import { ThemeProvider } from "../providers/theme-provider.js";
import { UnicodeProvider } from "../providers/unicode-provider.js";
import { useTheme } from "../hooks/use-theme.js";
import { getTheme } from "../lib/themes.js";
import type { FeedItem, Snapshot, Store, ToolCallItem } from "./store.js";

function ToolResultView({ call }: { call: ToolCallItem }) {
  const theme = useTheme();
  if (call.result === undefined) return null;
  const argEntries = Object.entries(call.args);
  return (
    <Box paddingLeft={2} flexDirection="column">
      {argEntries.length > 0 ? (
        <Box flexDirection="column" marginBottom={1}>
          {argEntries.map(([k, v]) => (
            <Box key={k} gap={1}>
              <Text color={theme.colors.accent}>{`${k}:`}</Text>
              <Text dimColor>{truncate(String(v).replace(/\n/g, " ⏎ "), 100)}</Text>
            </Box>
          ))}
        </Box>
      ) : null}
      {call.isError ? null : call.name === "edit" && call.args.old !== undefined ? (
        <DiffView
          oldText={String(call.args.old ?? "")}
          newText={String(call.args.new ?? "")}
          filename={String(call.args.path ?? "")}
          mode="unified"
          context={2}
        />
      ) : (
        <Text dimColor wrap="wrap">
          {truncate(call.result ?? "", 2000)}
        </Text>
      )}
    </Box>
  );
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n) + "…";
}

function FeedRow({ item }: { item: FeedItem }) {
  switch (item.kind) {
    case "user":
      return (
        <Box marginBottom={1} flexDirection="column">
          <ChatMessage sender="user">{item.text}</ChatMessage>
        </Box>
      );
    case "assistant":
      return (
        <Box marginBottom={1} flexDirection="column">
          <ChatMessage sender="assistant">
            <Markdown streaming={item.streaming}>{item.text}</Markdown>
          </ChatMessage>
        </Box>
      );
    case "tool": {
      const call = item.call;
      return (
        <Box marginBottom={1} flexDirection="column">
          <ToolCall
            name={call.name}
            args={call.args}
            status={call.status}
            duration={call.durationMs}
            collapsible={false}
          />
          {call.status !== "running" ? <ToolResultView call={call} /> : null}
        </Box>
      );
    }
    case "usage":
      return (
        <Box marginBottom={1}>
          <TokenUsage
            prompt={item.meta.promptTokens}
            completion={item.meta.completionTokens}
            model={item.meta.model}
          />
        </Box>
      );
    case "thinking":
      return (
        <Box marginBottom={1}>
          <ThinkingBlock
            content=""
            label="Thought"
            duration={Math.round(item.seconds * 1000)}
          />
        </Box>
      );
    case "notice":
      return (
        <Box marginBottom={1}>
          <StatusMessage variant="info">{item.text}</StatusMessage>
        </Box>
      );
    case "error":
      return (
        <Box marginBottom={1} flexDirection="column">
          <ChatMessage sender="error">{item.text}</ChatMessage>
        </Box>
      );
    case "panel":
      return (
        <Box marginBottom={1}>
          <Panel title={item.title} paddingX={1} borderStyle="round">
            {item.rows.map(([k, v]) => (
              <Box key={k} gap={1}>
                <Text bold>{`${k}:`}</Text>
                <Text dimColor wrap="wrap">
                  {v}
                </Text>
              </Box>
            ))}
          </Panel>
        </Box>
      );
  }
}

function ChatViewport({
  snap,
  height,
}: {
  snap: Snapshot;
  height: number;
}) {
  const total = snap.feed.length;
  const start = Math.max(0, total - height - snap.scrollOffset);
  const end = total - snap.scrollOffset;
  const items = snap.feed.slice(start, end);
  return (
    <Box flexDirection="column" height={height} overflow="hidden" paddingLeft={1} paddingTop={1}>
      {snap.scrollOffset > 0 ? (
        <Text dimColor>{`… scrolled up ${snap.scrollOffset} items (↓ to follow)`}</Text>
      ) : null}
      {items.map((item, i) => (
        <FeedRow key={`${start + i}-${item.kind}`} item={item} />
      ))}
      {total === 0 ? (
        <Text dimColor>No messages yet. Type a task below, or /models to pick a model.</Text>
      ) : null}
    </Box>
  );
}

function ModelSelectorOverlay({
  snap,
  groupLabel,
  onPick,
  onClose,
}: {
  snap: Snapshot;
  groupLabel: string;
  onPick: (id: string) => void;
  onClose: () => void;
}) {
  useInput((_ch, key) => {
    if (key.escape) onClose();
  });
  const maxRows = 12;
  const all = snap.modelChoices;
  const models = all.slice(0, maxRows).map((m) => ({
    id: m.id,
    name: m.label,
    provider: groupLabel === "theme" ? "termcn themes" : m.free ? "free" : "paid",
  }));
  const selected = groupLabel === "theme" ? snap.themeName : snap.model;
  const hidden = all.length - models.length;
  return (
    <Box flexDirection="column" paddingLeft={1}>
      <ModelSelector
        models={models}
        defaultValue={selected}
        showProvider
        groupByProvider
        autoFocus
        isActive
        onSelect={(id) => {
          onClose();
          onPick(id);
        }}
      />
      {hidden > 0 ? (
        <Text dimColor>{`… ${hidden} more — /model <id> sets any model directly`}</Text>
      ) : null}
      <Text dimColor> ↑↓ move · enter select · esc cancel</Text>
    </Box>
  );
}

function HelpOverlay({ onClose }: { onClose: () => void }) {
  useInput((_ch, key) => {
    if (key.escape) onClose();
  });
  const commands: Array<[string, string]> = [
    ["/help", "show this help"],
    ["/exit  /q", "quit (Ctrl-C also quits)"],
    ["/clear", "clear the conversation"],
    ["/models", "model picker (arrow keys + enter)"],
    ["/model <id>", "show or set the model"],
    ["/theme [name]", "switch UI theme (termcn themes)"],
    ["/config", "show current config"],
    ["/approve [on|off]", "toggle auto-approve of tool calls"],
    ["/save [id]", "save this session"],
    ["/load <id>", "load a saved session"],
    ["/sessions", "list saved sessions"],
    ["/cwd [path]", "show or change the working directory"],
    ["/tools", "list available tools"],
    ["/sys <text>", "append to the system message"],
    ["/paste", "multi-line input via $EDITOR"],
  ];
  return (
    <Box paddingLeft={1}>
      <Panel title="commands" paddingX={1} borderStyle="round">
        {commands.map(([cmd, desc]) => (
          <Box key={cmd} gap={2}>
            <Text bold>{cmd.padEnd(18)}</Text>
            <Text dimColor>{desc}</Text>
          </Box>
        ))}
        <Text dimColor>esc closes</Text>
      </Panel>
    </Box>
  );
}

export interface AppProps {
  store: Store;
  onSubmit: (text: string) => Promise<void> | void;
  onModelPick: (id: string) => void;
  onThemePick: (name: string) => void;
  onExit: () => void;
}

export function App(props: AppProps) {
  useStore(props.store);
  const snap = props.store.getSnapshot();
  return (
    <ThemeProvider theme={getTheme(snap.themeName)}>
      <UnicodeProvider>
        <AppInner {...props} snap={snap} />
      </UnicodeProvider>
    </ThemeProvider>
  );
}

function AppInner({ store, onSubmit, onModelPick, onThemePick, onExit, snap }: AppProps & { snap: Snapshot }) {
  const { stdout } = useStdout();
  const { exit } = useApp();
  const [input, setInput] = useState("");
  const theme = useTheme();
  const submitRef = useRef(onSubmit);
  submitRef.current = onSubmit;

  const rows = (stdout as unknown as { rows?: number }).rows ?? process.stdout.rows ?? 24;
  // chrome: header with bordered badges (~6) + hints (1) + margins (~2)
  const contentHeight = Math.max(4, rows - 8);

  const locked = snap.busy || snap.approval !== null || snap.overlay !== null;

  useInput((_ch, key) => {
    if (key.upArrow) store.scroll(1);
    else if (key.downArrow) store.scroll(-1);
    else if (key.pageUp) store.scroll(10);
    else if (key.pageDown) store.scroll(-10);
  });

  const handleSubmit = useCallback(
    (value: string) => {
      const text = value.trim();
      setInput("");
      if (!text) return;
      if (text === "/exit" || text === "/quit" || text === "/q") {
        onExit();
        return;
      }
      void submitRef.current(text);
    },
    [onExit],
  );

  useEffect(() => {
    if (snap.busy || snap.approval) setInput((v) => v);
  }, [snap.busy, snap.approval]);

  void exit;

  return (
    <Box flexDirection="column" height={rows} overflow="hidden" width={(stdout as unknown as { columns?: number }).columns ?? 80}>
      <AppShell>
      <AppShell.Header>
        <Box paddingLeft={1} flexDirection="row" gap={1}>
          <Text>
            <Text color={theme.colors.primary} bold>
              ❯_
            </Text>
            <Text bold> termagent</Text>
          </Text>
          <Badge variant="default">{snap.model}</Badge>
          <Badge variant="info">{snap.themeName}</Badge>
        </Box>
      </AppShell.Header>

      {snap.overlay ? null : <ChatViewport snap={snap} height={contentHeight} />}

      {snap.approval ? (
        <Box paddingLeft={1}>
          <ToolApproval
            name={snap.approval.kind}
            description={snap.approval.detail}
            risk={snap.approval.risk}
            onApprove={() => store.resolveApproval(true)}
            onDeny={() => store.resolveApproval(false)}
            onAlwaysAllow={() => store.resolveApproval(true, true)}
          />
        </Box>
      ) : snap.overlay === "models" || snap.overlay === "themes" ? (
        <Box flexDirection="column" height={contentHeight} overflow="hidden" paddingLeft={1}>
          <ModelSelectorOverlay
            snap={snap}
            groupLabel={snap.overlay === "themes" ? "theme" : "model"}
            onPick={snap.overlay === "themes" ? onThemePick : onModelPick}
            onClose={() => store.setOverlay(null)}
          />
        </Box>
      ) : snap.overlay === "help" ? (
        <Box flexDirection="column" height={contentHeight} overflow="hidden" paddingLeft={1}>
          <HelpOverlay onClose={() => store.setOverlay(null)} />
        </Box>
      ) : (
        <>
          <Box paddingLeft={1} paddingRight={1}>
            <AppShell.Input
              value={input}
              onValueChange={setInput}
              onSubmit={handleSubmit}
              placeholder={snap.busy ? "working…" : "type a task, /help for commands"}
              prefix="❯"
              borderStyle="round"
              readOnly={snap.busy}
              isActive={!locked}
              autoFocus={!locked}
              aria-label="Prompt"
            />
          </Box>
        </>
      )}

      <AppShell.Hints
        items={["/help commands · /models models · /theme themes · ↑↓ scroll · Ctrl-C quit"]}
      />
      </AppShell>
    </Box>
  );
}

function useStore(store: Store): void {
  const [, force] = React.useReducer((n: number) => n + 1, 0);
  useEffect(() => store.subscribe(() => force()), [store]);
}
