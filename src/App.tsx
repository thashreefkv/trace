import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient, QueryDevtoolsMount } from "./lib/queryClient";
import { useEventInvalidations } from "./lib/eventInvalidations";
import {
  HashRouter,
  NavLink,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useSearchParams,
} from "react-router-dom";
import { Toaster } from "./components/Toaster";
import { BackgroundTasksProvider } from "./lib/backgroundTasks";
import { BackgroundTasksIndicator } from "./components/BackgroundTasksIndicator";
import { toast } from "./lib/toast";
import { getGeminiKeyStatus } from "./lib/ipc";
import {
  CalendarDays,
  CheckCircle2,
  ChevronLeft,
  FolderTree,
  FileText,
  Inbox,
  KanbanSquare,
  Layers3,
  Mail,
  Mic,
  Network,
  Plus,
  Settings,
  Sparkles,
  User,
  UsersRound,
} from "lucide-react";
import { SearchBar } from "./components/SearchBar";
import { CommandPalette } from "./components/CommandPalette";
import traceLogoUrl from "./assets/trace-logo.svg";
import { CaptureInbox } from "./routes/CaptureInbox";
import { CaptureWindow } from "./routes/CaptureWindow";
import { ConversationIngest } from "./routes/ConversationIngest";
import { DeliverableBoard } from "./routes/DeliverableBoard";
import { DeliverableDetail } from "./routes/DeliverableDetail";
import { EmailWorkspace } from "./routes/EmailWorkspace";
import { FilesWorkspace } from "./routes/FilesWorkspace";
import { InitiativeDetail } from "./routes/InitiativeDetail";
import { InitiativeList } from "./routes/InitiativeList";
import { AppSettings } from "./routes/AppSettings";
import { AskWorkspace } from "./routes/AskWorkspace";
import { ReportsWorkspace } from "./routes/Reports";
import { MeetingDetail } from "./routes/MeetingDetail";
import { Meetings } from "./routes/Meetings";
import { SpotlightWindow } from "./routes/SpotlightWindow";
import { StakeholderLens } from "./routes/StakeholderLens";
import { TrayWidget } from "./routes/TrayWidget";
import { WeekView } from "./routes/WeekView";
import { BrainExplorer } from "./components/brain/BrainExplorer";
import { BrainErrorBoundary } from "./components/brain/BrainErrorBoundary";
// WorkContextGraph is no longer mounted; the /context route now redirects
// to /settings/brain. The file stays on disk for one more release window.
import { MyProfile } from "./routes/MyProfile";
import { createCapture } from "./lib/ipc";

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BackgroundTasksProvider>
        <HashRouter>
          <AppRoutes />
        </HashRouter>
        <Toaster />
        <QueryDevtoolsMount />
      </BackgroundTasksProvider>
    </QueryClientProvider>
  );
}

