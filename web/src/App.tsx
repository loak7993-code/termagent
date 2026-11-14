import { useEffect, useRef, useState } from "react";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { ArrowClockwise, ArrowRight, X } from "@phosphor-icons/react";
import { ApprovalPanel } from "@/components/ApprovalPanel";
import { Brand } from "@/components/Brand";
import { Composer } from "@/components/Composer";
import { MessageList } from "@/components/MessageList";
import { ModelDialog } from "@/components/ModelDialog";
import { Onboarding } from "@/components/Onboarding";
import { Sidebar } from "@/components/Sidebar";
import { TopBar } from "@/components/TopBar";
import { Button } from "@/components/ui/button";
import { useAgent } from "@/hooks/useAgent";
import { booted } from "@/lib/termagent";

function NotServed() {
  return (
    <div className="relative grid min-h-dvh place-items-center px-6">
      <div className="haze" />
      <div className="relative z-10 w-full max-w-[480px] rounded-[14px] bg-panel p-7">
        <Brand />
        <h1 className="mt-6 text-[20px] font-medium text-ink">Start the local server</h1>
        <p className="mt-2 text-[13px] leading-6 text-dim">
          This page is a static build. The chat needs the termagent process that serves it.
        </p>
        <pre className="mono mt-4 rounded-md bg-black/40 px-3 py-2.5 text-[12px] text-ink">
          termagent --web
        </pre>
        <p className="mt-3 text-[11.5px] leading-5 text-faint">
          Then open http://127.0.0.1:6033. The server binds to loopback only and mints a fresh
          token each boot.
        </p>
      </div>
    </div>
  );
}

function Boot() {
  return (
    <div className="flex min-h-dvh">
      <div className="hidden w-[276px] shrink-0 border-r border-line/70 bg-[#0a0a0c] p-4 md:block">
        <div className="h-8 w-32 animate-pulse rounded bg-white/[0.05]" />
        <div className="mt-6 h-9 w-full animate-pulse rounded-md bg-white/[0.04]" />
        <div className="mt-6 space-y-2">
          {Array.from({ length: 6 }).map((_, index) => (
            <div key={index} className="h-10 animate-pulse rounded-md bg-white/[0.03]" />
          ))}
        </div>
      </div>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="h-14 border-b border-line/70" />
        <div className="mx-auto w-full max-w-[880px] flex-1 space-y-4 px-6 py-8">
          {Array.from({ length: 4 }).map((_, index) => (
            <div key={index} className="h-20 animate-pulse rounded-md bg-white/[0.03]" />
          ))}
        </div>
        <div className="mx-auto w-full max-w-[880px] px-6 pb-6">
          <div className="h-24 animate-pulse rounded-[12px] bg-panel" />
        </div>
      </div>
    </div>
  );
}

function Suggestions({ onPick }: { onPick: (text: string) => void }) {
  const prompts = [
    "Read this repo and explain how it is put together",
    "Run the tests and fix whatever fails",
    "Add a --json flag to the CLI",
  ];
  return (
    <div className="pt-8">
      <p className="mono mb-3 text-[10px] uppercase tracking-[0.16em] text-faint">
        Start here
      </p>
      <div className="space-y-2">
        {prompts.map((prompt) => (
          <button
            key={prompt}
            type="button"
            onClick={() => onPick(prompt)}
            className="group flex w-full items-center gap-3 rounded-md bg-white/[0.025] px-3.5 py-3 text-left transition-colors hover:bg-white/[0.055] active:scale-[0.995]"
          >
            <span className="text-[13px] text-ink/90">{prompt}</span>
            <ArrowRight
              size={14}
              weight="bold"
              className="ml-auto shrink-0 text-faint transition-transform group-hover:translate-x-0.5 group-hover:text-brand"
            />
          </button>
        ))}
      </div>
    </div>
  );
}

