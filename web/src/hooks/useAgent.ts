import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  itemsFromEvents,
  uid,
  type Approval,
  type Effort,
  type Item,
  type Snapshot,
  type StoredEvent,
  type ToolItem,
} from "@/lib/termagent";

interface WsEvent {
  type: string;
  state?: Snapshot;
  session?: Snapshot["session"];
  delta?: string;
  value?: string;
  id?: string;
  name?: string;
  tool?: string;
  summary?: string;
  args?: unknown;
  risk?: "low" | "medium" | "high";
  ok?: boolean;
  output?: string;
  duration_ms?: number;
  input?: number;
  output_tokens?: number;
  message?: string;
  busy?: boolean;
  provider?: string;
  model?: string;
  effort?: Effort;
  title?: string;
}

function appendAssistant(prev: Item[], text: string): Item[] {
  const last = prev[prev.length - 1];
  if (last && last.kind === "assistant" && last.streaming) {
    return [...prev.slice(0, -1), { ...last, text: last.text + text }];
  }
  return [...prev.map(closeStreaming), { kind: "assistant", id: uid(), text, streaming: true }];
}

function appendReasoning(prev: Item[], text: string): Item[] {
  const last = prev[prev.length - 1];
  if (last && last.kind === "reasoning" && last.live) {
    return [...prev.slice(0, -1), { ...last, text: last.text + text }];
  }
  return [...prev.map(closeStreaming), { kind: "reasoning", id: uid(), text, live: true }];
}

function closeStreaming(item: Item): Item {
  if (item.kind === "assistant") return { ...item, streaming: false };
  if (item.kind === "reasoning") return { ...item, live: false };
  return item;
}

function updateTool(prev: Item[], id: string, patch: Partial<ToolItem>): Item[] {
  return prev.map((item) =>
    item.kind === "tool" && item.id === id ? { ...item, ...patch } : item,
  );
}

export interface AgentApi {
  snapshot: Snapshot | null;
  items: Item[];
  pending: Approval | null;
  busy: boolean;
  streaming: boolean;
  connected: boolean;
  resolvedEffort: Effort | null;
  banner: string | null;
  loading: boolean;
  send: (text: string) => void;
  approve: (id: string, allow: boolean) => void;
  cancel: () => void;
  setModel: (provider: string, model: string, effort: Effort) => void;
  setEffort: (effort: Effort) => void;
  newSession: () => void;
  loadSession: (id: string) => void;
  renameSession: (id: string, title: string) => void;
  deleteSession: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
  refreshRegistry: () => Promise<void>;
  onboard: (input: {
    provider: string;
    model: string;
    api_key: string;
    effort: Effort;
  }) => Promise<void>;
  dismissBanner: () => void;
}