function AppRoutes() {
  const location = useLocation();
  useEventInvalidations();

  if (location.pathname === "/capture") {
    return (
      <Routes>
        <Route path="/capture" element={<CaptureWindow />} />
        <Route path="*" element={<Navigate replace to="/capture" />} />
      </Routes>
    );
  }

  if (location.pathname === "/tray") {
    return (
      <Routes>
        <Route path="/tray" element={<TrayWidget />} />
        <Route path="*" element={<Navigate replace to="/tray" />} />
      </Routes>
    );
  }

  if (location.pathname === "/spotlight") {
    return (
      <Routes>
        <Route path="/spotlight" element={<SpotlightWindow />} />
        <Route path="*" element={<Navigate replace to="/spotlight" />} />
      </Routes>
    );
  }

  const [paletteOpen, setPaletteOpen] = useState(false);
  const [captureOpen, setCaptureOpen] = useState(false);

  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    void listen("show-capture", () => setCaptureOpen(true)).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  const navigate = useNavigate();

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void getGeminiKeyStatus().then((status) => {
      if (!status?.configured) {
        toast.warning("Add your Gemini key to enable AI features", {
          description: "Ask, brain memory, and email intelligence all require a key.",
          duration: Infinity,
          id: "first-run:gemini",
          action: {
            label: "Add key",
            onClick: () => navigate("/settings/connections?focus=gemini-key"),
          },
        });
      }
    });
  }, [navigate]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    type BudgetEvent = {
      kind: "warning" | "exceeded" | "recovery";
      scope: "daily" | "monthly";
      spent_usd: number;
      limit_usd: number;
      threshold_pct: number;
      block_active: boolean;
    };
    void listen<BudgetEvent>("budget:event", (event) => {
      const p = event.payload;
      const scopeLabel = p.scope === "daily" ? "Daily" : "Monthly";
      const id = `budget:${p.scope}`;
      const fmt = (n: number) => `$${n.toFixed(2)}`;
      if (p.kind === "recovery") {
        toast.success(`${scopeLabel} AI spend back under threshold`, {
          id,
          duration: 6000,
        });
      } else if (p.kind === "exceeded") {
        const verb = p.block_active ? "blocked — exceeded" : "exceeded";
        toast.error(`${scopeLabel} AI budget ${verb}`, {
          description: `Spent ${fmt(p.spent_usd)} of ${fmt(p.limit_usd)}. ${
            p.block_active
              ? "AI calls are paused until you raise the limit."
              : "Calls are still running — toggle blocking in Settings if you want a hard cap."
          }`,
          duration: Infinity,
          id,
          action: { label: "Adjust", onClick: () => navigate("/settings/ai?focus=ai-budget") },
        });
      } else {
        toast.warning(
          `${scopeLabel} AI spend at ${Math.round(p.threshold_pct)}% of limit`,
          {
            description: `Spent ${fmt(p.spent_usd)} of ${fmt(p.limit_usd)}.`,
            duration: Infinity,
            id,
            action: { label: "Adjust", onClick: () => navigate("/settings/ai?focus=ai-budget") },
          },
        );
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [navigate]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    void listen<{ new_messages: number; new_threads: number }>("gmail:new-mail", async (event) => {
      if (!("Notification" in window)) return;
      if (Notification.permission === "default") {
        await Notification.requestPermission();
      }
      if (Notification.permission !== "granted") return;
      const count = event.payload.new_messages;
      new Notification("New Gmail in Trace", {
        body: `${count} new message${count === 1 ? "" : "s"} synced locally.`,
      });
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    void listen<{
      source_kind: string;
      source_id: string | null;
      created: number;
      updated: number;
      skipped: number;
    }>("memory:extracted", async (event) => {
      window.dispatchEvent(new CustomEvent("trace:memory-extracted", { detail: event.payload }));
      if (!("Notification" in window)) return;
      if (Notification.permission === "default") {
        await Notification.requestPermission();
      }
      if (Notification.permission !== "granted") return;
      const { created, updated, source_kind } = event.payload;
      const total = created + updated;
      if (total === 0) return;
      const verb = created > 0 ? `learned ${created}` : `refreshed ${updated}`;
      new Notification("Trace memory", {
        body: `Memory ${verb} fact${total === 1 ? "" : "s"} from ${source_kind}.`,
      });
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  return (
    <>
      <NavigationBridge />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
      {captureOpen && <QuickCapture onClose={() => setCaptureOpen(false)} />}
      <div className="app-shell flex h-screen overflow-hidden">
        <aside className="hidden w-64 border-r border-zinc-200/60 bg-white/85 px-4 py-5 backdrop-blur md:block">
          <div className="mb-8 flex items-center gap-3 px-2">
            <img
              alt=""
              aria-hidden="true"
              className="h-8 w-8 flex-shrink-0"
              src={traceLogoUrl}
            />
            <div className="min-w-0">
              <div className="text-[15px] font-semibold leading-5 tracking-tight text-zinc-950">Trace</div>
              <div className="truncate text-[11px] font-medium text-zinc-400">Work memory</div>
            </div>
          </div>
          <nav aria-label="Primary" className="space-y-1">
            <PrimaryLink to="/deliverables">
              <KanbanSquare aria-hidden="true" size={16} strokeWidth={1.5} />
              Deliverables
            </PrimaryLink>
            <PrimaryLink to="/ask">
              <Sparkles aria-hidden="true" size={16} strokeWidth={1.5} />
              Ask
            </PrimaryLink>
            <PrimaryLink to="/reports">
              <FileText aria-hidden="true" size={16} strokeWidth={1.5} />
              Reports
            </PrimaryLink>
            <PrimaryLink to="/week">
              <CalendarDays aria-hidden="true" size={16} strokeWidth={1.5} />
              Week
            </PrimaryLink>
            <PrimaryLink to="/captures">
              <Inbox aria-hidden="true" size={16} strokeWidth={1.5} />
              Captures
            </PrimaryLink>
            <PrimaryLink to="/email">
              <Mail aria-hidden="true" size={16} strokeWidth={1.5} />
              Email
            </PrimaryLink>
            <PrimaryLink to="/files">
              <FolderTree aria-hidden="true" size={16} strokeWidth={1.5} />
              Files
            </PrimaryLink>
            <PrimaryLink to="/meetings">
              <Mic aria-hidden="true" size={16} strokeWidth={1.5} />
              Meetings
            </PrimaryLink>
            <PrimaryLink to="/initiatives">
              <Layers3 aria-hidden="true" size={16} strokeWidth={1.5} />
              Initiatives
            </PrimaryLink>
            <PrimaryLink to="/stakeholders">
              <UsersRound aria-hidden="true" size={16} strokeWidth={1.5} />
              Stakeholders
            </PrimaryLink>
            <PrimaryLink to="/brain">
              <Network aria-hidden="true" size={16} strokeWidth={1.5} />
              Brain
            </PrimaryLink>
            <PrimaryLink to="/settings">
              <Settings aria-hidden="true" size={16} strokeWidth={1.5} />
              Settings
            </PrimaryLink>
            <PrimaryLink to="/profile">
              <User aria-hidden="true" size={16} strokeWidth={1.5} />
              My Profile
            </PrimaryLink>
          </nav>
        </aside>

        <main className="flex min-w-0 flex-1 flex-col">
          <header className="sticky top-0 z-40 border-b border-zinc-200/60 bg-[var(--surface-header)] px-5 py-3 backdrop-blur-md">
            <div className="flex items-center gap-3">
              <BackButton />
              <SearchBar />
              <BackgroundTasksIndicator />
              <button
                className="hidden shrink-0 items-center gap-1.5 rounded-lg border border-zinc-200 bg-white px-2.5 py-2 text-[11px] font-medium text-zinc-400 shadow-sm transition-colors hover:border-zinc-300 hover:text-zinc-600 md:flex dark:border-zinc-700 dark:bg-zinc-900"
                onClick={() => setPaletteOpen(true)}
                title="Command palette"
                type="button"
              >
                <kbd className="font-sans">⌘K</kbd>
              </button>
            </div>
          </header>
          <div className="min-h-0 flex-1 overflow-auto">
            <Routes>
              <Route path="/" element={<Navigate replace to="/deliverables" />} />
              <Route path="/deliverables" element={<DeliverableBoard />} />
              <Route path="/ask" element={<AskWorkspace />} />
              <Route path="/reports" element={<ReportsWorkspace />} />
              <Route path="/deliverables/:deliverableId" element={<DeliverableDetail />} />
              <Route path="/week" element={<WeekView />} />
              <Route path="/captures" element={<CaptureInbox />} />
              <Route path="/email" element={<EmailWorkspace />} />
              <Route path="/files" element={<FilesWorkspace />} />
              <Route path="/conversations/ingest" element={<ConversationIngest />} />
              <Route path="/meetings" element={<Meetings />} />
              <Route path="/meetings/:id" element={<MeetingDetail />} />
              <Route path="/initiatives" element={<InitiativeList />} />
              <Route path="/initiatives/:initiativeId" element={<InitiativeDetail />} />
              <Route path="/stakeholders" element={<StakeholderLens />} />
              <Route path="/stakeholders/:stakeholderId" element={<StakeholderLens />} />
              {/* Phase 3 sunset: /context now redirects to the new Brain explorer.
                  WorkContextGraph.tsx kept on disk for one more release in case
                  rollback is needed; will be deleted after Phase 3 stabilizes. */}
              {/* Brain is a first-class top-level page (its own route, not nested
                  under Settings). Legacy routes redirect here so old bookmarks
                  keep working. */}
              <Route
                path="/brain"
                element={
                  <BrainErrorBoundary>
                    <div className="flex h-full min-h-0 flex-1 flex-col p-3">
                      <BrainExplorer />
                    </div>
                  </BrainErrorBoundary>
                }
              />
              <Route path="/context" element={<Navigate replace to="/brain" />} />
              <Route path="/settings/brain" element={<Navigate replace to="/brain" />} />
              <Route path="/settings/*" element={<AppSettings />} />
              <Route path="/profile" element={<MyProfile />} />
              <Route path="/tray" element={<TrayWidget />} />
            </Routes>
          </div>
        </main>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Global back button — shown when there's a thread open in email or any
// sub-detail page where navigate(-1) is meaningful
// ---------------------------------------------------------------------------

function BackButton() {
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();

  const hasThread = Boolean(params.get("thread"));
  const isEmailDetail = location.pathname === "/email" && hasThread;
  const isSubDetail =
    /^\/(deliverables|initiatives|meetings)\/[^/]+/.test(location.pathname) ||
    (/^\/stakeholders\/[^/]+/.test(location.pathname) && !location.pathname.endsWith("/stakeholders"));

  if (!isEmailDetail && !isSubDetail) return null;

  return (
    <button
      className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-zinc-200 bg-white text-zinc-500 shadow-sm transition-colors hover:border-zinc-300 hover:text-zinc-800"
      onClick={() => navigate(-1)}
      title="Go back"
      type="button"
    >
      <ChevronLeft size={16} />
    </button>
  );
}

// ---------------------------------------------------------------------------
// Quick-capture modal — shown via Cmd+Shift+M
// ---------------------------------------------------------------------------

function QuickCapture({ onClose }: { onClose: () => void }) {
  const [value, setValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!value.trim() || saving) return;
    setSaving(true);
    try {
      await createCapture({ kind: "thought", body: value.trim() });
      setSaved(true);
      setTimeout(() => onClose(), 700);
    } catch {
      setSaving(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center"
      style={{ paddingTop: "22vh", background: "rgba(0,0,0,0.18)", backdropFilter: "blur(2px)" }}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="w-[520px] rounded-2xl bg-white shadow-2xl border border-zinc-200/80 overflow-hidden">
        {/* Input row */}
        <form onSubmit={handleSubmit}>
          <div className="flex items-center gap-3 px-5 py-4">
            <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg bg-zinc-100">
              {saved ? (
                <CheckCircle2 size={14} strokeWidth={2} className="text-emerald-500" />
              ) : (
                <Plus size={14} strokeWidth={2} className="text-zinc-500" />
              )}
            </div>
            <input
              ref={inputRef}
              type="text"
              placeholder="Capture a thought…"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              disabled={saving}
              className="flex-1 text-[15px] text-zinc-900 placeholder:text-zinc-400 outline-none bg-transparent disabled:opacity-50"
            />
            <button
              type="submit"
              disabled={!value.trim() || saving}
              className="h-7 px-3 rounded-lg bg-zinc-900 text-white text-[12px] font-medium disabled:opacity-30 transition-opacity flex-shrink-0"
            >
              {saved ? "Saved" : "Save"}
            </button>
          </div>
        </form>

        {/* Footer hint */}
        <div className="border-t border-zinc-100 px-5 py-2 flex items-center gap-4">
          <span className="text-[11px] text-zinc-400">
            <kbd className="font-sans">↵</kbd> save &nbsp;·&nbsp; <kbd className="font-sans">esc</kbd> cancel
          </span>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------

function NavigationBridge() {
  const navigate = useNavigate();

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let unlisten: (() => void) | undefined;

    void listen<string>("navigate-to", (event) => {
      navigate(event.payload);
    }).then((nextUnlisten) => {
      unlisten = nextUnlisten;
    });

    return () => {
      unlisten?.();
    };
  }, [navigate]);

  return null;
}

function isTauriRuntime() {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

interface PrimaryLinkProps {
  to: string;
  children: ReactNode;
}

function PrimaryLink({ to, children }: PrimaryLinkProps) {
  return (
    <NavLink
      className={({ isActive }) =>
        [
          "flex h-8 items-center gap-2.5 rounded-lg px-2 text-[13px] font-medium transition-colors duration-100 outline-none",
          isActive
            ? "bg-zinc-950 text-white shadow-sm"
            : "text-zinc-500 hover:bg-zinc-100/80 hover:text-zinc-950",
        ].join(" ")
      }
      to={to}
    >
      {children}
    </NavLink>
  );
}

export default App;