function Gateway() {
  const agent = useAgent();
  const [onboarding, setOnboarding] = useState(false);
  const [onboardProvider, setOnboardProvider] = useState<string | undefined>(undefined);
  const [modelsOpen, setModelsOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const nearBottom = useRef(true);

  const snapshot = agent.snapshot;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const meta = event.metaKey || event.ctrlKey;
      if (!meta) return;
      if (event.key.toLowerCase() === "k") {
        event.preventDefault();
        setModelsOpen(true);
      } else if (event.key.toLowerCase() === "j") {
        event.preventDefault();
        agent.newSession();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [agent]);

  useEffect(() => {
    if (!nearBottom.current) return;
    bottomRef.current?.scrollIntoView({
      behavior: agent.streaming ? "auto" : "smooth",
      block: "end",
    });
  }, [agent.items, agent.streaming, agent.pending]);

  if (agent.loading && !snapshot) return <Boot />;

  if (!snapshot) {
    return (
      <div className="relative grid min-h-dvh place-items-center px-6">
        <div className="haze" />
        <div className="relative z-10 w-full max-w-[480px] rounded-[14px] bg-panel p-7">
          <Brand />
          <h1 className="mt-6 text-[20px] font-medium text-ink">termagent is not answering</h1>
          <p className="mt-2 text-[13px] leading-6 text-dim">
            {agent.banner ?? "The local server stopped. Restart it and try again."}
          </p>
          <Button
            onClick={() => void agent.refresh()}
            className="mt-4 h-9 rounded-md bg-brand px-3 text-[12.5px] text-white hover:bg-brand/90"
          >
            <ArrowClockwise size={14} weight="bold" /> Retry
          </Button>
        </div>
      </div>
    );
  }

  if (!snapshot.configured || onboarding) {
    return (
      <Onboarding
        snapshot={snapshot}
        initialProvider={onboardProvider}
        onComplete={async (input) => {
          await agent.onboard(input);
          setOnboarding(false);
          setOnboardProvider(undefined);
        }}
        onClose={
          snapshot.configured
            ? () => {
                setOnboarding(false);
                setOnboardProvider(undefined);
              }
            : undefined
        }
        onRefreshRegistry={agent.refreshRegistry}
      />
    );
  }

  const title = snapshot.session?.title ?? "New session";
  const modelName = snapshot.modelName ?? snapshot.model;
  const cwd = snapshot.session?.cwd ?? "";

  return (
    <MotionConfig reducedMotion="user">
      <div className="relative flex min-h-dvh">
        <div className="haze" />

        <div className="relative z-10 hidden h-dvh md:block">
          <Sidebar
            snapshot={snapshot}
            activeId={snapshot.session?.id ?? null}
            onNew={() => agent.newSession()}
            onSelect={agent.loadSession}
            onRename={agent.renameSession}
            onDelete={agent.deleteSession}
            onOpenModels={() => setModelsOpen(true)}
            onOpenOnboarding={() => {
              setOnboardProvider(undefined);
              setOnboarding(true);
            }}
          />
        </div>

        <AnimatePresence>
          {sidebarOpen && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="fixed inset-0 z-40 md:hidden"
            >
              <button
                type="button"
                aria-label="Close sessions"
                onClick={() => setSidebarOpen(false)}
                className="absolute inset-0 bg-black/60 backdrop-blur-[2px]"
              />
              <motion.div
                initial={{ x: -24, opacity: 0 }}
                animate={{ x: 0, opacity: 1 }}
                exit={{ x: -24, opacity: 0 }}
                transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
                className="absolute inset-y-0 left-0 z-50"
              >
                <Sidebar
                  snapshot={snapshot}
                  activeId={snapshot.session?.id ?? null}
                  onNew={() => {
                    agent.newSession();
                    setSidebarOpen(false);
                  }}
                  onSelect={agent.loadSession}
                  onRename={agent.renameSession}
                  onDelete={agent.deleteSession}
                  onOpenModels={() => {
                    setModelsOpen(true);
                    setSidebarOpen(false);
                  }}
                  onOpenOnboarding={() => {
                    setSidebarOpen(false);
                    setOnboardProvider(undefined);
                    setOnboarding(true);
                  }}
                  onClose={() => setSidebarOpen(false)}
                />
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>

        <main className="relative z-10 flex h-dvh min-w-0 flex-1 flex-col">
          <TopBar
            snapshot={snapshot}
            connected={agent.connected}
            busy={agent.busy}
            resolvedEffort={agent.resolvedEffort}
            title={title}
            onOpenSidebar={() => setSidebarOpen(true)}
            onOpenModels={() => setModelsOpen(true)}
            onOpenOnboarding={() => {
              setOnboardProvider(undefined);
              setOnboarding(true);
            }}
            onRename={(next) => {
              if (snapshot.session) agent.renameSession(snapshot.session.id, next);
            }}
          />

          {agent.banner && dismissed !== agent.banner && (
            <div className="flex items-start gap-2 border-b border-line bg-danger/10 px-4 py-2.5 text-[12px] text-ink md:px-6">
              <span className="min-w-0 flex-1">{agent.banner}</span>
              <button
                type="button"
                aria-label="Dismiss"
                onClick={() => {
                  setDismissed(agent.banner);
                  agent.dismissBanner();
                }}
                className="text-dim hover:text-ink"
              >
                <X size={13} weight="bold" />
              </button>
            </div>
          )}

          <div
            ref={scrollRef}
            onScroll={(event) => {
              const node = event.currentTarget;
              nearBottom.current =
                node.scrollHeight - node.scrollTop - node.clientHeight < 120;
            }}
            className="min-h-0 flex-1 overflow-y-auto"
          >
            <div className="mx-auto w-full max-w-[880px] px-4 pb-6 pt-6 md:px-6 md:pt-8">
              {agent.items.length === 0 ? (
                <div>
                  <p className="mono text-[10.5px] tracking-[0.04em] text-faint">
                    {cwd || "this workspace"}
                  </p>
                  <h2 className="mt-2 text-[24px] font-medium leading-tight tracking-tight text-ink">
                    What should we build?
                  </h2>
                  <p className="mt-1.5 max-w-[62ch] text-[13.5px] leading-6 text-dim">
                    {modelName} is connected. termagent reads files, edits code and runs commands
                    in this folder. Nothing leaves your machine except the model requests.
                  </p>
                  <Suggestions onPick={agent.send} />
                </div>
              ) : (
                <MessageList items={agent.items} modelName={modelName} streaming={agent.streaming} />
              )}
              <div ref={bottomRef} className="h-px" />
            </div>
          </div>

          {agent.pending && (
            <ApprovalPanel
              approval={agent.pending}
              cwd={cwd}
              onAnswer={(allow) => agent.approve(agent.pending!.id, allow)}
            />
          )}

          <Composer
            snapshot={snapshot}
            busy={agent.busy}
            connected={agent.connected}
            resolvedEffort={agent.resolvedEffort}
            onSend={agent.send}
            onStop={agent.cancel}
            onOpenModels={() => setModelsOpen(true)}
            onEffort={agent.setEffort}
          />
        </main>
      </div>

      <ModelDialog
        open={modelsOpen}
        onOpenChange={setModelsOpen}
        snapshot={snapshot}
        onUse={(provider, model, effort) => agent.setModel(provider, model, effort)}
        onRefreshRegistry={agent.refreshRegistry}
        onSetupProvider={(providerId) => {
          setOnboardProvider(providerId);
          setOnboarding(true);
        }}
      />
    </MotionConfig>
  );
}

export default function App() {
  if (!booted) return <NotServed />;
  return <Gateway />;
}