export function useAgent(): AgentApi {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [items, setItems] = useState<Item[]>([]);
  const [pending, setPending] = useState<Approval | null>(null);
  const [busy, setBusy] = useState(false);
  const [streaming, setStreaming] = useState(false);
  const [connected, setConnected] = useState(false);
  const [resolvedEffort, setResolvedEffort] = useState<Effort | null>(null);
  const [banner, setBanner] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const snapshotRef = useRef<Snapshot | null>(null);
  snapshotRef.current = snapshot;
  const wsRef = useRef<WebSocket | null>(null);
  const timerRef = useRef<number | null>(null);
  const attemptRef = useRef(0);
  const unmounted = useRef(false);
  const buffered = useRef({ text: "", reasoning: "" });
  const rafRef = useRef<number | null>(null);

  const flush = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
    const { text, reasoning } = buffered.current;
    if (!text && !reasoning) return;
    buffered.current = { text: "", reasoning: "" };
    setItems((prev) => {
      let next = prev;
      if (reasoning) next = appendReasoning(next, reasoning);
      if (text) next = appendAssistant(next, text);
      return next;
    });
  }, []);

  const schedule = useCallback(() => {
    if (rafRef.current !== null) return;
    rafRef.current = requestAnimationFrame(flush);
  }, [flush]);

  const refresh = useCallback(async () => {
    try {
      const next = await api<Snapshot>("/api/state");
      setSnapshot(next);
    } catch (error) {
      setBanner(error instanceof Error ? error.message : "cannot reach the server");
    }
  }, []);

  const handle = useCallback(
    (event: WsEvent) => {
      switch (event.type) {
        case "hello":
          if (event.state) {
            setSnapshot(event.state);
            setItems(itemsFromEvents(event.state.session?.events));
            setBusy(event.state.busy);
            setLoading(false);
          }
          break;
        case "text":
          buffered.current.text += event.delta ?? "";
          schedule();
          setStreaming(true);
          break;
        case "reasoning":
          buffered.current.reasoning += event.delta ?? "";
          schedule();
          break;
        case "tool_start":
          flush();
          setItems((prev) => [
            ...prev.map(closeStreaming),
            {
              kind: "tool",
              id: event.id ?? uid(),
              name: event.name ?? "tool",
              summary: event.summary ?? "",
              risk: event.risk ?? "low",
              running: true,
              ok: true,
              output: "",
              args: event.args,
              started: Date.now(),
            },
          ]);
          break;
        case "tool_result":
          setItems((prev) =>
            updateTool(prev, event.id ?? "", {
              running: false,
              ok: Boolean(event.ok),
              output: event.output ?? "",
              elapsed: event.duration_ms ?? 0,
            }),
          );
          break;
        case "approval":
          setPending({
            id: event.id ?? "",
            tool: event.tool ?? event.name ?? "tool",
            summary: event.summary ?? "",
            args: event.args,
            risk: event.risk ?? "medium",
          });
          setStreaming(false);
          break;
        case "effort":
          setResolvedEffort((event.value as Effort) ?? null);
          break;
        case "usage":
          setSnapshot((prev) =>
            prev
              ? { ...prev, totals: { input: event.input ?? 0, output: event.output_tokens ?? 0 } }
              : prev,
          );
          break;
        case "done":
          flush();
          setBusy(false);
          setStreaming(false);
          setPending(null);
          setItems((prev) => prev.map(closeStreaming));
          void refresh();
          break;
        case "busy":
          setBusy(Boolean(event.busy));
          if (!event.busy) setStreaming(false);
          break;
        case "session":
          if (event.session) {
            const session = event.session;
            setSnapshot((prev) => (prev ? { ...prev, session } : prev));
            if (session.events !== null) {
              setItems(itemsFromEvents(session.events));
              setBusy(false);
              setStreaming(false);
              setPending(null);
            }
          }
          break;
        case "model":
          setResolvedEffort(null);
          setSnapshot((prev) =>
            prev
              ? {
                  ...prev,
                  provider: event.provider ?? prev.provider,
                  model: event.model ?? prev.model,
                  effort: event.effort ?? prev.effort,
                }
              : prev,
          );
          break;
        case "renamed":
          setSnapshot((prev) =>
            prev
              ? {
                  ...prev,
                  sessions: prev.sessions.map((s) =>
                    s.id === event.id ? { ...s, title: event.title ?? s.title } : s,
                  ),
                  session:
                    prev.session && prev.session.id === event.id
                      ? { ...prev.session, title: event.title ?? prev.session.title }
                      : prev.session,
                }
              : prev,
          );
          break;
        case "error":
          setBanner(event.message ?? "something went wrong");
          void refresh();
          setBusy(false);
          setStreaming(false);
          setPending(null);
          setItems((prev) => [
            ...prev.map(closeStreaming),
            { kind: "error", id: uid(), text: event.message ?? "request failed" },
          ]);
          break;
      }
    },
    [flush, refresh, schedule],
  );

  const connect = useCallback(() => {
    if (unmounted.current) return;
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    const socket = new WebSocket(
      `${scheme}://${location.host}/ws?token=${encodeURIComponent(window.__TERMAGENT__?.token ?? "")}`,
    );
    wsRef.current = socket;
    socket.onopen = () => {
      attemptRef.current = 0;
      setConnected(true);
    };
    socket.onmessage = (message) => {
      try {
        handle(JSON.parse(message.data as string) as WsEvent);
      } catch {
        setBanner("received a malformed message");
      }
    };
    socket.onclose = () => {
      setConnected(false);
      if (unmounted.current) return;
      const delay = Math.min(500 * 2 ** attemptRef.current, 5000);
      attemptRef.current += 1;
      timerRef.current = window.setTimeout(connect, delay);
    };
    socket.onerror = () => {
      socket.close();
    };
  }, [handle]);

  useEffect(() => {
    unmounted.current = false;
    let cancelled = false;
    api<Snapshot>("/api/state")
      .then((state) => {
        if (cancelled) return;
        setSnapshot(state);
        setItems(itemsFromEvents(state.session?.events));
        setBusy(state.busy);
        setLoading(false);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setLoading(false);
        setBanner(
          error instanceof Error
            ? error.message
            : "cannot reach termagent. Start it with `termagent --web`.",
        );
      });
    connect();
    return () => {
      cancelled = true;
      unmounted.current = true;
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [connect]);

  const sendJson = useCallback((payload: unknown) => {
    const socket = wsRef.current;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      setBanner("not connected to termagent");
      return false;
    }
    socket.send(JSON.stringify(payload));
    return true;
  }, []);

  const send = useCallback(
    (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || busy) return;
      if (!sendJson({ type: "message", text: trimmed })) return;
      setItems((prev) => [
        ...prev.map(closeStreaming),
        { kind: "user", id: uid(), text: trimmed },
      ]);
      setBusy(true);
      setStreaming(true);
      setBanner(null);
      flush();
    },
    [busy, flush, sendJson],
  );

  const approve = useCallback(
    (id: string, allow: boolean) => {
      sendJson({ type: "approve", id, allow });
      setPending(null);
      setBusy(true);
      if (allow) setStreaming(true);
    },
    [sendJson],
  );

  const cancel = useCallback(() => {
    sendJson({ type: "cancel" });
  }, [sendJson]);

  const setModel = useCallback(
    (provider: string, model: string, effort: Effort) => {
      sendJson({ type: "set_model", provider, model, effort });
    },
    [sendJson],
  );

  const setEffort = useCallback(
    (effort: Effort) => {
      const state = snapshotRef.current;
      if (!state) return;
      sendJson({
        type: "set_model",
        provider: state.provider,
        model: state.model,
        effort,
      });
    },
    [sendJson],
  );

  const newSession = useCallback(() => {
    sendJson({ type: "new_session" });
    setItems([]);
    setPending(null);
    setBanner(null);
  }, [sendJson]);

  const loadSession = useCallback(
    (id: string) => {
      sendJson({ type: "load_session", id });
      setItems([]);
      setPending(null);
      setBanner(null);
    },
    [sendJson],
  );

  const renameSession = useCallback(
    (id: string, title: string) => {
      if (title.trim()) sendJson({ type: "rename", id, title: title.trim() });
    },
    [sendJson],
  );

  const deleteSession = useCallback(
    async (id: string) => {
      await api(`/api/sessions/${encodeURIComponent(id)}`, { method: "DELETE" });
      await refresh();
    },
    [refresh],
  );

  const refreshRegistry = useCallback(async () => {
    const next = await api<Snapshot>("/api/registry/refresh", { method: "POST" });
    setSnapshot(next);
  }, []);

  const onboard = useCallback(
    async (input: { provider: string; model: string; api_key: string; effort: Effort }) => {
      const next = await api<Snapshot>("/api/onboard", {
        method: "POST",
        body: JSON.stringify(input),
      });
      setSnapshot(next);
      setItems(itemsFromEvents(next.session?.events));
      setBusy(false);
      setBanner(null);
    },
    [],
  );

  return {
    snapshot,
    items,
    pending,
    busy,
    streaming,
    connected,
    resolvedEffort,
    banner,
    loading,
    send,
    approve,
    cancel,
    setModel,
    setEffort,
    newSession,
    loadSession,
    renameSession,
    deleteSession,
    refresh,
    refreshRegistry,
    onboard,
    dismissBanner: () => setBanner(null),
  };
}

export type { StoredEvent };
