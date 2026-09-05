import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  Link,
  Navigate,
  NavLink,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useSearchParams,
} from "react-router-dom";
import {
  Brain,
  CalendarDays,
  Check,
  CheckCircle2,
  Clipboard,
  CloudCog,
  Coins,
  Eye,
  FolderTree,
  KeyRound,
  LayoutDashboard,
  Loader2,
  Mail,
  Network,
  NotebookText,
  RefreshCw,
  Search,
  Server,
  ShieldAlert,
  SlidersHorizontal,
  Smartphone,
  Sparkles,
  Trash2,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import {
  appleNotesSetup,
  appleNotesSyncNow,
  clearGeminiApiKey,
  gcalConnect,
  gcalDisconnect,
  gcalStatus,
  gcalSync,
  getBudgetStatus,
  getEmbeddingProgress,
  getGeminiKeyStatus,
  getMcpConfigSnippet,
  getMcpInstallState,
  getMemorySettings,
  getSiriApiStatus,
  getSiriToken,
  regenerateSiriToken,
  type SiriApiStatus,
  gmailConnect,
  gmailDisconnect,
  gmailStatus,
  gmailSyncNow,
  gmailUpdateSyncSettings,
  installMcpServer,
  openMcpLog,
  saveGeminiApiKey,
  testGeminiApiKey,
  updateMemorySettings,
} from "../lib/ipc";
import {
  disconnectGoogleDrive,
  driveConnect,
  driveStatus,
  type DriveStatus,
} from "../lib/files";
import { BudgetSettingsCard } from "../components/BudgetSettingsCard";
import { EmbeddingStatusPanel } from "../components/EmbeddingStatusPanel";
import { EvalHarnessPanel } from "../components/EvalHarnessPanel";
import { GeminiUsagePanel } from "../components/GeminiUsagePanel";
import { GmailSyncSettingsControls } from "../components/GmailSyncSettingsControls";
import { InferenceReviewQueue } from "../components/InferenceReviewQueue";
import { PromptInjectionLogPanel } from "../components/PromptInjectionLogPanel";
import { RLDigestPanel } from "../components/RLDigestPanel";
import { RLLearningSettings } from "../components/RLLearningSettings";
import { SenderRulesPanel } from "../components/SenderRulesPanel";
import { SupersessionLog } from "../components/SupersessionLog";
import { ToolCallLogPanel } from "../components/ToolCallLogPanel";
import { WorkMailDomainSettings } from "../components/WorkMailDomainSettings";
import type {
  BudgetStatus,
  EmbeddingProgress,
  GCalStatus,
  GeminiKeyStatus,
  GmailStatus,
  GmailSyncSettingsInput,
  McpConfigSnippet,
  McpInstallState,
  MemorySettings,
  UpdateMemorySettingsInput,
} from "../lib/types";

type SettingsPageId =
  | "overview"
  | "connections"
  | "email"
  | "ai"
  | "brain"
  | "memory-learning"
  | "diagnostics";

interface SettingsPage {
  description: string;
  id: SettingsPageId;
  icon: LucideIcon;
  path: string;
  title: string;
  width: string;
  fullBleed?: boolean;
}

interface SettingsSearchSection {
  aliases: string[];
  description: string;
  id: string;
  page: SettingsPageId;
  title: string;
}

const SETTINGS_PAGES: SettingsPage[] = [
  {
    description: "Connection health, AI spend, and indexing status at a glance.",
    icon: LayoutDashboard,
    id: "overview",
    path: "/settings",
    title: "Overview",
    width: "max-w-5xl",
  },
  {
    description: "Credentials, external accounts, and MCP integration details.",
    icon: Server,
    id: "connections",
    path: "/settings/connections",
    title: "Connections",
    width: "max-w-3xl",
  },
  {
    description: "Work Mail sync behavior, sender rules, and scope controls.",
    icon: Mail,
    id: "email",
    path: "/settings/email",
    title: "Email",
    width: "max-w-5xl",
  },
  {
    description: "Budget limits, Gemini usage, and semantic indexing health.",
    icon: Sparkles,
    id: "ai",
    path: "/settings/ai",
    title: "AI",
    width: "max-w-5xl",
  },
  // Brain was previously a Settings sub-page. It's now a first-class top-level
  // route (/brain) so the knowledge-graph explorer doesn't get nested under the
  // Settings header + sub-nav. Kept out of SETTINGS_SECTIONS so it no longer
  // shows in the Settings sidebar.
  {
    description: "Memory capture policy and feedback-driven learning controls.",
    icon: SlidersHorizontal,
    id: "memory-learning",
    path: "/settings/memory-learning",
    title: "Memory & Learning",
    width: "max-w-5xl",
  },
  {
    description: "Evaluation harnesses, audit logs, and prompt safety traces.",
    icon: ShieldAlert,
    id: "diagnostics",
    path: "/settings/diagnostics",
    title: "Diagnostics",
    width: "max-w-6xl",
  },
];

const SETTINGS_SEARCH_SECTIONS: SettingsSearchSection[] = [
  {
    aliases: ["dashboard", "health", "status", "summary"],
    description: "Connection, budget, and indexing status.",
    id: "settings-overview",
    page: "overview",
    title: "Settings overview",
  },
  {
    aliases: ["gemini", "api key", "model key", "ai key"],
    description: "Save, test, or clear the Gemini API key.",
    id: "gemini-key",
    page: "connections",
    title: "Gemini API key",
  },
  {
    aliases: ["gmail", "google mail", "account"],
    description: "Connect or disconnect the Gmail account.",
    id: "gmail-account",
    page: "connections",
    title: "Gmail connection",
  },
  {
    aliases: ["drive", "google drive", "files account"],
    description: "Manage Google Drive access.",
    id: "drive-account",
    page: "connections",
    title: "Google Drive connection",
  },
  {
    aliases: ["calendar", "gcal", "week sync"],
    description: "Manage Google Calendar access and sync.",
    id: "calendar-account",
    page: "connections",
    title: "Google Calendar connection",
  },
  {
    aliases: ["apple notes", "notes", "capture inbox", "notes sync"],
    description: "Sync notes from Apple Notes into the Capture inbox.",
    id: "apple-notes",
    page: "connections",
    title: "Apple Notes",
  },
  {
    aliases: ["mcp", "claude desktop", "snippet", "server"],
    description: "Install state, paths, config snippet, and log.",
    id: "mcp-server",
    page: "connections",
    title: "MCP server",
  },
  {
    aliases: ["siri", "shortcuts", "iphone", "remote", "tailscale", "ask trace"],
    description: "Reach Trace from your iPhone or Mac via Siri Shortcuts over Tailscale.",
    id: "siri-remote-access",
    page: "connections",
    title: "Siri & Apple Shortcuts",
  },
  {
    aliases: ["gmail sync", "mail poll", "thread cap", "notifications", "backfill"],
    description: "Canonical Work Mail sync controls.",
    id: "gmail-sync",
    page: "email",
    title: "Gmail sync behavior",
  },
  {
    aliases: ["sender", "rule", "classifier", "override"],
    description: "Deterministic sender rules.",
    id: "sender-rules",
    page: "email",
    title: "Email sender rules",
  },
  {
    aliases: ["domains", "work scope", "mail scope"],
    description: "Work Mail sender domain scope.",
    id: "work-mail-domains",
    page: "email",
    title: "Work Mail domains",
  },
  {
    aliases: ["budget", "limit", "spend", "cost cap"],
    description: "Warn or block AI calls by spend.",
    id: "ai-budget",
    page: "ai",
    title: "AI budget",
  },
  {
    aliases: ["gemini usage", "tokens", "cost", "usage"],
    description: "Gemini call and token reporting.",
    id: "gemini-usage",
    page: "ai",
    title: "Gemini usage",
  },
  {
    aliases: ["embedding", "embeddings", "semantic", "index", "retrieval"],
    description: "Semantic embedding queue and health.",
    id: "semantic-embeddings",
    page: "ai",
    title: "Semantic embeddings",
  },
  {
    aliases: ["graph", "network", "entities", "context", "memory map", "knowledge graph", "explorer"],
    description: "WebGL knowledge-graph explorer with semantic and Cypher search.",
    id: "brain-explorer",
    page: "brain",
    title: "Brain explorer",
  },
  {
    aliases: ["cypher", "query", "kuzu", "opencypher", "graph query"],
    description: "Run read-only Cypher against the knowledge graph.",
    id: "brain-cypher",
    page: "brain",
    title: "Cypher query",
  },
  {
    aliases: ["nl search", "semantic search", "find entities", "ask graph"],
    description: "Natural-language entity search across the graph.",
    id: "brain-search",
    page: "brain",
    title: "Brain natural-language search",
  },
  {
    aliases: ["memory", "recall", "auto extract", "confirmation", "retrieval limit"],
    description: "Memory capture and retrieval policy.",
    id: "memory-policy",
    page: "memory-learning",
    title: "Memory policy",
  },
  {
    aliases: ["brain digest", "learning", "feedback", "bandit"],
    description: "Learning summaries from feedback.",
    id: "brain-learning",
    page: "memory-learning",
    title: "Brain learning",
  },
  {
    aliases: ["inference", "review queue", "accept", "reject"],
    description: "Pending learning review.",
    id: "inference-review",
    page: "memory-learning",
    title: "Inference review queue",
  },
  {
    aliases: ["supersession", "override", "recent decisions"],
    description: "Recent inference overrides.",
    id: "supersessions",
    page: "memory-learning",
    title: "Recent supersessions",
  },
  {
    aliases: ["policies", "template", "weights", "threshold"],
    description: "Per-template learning policy details.",
    id: "learning-policies",
    page: "memory-learning",
    title: "Learning policies",
  },
  {
    aliases: ["eval", "evaluation", "fixtures", "baseline"],
    description: "Quality fixtures and baselines.",
    id: "eval-harness",
    page: "diagnostics",
    title: "Eval harness",
  },
  {
    aliases: ["tool call", "tools", "audit log"],
    description: "Audit every AI tool invocation.",
    id: "tool-call-log",
    page: "diagnostics",
    title: "Tool calls",
  },
  {
    aliases: ["prompt injection", "safety", "flags", "refusals"],
    description: "Prompt safety audit trail.",
    id: "prompt-injection-log",
    page: "diagnostics",
    title: "Prompt injection log",
  },
];

export function AppSettings() {
  return (
    <SettingsShell>
      <Routes>
        <Route index element={<SettingsOverview />} />
        <Route path="connections" element={<ConnectionsSettings />} />
        <Route path="email" element={<EmailSettings />} />
        <Route path="ai" element={<AiSettings />} />
        {/* /settings/brain is intercepted at the top level (App.tsx) and
            redirected to /brain; this route stays as a safety net in case of
            stale links inside the Settings shell. */}
        <Route path="brain" element={<Navigate replace to="/brain" />} />
        <Route path="memory-learning" element={<MemoryLearningSettings />} />
        <Route path="diagnostics" element={<DiagnosticsSettings />} />
        <Route
          path="mcp"
          element={<Navigate replace to="/settings/connections?focus=mcp-server" />}
        />
        <Route path="*" element={<Navigate replace to="/settings" />} />
      </Routes>
    </SettingsShell>
  );
}

function SettingsShell({ children }: { children: ReactNode }) {
  const location = useLocation();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const activePage = pageForPath(location.pathname);

  useEffect(() => {
    const focusId = params.get("focus");
    if (!focusId) return;
    const raf = window.requestAnimationFrame(() => {
      const target = document.getElementById(focusId);
      target?.scrollIntoView({ behavior: "smooth", block: "start" });
      target?.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(raf);
  }, [location.pathname, params]);

  const fullBleed = Boolean(activePage.fullBleed);

  return (
    <div
      className={
        fullBleed
          ? "flex min-h-full w-full flex-col px-4 py-4"
          : "mx-auto min-h-full max-w-7xl px-5 py-6"
      }
    >
      <div className="mb-5 rounded-2xl border border-zinc-100 bg-white/85 px-5 py-4 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <p className="page-kicker">Power controls</p>
            <h1 className="text-2xl font-semibold tracking-tight text-zinc-950">Settings</h1>
            <p className="mt-1 max-w-2xl text-sm text-zinc-500">
              Tune connections, mail intake, AI spend, memory behavior, and diagnostics without
              losing control detail in one long page.
            </p>
          </div>
          <QuickJumpSearch />
        </div>
      </div>

      <div className="mb-4 lg:hidden">
        <label className="space-y-1.5">
          <span className="field-label">Category</span>
          <select
            className="field-control"
            onChange={(event) => navigate(event.currentTarget.value)}
            value={activePage.path}
          >
            {SETTINGS_PAGES.map((page) => (
              <option key={page.id} value={page.path}>
                {page.title}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="grid gap-5 lg:grid-cols-[240px_minmax(0,1fr)]">
        <aside className="hidden lg:block">
          <nav
            aria-label="Settings"
            className="sticky top-20 rounded-2xl border border-zinc-100 bg-white p-2 shadow-[0_2px_12px_rgba(0,0,0,0.04)]"
          >
            {SETTINGS_PAGES.map((page) => {
              const Icon = page.icon;
              return (
                <NavLink
                  className={({ isActive }) =>
                    [
                      "mb-1 flex gap-3 rounded-xl px-3 py-2.5 outline-none transition-colors last:mb-0",
                      isActive || activePage.id === page.id
                        ? "bg-zinc-950 text-white"
                        : "text-zinc-600 hover:bg-zinc-50 hover:text-zinc-950",
                    ].join(" ")
                  }
                  end={page.id === "overview"}
                  key={page.id}
                  to={page.path}
                >
                  <Icon className="mt-0.5 shrink-0" size={15} />
                  <span className="min-w-0">
                    <span className="block text-[13px] font-semibold">{page.title}</span>
                    <span
                      className={[
                        "mt-0.5 block text-[11px] leading-4",
                        activePage.id === page.id ? "text-zinc-300" : "text-zinc-400",
                      ].join(" ")}
                    >
                      {page.description}
                    </span>
                  </span>
                </NavLink>
              );
            })}
          </nav>
        </aside>

        <main className={`flex min-w-0 flex-col ${activePage.width}`}>
          {!fullBleed && (
            <div className="mb-4">
              <h2 className="text-xl font-semibold tracking-tight text-zinc-950">
                {activePage.title}
              </h2>
              <p className="mt-1 text-sm text-zinc-500">{activePage.description}</p>
            </div>
          )}
          {children}
        </main>
      </div>
    </div>
  );
}

function QuickJumpSearch() {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const results = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return [];
    return SETTINGS_SEARCH_SECTIONS.filter((section) =>
      [section.title, section.description, ...section.aliases]
        .join(" ")
        .toLowerCase()
        .includes(normalized),
    ).slice(0, 6);
  }, [query]);

  function jump(section: SettingsSearchSection) {
    const page = pageById(section.page);
    setQuery("");
    navigate(`${page.path}?focus=${section.id}`);
  }

  return (
    <div className="relative w-full xl:w-[380px]">
      <label className="relative block">
        <Search
          aria-hidden="true"
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          size={15}
        />
        <input
          aria-label="Jump to a setting"
          className="field-control h-10 pl-9"
          onChange={(event) => setQuery(event.currentTarget.value)}
          placeholder="Jump to API key, budget, sender rules..."
          value={query}
        />
      </label>
      {query.trim() ? (
        <div className="absolute right-0 top-[calc(100%+0.4rem)] z-30 w-full overflow-hidden rounded-2xl border border-zinc-100 bg-white p-1 shadow-xl">
          {results.length > 0 ? (
            results.map((section) => (
              <button
                className="flex w-full items-start justify-between gap-3 rounded-xl px-3 py-2 text-left hover:bg-zinc-50"
                key={section.id}
                onClick={() => jump(section)}
                type="button"
              >
                <span>
                  <span className="block text-[13px] font-semibold text-zinc-900">
                    {section.title}
                  </span>
                  <span className="mt-0.5 block text-[11px] text-zinc-400">
                    {section.description}
                  </span>
                </span>
                <span className="mt-0.5 rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-semibold uppercase text-zinc-500">
                  {pageById(section.page).title}
                </span>
              </button>
            ))
          ) : (
            <div className="px-3 py-4 text-[12px] text-zinc-400">
              No setting matches "{query.trim()}".
            </div>
          )}
        </div>
      ) : null}
    </div>
  );
}

function SettingsOverview() {
  const [snapshot, setSnapshot] = useState<OverviewSnapshot | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let active = true;
    async function load() {
      setLoading(true);
      const [gemini, gmail, drive, calendar, mcp, budget, embeddings] =
        await Promise.allSettled([
          getGeminiKeyStatus(),
          gmailStatus(),
          driveStatus(),
          gcalStatus(),
          getMcpInstallState(),
          getBudgetStatus(),
          getEmbeddingProgress(),
        ]);
      if (!active) return;
      setSnapshot({
        budget: settledValue(budget),
        calendar: settledValue(calendar),
        drive: settledValue(drive),
        embeddings: settledValue(embeddings),
        gemini: settledValue(gemini),
        gmail: settledValue(gmail),
        mcp: settledValue(mcp),
      });
      setLoading(false);
    }
    void load();
    return () => {
      active = false;
    };
  }, []);

  const cards: OverviewCardProps[] = [
    {
      description: "Required for Ask, AI mail analysis, and planning.",
      icon: KeyRound,
      id: "overview-gemini",
      label: "Gemini API",
      status: snapshot?.gemini?.configured ? "Configured" : "Needs key",
      tone: snapshot?.gemini?.configured ? "ok" : "warn",
      to: "/settings/connections?focus=gemini-key",
    },
    {
      description: snapshot?.gmail?.settings?.account_email ?? "Local Work Mail sync.",
      icon: Mail,
      id: "overview-gmail",
      label: "Gmail",
      status: snapshot?.gmail?.connected ? "Connected" : "Not connected",
      tone: snapshot?.gmail?.connected ? "ok" : "muted",
      to: "/settings/email?focus=gmail-sync",
    },
    {
      description:
        snapshot?.drive && snapshot.drive.accounts.length > 0
          ? snapshot.drive.accounts.map((account) => account.email).join(", ")
          : "Browse and import Drive files.",
      icon: FolderTree,
      id: "overview-drive",
      label: "Google Drive",
      status: snapshot?.drive?.connected ? "Connected" : "Not connected",
      tone: snapshot?.drive?.connected ? "ok" : "muted",
      to: "/settings/connections?focus=drive-account",
    },
    {
      description: "Week scheduling and calendar event sync.",
      icon: CalendarDays,
      id: "overview-calendar",
      label: "Google Calendar",
      status: snapshot?.calendar?.connected ? "Connected" : "Not connected",
      tone: snapshot?.calendar?.connected ? "ok" : "muted",
      to: "/settings/connections?focus=calendar-account",
    },
    {
      description: "Claude Desktop local integration.",
      icon: Server,
      id: "overview-mcp",
      label: "MCP server",
      status: snapshot?.mcp?.installed ? "Installed" : "Needs install",
      tone: snapshot?.mcp?.last_error ? "bad" : snapshot?.mcp?.installed ? "ok" : "warn",
      to: "/settings/connections?focus=mcp-server",
    },
    {
      description: budgetDescription(snapshot?.budget),
      icon: Coins,
      id: "overview-budget",
      label: "AI budget",
      status: budgetStatus(snapshot?.budget),
      tone: budgetTone(snapshot?.budget),
      to: "/settings/ai?focus=ai-budget",
    },
    {
      description: embeddingDescription(snapshot?.embeddings),
      icon: Network,
      id: "overview-embeddings",
      label: "Semantic embeddings",
      status: embeddingStatus(snapshot?.embeddings),
      tone:
        snapshot?.embeddings && snapshot.embeddings.pending + snapshot.embeddings.stale > 0
          ? "warn"
          : snapshot?.embeddings
            ? "ok"
            : "muted",
      to: "/settings/ai?focus=semantic-embeddings",
    },
  ];

  return (
    <Anchor id="settings-overview">
      <div className="grid gap-3 md:grid-cols-2">
        {cards.map((card) => (
          <OverviewCard key={card.id} loading={loading && !snapshot} {...card} />
        ))}
      </div>

      <div className="mt-4 rounded-2xl border border-zinc-100 bg-white px-5 py-4 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <p className="page-kicker">Jump routes</p>
            <p className="text-sm text-zinc-600">
              Connections hold accounts. Email, AI, Learning, and Diagnostics hold the detailed
              controls.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            {SETTINGS_PAGES.filter((page) => page.id !== "overview").map((page) => (
              <Link className="btn" key={page.id} to={page.path}>
                {page.title}
              </Link>
            ))}
          </div>
        </div>
      </div>
    </Anchor>
  );
}

function ConnectionsSettings() {
  return (
    <div className="space-y-5">
      <GeminiKeyPanel />
      <GmailConnectionPanel />
      <DriveConnectionPanel />
      <CalendarConnectionPanel />
      <AppleNotesPanel />
      <SiriRemoteAccessPanel />
      <McpServerPanel />
    </div>
  );
}

function EmailSettings() {
  return (
    <div className="space-y-5">
      <GmailSyncPanel />
      <Anchor id="sender-rules">
        <SenderRulesPanel />
      </Anchor>
      <WorkMailDomainSettings />
    </div>
  );
}

function AiSettings() {
  return (
    <div className="space-y-5">
      <Anchor id="ai-budget">
        <BudgetSettingsCard />
      </Anchor>
      <Anchor id="gemini-usage">
        <GeminiUsagePanel />
      </Anchor>
      <Anchor id="semantic-embeddings">
        <EmbeddingStatusPanel />
      </Anchor>
    </div>
  );
}

// BrainSettings was removed when Brain became a top-level route (/brain).
// The Settings router now redirects /settings/brain → /brain.

function MemoryLearningSettings() {
  return (
    <div className="space-y-5">
      <MemoryPolicyPanel />
      <section id="brain-learning" tabIndex={-1} className="scroll-mt-24 outline-none">
        <div className="mb-3 flex flex-col gap-1">
          <p className="page-kicker">Feedback loop</p>
          <h3 className="text-base font-semibold text-zinc-950">Brain learning</h3>
          <p className="text-sm text-zinc-500">
            Review feedback, threshold drift, and template-specific learned behavior together.
          </p>
        </div>
        <div className="space-y-5">
          <RLDigestPanel />
          <Anchor id="inference-review">
            <InferenceReviewQueue />
          </Anchor>
          <Anchor id="supersessions">
            <SupersessionLog />
          </Anchor>
          <Anchor id="learning-policies">
            <RLLearningSettings />
          </Anchor>
        </div>
      </section>
    </div>
  );
}

function DiagnosticsSettings() {
  return (
    <div className="space-y-5">
      <Anchor id="eval-harness">
        <EvalHarnessPanel />
      </Anchor>
      <Anchor id="tool-call-log">
        <ToolCallLogPanel />
      </Anchor>
      <Anchor id="prompt-injection-log">
        <PromptInjectionLogPanel />
      </Anchor>
    </div>
  );
}

function GeminiKeyPanel() {
  const [status, setStatus] = useState<GeminiKeyStatus | null>(null);
  const [key, setKey] = useState("");
  const [message, setMessage] = useState<Message | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    getGeminiKeyStatus().then(setStatus).catch((error) => setMessage({ ok: false, text: String(error) }));
  }, []);

  async function saveKey() {
    if (!key.trim()) return;
    setLoading(true);
    setMessage(null);
    try {
      setStatus(await saveGeminiApiKey(key.trim()));
      setKey("");
      setMessage({ ok: true, text: "Gemini API key saved." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  async function testKey() {
    setLoading(true);
    setMessage(null);
    try {
      const result = await testGeminiApiKey();
      setMessage({ ok: result.ok, text: result.message });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  async function clearKey() {
    setLoading(true);
    setMessage(null);
    try {
      setStatus(await clearGeminiApiKey());
      setMessage({ ok: true, text: "Gemini API key cleared." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  return (
    <SettingsPanel
      description="Powers Ask, task generation, briefings, Work Mail analysis, and week planning."
      icon={KeyRound}
      id="gemini-key"
      status={
        <StatusPill tone={status?.configured ? "ok" : "warn"}>
          {status?.configured ? "Configured" : "Not set"}
        </StatusPill>
      }
      title="Gemini API"
    >
      {message ? <Feedback message={message} /> : null}
      <div className="space-y-1.5">
        <label className="field-label">{status?.configured ? "Replace key" : "API key"}</label>
        <div className="flex flex-col gap-2 sm:flex-row">
          <input
            autoComplete="off"
            autoCorrect="off"
            className="field-control flex-1"
            onChange={(event) => setKey(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void saveKey();
            }}
            placeholder="AIzaSy..."
            spellCheck={false}
            type="password"
            value={key}
          />
          <button
            className="btn btn-primary shrink-0"
            disabled={loading || !key.trim()}
            onClick={() => void saveKey()}
            type="button"
          >
            {loading ? <Loader2 className="animate-spin" size={14} /> : <Check size={14} />}
            Save
          </button>
        </div>
        <p className="text-[11px] text-zinc-400">
          Key material stays in the local Keychain. Get one from{" "}
          <a
            className="text-sky-600 hover:underline"
            href="https://aistudio.google.com/app/apikey"
            rel="noopener noreferrer"
            target="_blank"
          >
            Google AI Studio
          </a>
          .
        </p>
      </div>

      {status?.configured ? (
        <div className="flex flex-wrap gap-2 pt-1">
          <button className="btn" disabled={loading} onClick={() => void testKey()} type="button">
            {loading ? <Loader2 className="animate-spin" size={14} /> : <RefreshCw size={14} />}
            Test key
          </button>
          <button
            className="btn btn-danger"
            disabled={loading}
            onClick={() => void clearKey()}
            type="button"
          >
            <Trash2 size={14} />
            Clear
          </button>
        </div>
      ) : null}
    </SettingsPanel>
  );
}

function GmailConnectionPanel() {
  const [status, setStatus] = useState<GmailStatus | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    gmailStatus().then(setStatus).catch((error) => setMessage({ ok: false, text: String(error) }));
  }, []);

  async function connect() {
    setLoading(true);
    setMessage(null);
    try {
      setStatus(await gmailConnect());
      setMessage({ ok: true, text: "Gmail connected." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  async function disconnect() {
    setLoading(true);
    setMessage(null);
    try {
      setStatus(await gmailDisconnect());
      setMessage({ ok: true, text: "Gmail disconnected." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  return (
    <SettingsPanel
      description="Account access for local mail sync, approved send actions, and Work Mail intake."
      icon={Mail}
      id="gmail-account"
      status={
        <StatusPill tone={status?.connected ? "ok" : "muted"}>
          {status?.connected ? "Connected" : "Not connected"}
        </StatusPill>
      }
      title="Gmail account"
    >
      {message ? <Feedback message={message} /> : null}
      {status?.connected ? (
        <div className="space-y-3">
          <DetailStrip
            items={[
              ["Account", status.settings?.account_email ?? "Unknown"],
              ["Last sync", status.settings?.last_sync_completed_at ?? "Never"],
            ]}
          />
          <div className="flex flex-wrap gap-2">
            <Link className="btn btn-primary" to="/settings/email?focus=gmail-sync">
              Tune mail sync
            </Link>
            <button
              className="btn btn-danger"
              disabled={loading}
              onClick={() => void disconnect()}
              type="button"
            >
              {loading ? <Loader2 className="animate-spin" size={14} /> : <Trash2 size={14} />}
              Disconnect
            </button>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          <p className="text-sm leading-6 text-zinc-500">
            Google sign-in opens in a browser window. Trace requests read access for local
            indexing and send access only for approved compose actions.
          </p>
          <button
            className="btn btn-primary"
            disabled={loading}
            onClick={() => void connect()}
            type="button"
          >
            {loading ? <Loader2 className="animate-spin" size={14} /> : <Mail size={14} />}
            {loading ? "Waiting for browser..." : "Connect Gmail"}
          </button>
        </div>
      )}
    </SettingsPanel>
  );
}

function DriveConnectionPanel() {
  const [status, setStatus] = useState<DriveStatus | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [loading, setLoading] = useState(false);
  const [disconnecting, setDisconnecting] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    try {
      setStatus(await driveStatus());
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function connect() {
    setLoading(true);
    setMessage(null);
    try {
      await driveConnect();
      await load();
      setMessage({ ok: true, text: "Google Drive connected." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
      setLoading(false);
    }
  }

  async function disconnect(accountId: string, email: string) {
    if (!confirm(`Disconnect ${email} from Trace? Imported Drive files will be removed.`)) return;
    setDisconnecting(accountId);
    setMessage(null);
    try {
      await disconnectGoogleDrive(accountId);
      await load();
      setMessage({ ok: true, text: "Google Drive disconnected." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setDisconnecting(null);
    }
  }

  return (
    <SettingsPanel
      description="Browse Drive files beside local files and attach them to work entities."
      icon={CloudCog}
      id="drive-account"
      status={
        <StatusPill tone={status?.connected ? "ok" : "muted"}>
          {status?.connected ? "Connected" : "Not connected"}
        </StatusPill>
      }
      title="Google Drive"
    >
      {message ? <Feedback message={message} /> : null}
      {status?.accounts.length ? (
        <div className="space-y-2">
          {status.accounts.map((account) => (
            <div
              className="flex flex-col gap-2 rounded-xl border border-zinc-100 bg-zinc-50 px-3 py-2.5 sm:flex-row sm:items-center sm:justify-between"
              key={account.id}
            >
              <div>
                <p className="text-[13px] font-semibold text-zinc-900">{account.email}</p>
                <p className="text-[11px] text-zinc-400">Connected locally</p>
              </div>
              <button
                className="btn btn-danger"
                disabled={disconnecting === account.id}
                onClick={() => void disconnect(account.id, account.email)}
                type="button"
              >
                {disconnecting === account.id ? (
                  <Loader2 className="animate-spin" size={14} />
                ) : (
                  <Trash2 size={14} />
                )}
                Disconnect
              </button>
            </div>
          ))}
        </div>
      ) : (
        <p className="text-sm leading-6 text-zinc-500">
          Drive uses read access so Trace can browse and import the documents you choose.
        </p>
      )}
      <div className="flex flex-wrap gap-2 pt-1">
        <button className="btn btn-primary" disabled={loading} onClick={() => void connect()} type="button">
          {loading ? <Loader2 className="animate-spin" size={14} /> : <CloudCog size={14} />}
          Connect Drive
        </button>
        <Link className="btn" to="/files">
          Open Files
        </Link>
      </div>
    </SettingsPanel>
  );
}

function CalendarConnectionPanel() {
  const [status, setStatus] = useState<GCalStatus | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    gcalStatus().then(setStatus).catch((error) => setMessage({ ok: false, text: String(error) }));
  }, []);

  async function connect() {
    setConnecting(true);
    setMessage(null);
    try {
      setStatus(await gcalConnect());
      setMessage({ ok: true, text: "Google Calendar connected." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setConnecting(false);
    }
  }

  async function disconnect() {
    setConnecting(true);
    setMessage(null);
    try {
      setStatus(await gcalDisconnect());
      setMessage({ ok: true, text: "Google Calendar disconnected." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setConnecting(false);
    }
  }

  async function sync() {
    setSyncing(true);
    setMessage(null);
    try {
      await gcalSync();
      setMessage({ ok: true, text: "Google Calendar synced." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setSyncing(false);
    }
  }

  return (
    <SettingsPanel
      description="Week planning and scheduling use the primary Google Calendar cache."
      icon={CalendarDays}
      id="calendar-account"
      status={
        <StatusPill tone={status?.connected ? "ok" : "muted"}>
          {status?.connected ? "Connected" : "Not connected"}
        </StatusPill>
      }
      title="Google Calendar"
    >
      {message ? <Feedback message={message} /> : null}
      <div className="flex flex-wrap gap-2">
        {status?.connected ? (
          <>
            <button className="btn btn-primary" disabled={syncing} onClick={() => void sync()} type="button">
              {syncing ? <Loader2 className="animate-spin" size={14} /> : <RefreshCw size={14} />}
              Sync calendar
            </button>
            <button
              className="btn btn-danger"
              disabled={connecting}
              onClick={() => void disconnect()}
              type="button"
            >
              {connecting ? <Loader2 className="animate-spin" size={14} /> : <Trash2 size={14} />}
              Disconnect
            </button>
          </>
        ) : (
          <button
            className="btn btn-primary"
            disabled={connecting}
            onClick={() => void connect()}
            type="button"
          >
            {connecting ? <Loader2 className="animate-spin" size={14} /> : <CalendarDays size={14} />}
            Connect Calendar
          </button>
        )}
        <Link className="btn" to="/week">
          Open Week
        </Link>
      </div>
    </SettingsPanel>
  );
}

function AppleNotesPanel() {
  const [syncing, setSyncing] = useState(false);
  const [settingUp, setSettingUp] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  async function handleSetup() {
    setSettingUp(true);
    setMessage(null);
    try {
      const [inbox, captured] = await appleNotesSetup();
      setMessage({
        ok: true,
        text: `Created "${inbox}" and "${captured}" folders in Apple Notes.`,
      });
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setSettingUp(false);
    }
  }

  async function handleSyncNow() {
    setSyncing(true);
    setMessage(null);
    try {
      const count = await appleNotesSyncNow();
      setMessage({
        ok: true,
        text:
          count === 0
            ? "No new notes found."
            : `Imported ${count} note${count === 1 ? "" : "s"} into Capture.`,
      });
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setSyncing(false);
    }
  }

  return (
    <SettingsPanel
      id="apple-notes"
      icon={NotebookText}
      title="Apple Notes"
      description='Drop notes into a "Trace" folder in Apple Notes and they appear in Capture automatically.'
    >
      <div className="space-y-4">
        {/* How it works */}
        <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4 space-y-3">
          <p className="text-sm font-medium text-zinc-700">How it works</p>
          <ol className="space-y-1.5 text-sm text-zinc-500 list-decimal list-inside">
            <li>
              Click <span className="font-medium text-zinc-700">Set up folders</span> — Trace
              creates a <span className="font-medium text-zinc-700">Trace</span> folder and a{" "}
              <span className="font-medium text-zinc-700">Trace — Captured</span> folder in
              Apple Notes.
            </li>
            <li>
              Drop any note into <span className="font-medium text-zinc-700">Trace</span> on
              your Mac (or iPhone via iCloud).
            </li>
            <li>
              Trace polls every 60 seconds and imports new notes as{" "}
              <span className="font-medium text-zinc-700">Thought</span> captures, then moves
              the note to <span className="font-medium text-zinc-700">Trace — Captured</span>.
            </li>
          </ol>
        </div>

        {/* Actions */}
        <div className="flex flex-wrap gap-2">
          <button
            className="btn"
            onClick={() => void handleSetup()}
            disabled={settingUp || syncing}
          >
            {settingUp ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <NotebookText size={14} />
            )}
            Set up folders
          </button>
          <button
            className="btn"
            onClick={() => void handleSyncNow()}
            disabled={syncing || settingUp}
          >
            {syncing ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <RefreshCw size={14} />
            )}
            Sync now
          </button>
        </div>

        {/* Feedback */}
        {message && (
          <p
            className={`flex items-center gap-1.5 text-sm ${
              message.ok ? "text-emerald-600" : "text-red-600"
            }`}
          >
            {message.ok ? <Check size={14} /> : <XCircle size={14} />}
            {message.text}
          </p>
        )}
      </div>
    </SettingsPanel>
  );
}

function SiriRemoteAccessPanel() {
  const [status, setStatus] = useState<SiriApiStatus | null>(null);
  const [fullToken, setFullToken] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [copiedField, setCopiedField] = useState<"url" | "token" | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [loading, setLoading] = useState(false);

  async function load() {
    setLoading(true);
    try {
      setStatus(await getSiriApiStatus());
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function revealToken() {
    try {
      const token = await getSiriToken();
      setFullToken(token);
      setRevealed(true);
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    }
  }

  function hideToken() {
    setRevealed(false);
    setFullToken(null);
  }

  async function regenerate() {
    if (
      !window.confirm(
        "Regenerate the token? Any Shortcut still using the old value will start receiving 401 errors until you update it.",
      )
    ) {
      return;
    }
    setLoading(true);
    setMessage(null);
    try {
      const token = await regenerateSiriToken();
      setFullToken(token);
      setRevealed(true);
      setStatus(await getSiriApiStatus());
      setMessage({ ok: true, text: "New token generated. Update your Shortcut." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  async function copy(value: string, field: "url" | "token") {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedField(field);
      window.setTimeout(() => setCopiedField(null), 2000);
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    }
  }

  const tone: "ok" | "warn" = status?.running ? "ok" : "warn";
  const statusLabel = status?.running ? "Reachable over Tailscale" : "Tailscale not detected";

  return (
    <SettingsPanel
      description="Reach Trace from your iPhone or Mac via Siri Shortcuts — encrypted over Tailscale, no public hosting."
      icon={Smartphone}
      id="siri-remote-access"
      status={<StatusPill tone={tone}>{statusLabel}</StatusPill>}
      title="Siri & Apple Shortcuts"
    >
      {message ? <Feedback message={message} /> : null}

      {!status?.running ? (
        <div className="rounded-xl border border-amber-100 bg-amber-50 px-3 py-2.5 text-[12px] leading-5 text-amber-900">
          Install <span className="font-mono">tailscale</span> and sign in on this Mac, then quit and
          relaunch Trace. The remote API refuses to start until a Tailscale interface is detected,
          so the socket is never reachable from a public network.
        </div>
      ) : null}

      <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4 space-y-3">
        <div>
          <p className="field-label">Tailscale URL</p>
          <div className="mt-1 flex items-center gap-2">
            <code className="flex-1 truncate rounded-lg bg-white px-3 py-2 font-mono text-[12px] text-zinc-700 border border-zinc-100">
              {status?.tailscale_url ?? "—"}
            </code>
            <button
              className="btn"
              disabled={!status?.tailscale_url}
              onClick={() => status?.tailscale_url && void copy(status.tailscale_url, "url")}
              type="button"
            >
              {copiedField === "url" ? <Check className="text-emerald-600" size={13} /> : <Clipboard size={13} />}
              {copiedField === "url" ? "Copied" : "Copy"}
            </button>
          </div>
          <p className="mt-1 text-[11px] leading-5 text-zinc-500">
            Paste this into the &ldquo;Get Contents of URL&rdquo; action in your Shortcut. Routes are
            <span className="font-mono"> /ask </span> and <span className="font-mono">/capture</span>.
          </p>
        </div>

        <div>
          <p className="field-label">Bearer token</p>
          <div className="mt-1 flex items-center gap-2">
            <code className="flex-1 truncate rounded-lg bg-white px-3 py-2 font-mono text-[12px] text-zinc-700 border border-zinc-100">
              {revealed && fullToken ? fullToken : status?.token_preview ?? "—"}
            </code>
            {revealed ? (
              <button className="btn" onClick={hideToken} type="button">
                Hide
              </button>
            ) : (
              <button className="btn" onClick={() => void revealToken()} type="button">
                Reveal
              </button>
            )}
            <button
              className="btn"
              disabled={!revealed || !fullToken}
              onClick={() => fullToken && void copy(fullToken, "token")}
              type="button"
            >
              {copiedField === "token" ? <Check className="text-emerald-600" size={13} /> : <Clipboard size={13} />}
              {copiedField === "token" ? "Copied" : "Copy"}
            </button>
          </div>
          <p className="mt-1 text-[11px] leading-5 text-zinc-500">
            Add as an <span className="font-mono">Authorization: Bearer …</span> header in the
            Shortcut. Treat like a password.
          </p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <button className="btn" disabled={loading} onClick={() => void load()} type="button">
          {loading ? <Loader2 className="animate-spin" size={14} /> : <RefreshCw size={14} />}
          Refresh
        </button>
        <button className="btn btn-danger" disabled={loading} onClick={() => void regenerate()} type="button">
          <RefreshCw size={14} />
          Regenerate token
        </button>
      </div>
    </SettingsPanel>
  );
}

function McpServerPanel() {
  const [installState, setInstallState] = useState<McpInstallState | null>(null);
  const [snippet, setSnippet] = useState<McpConfigSnippet | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const [nextInstallState, nextSnippet] = await Promise.all([
        getMcpInstallState(),
        getMcpConfigSnippet(),
      ]);
      setInstallState(nextInstallState);
      setSnippet(nextSnippet);
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function reinstall() {
    setLoading(true);
    setMessage(null);
    try {
      const nextInstallState = await installMcpServer();
      setInstallState(nextInstallState);
      setSnippet(await getMcpConfigSnippet());
      setMessage({
        ok: !nextInstallState.last_error,
        text: nextInstallState.last_error ?? "MCP server reinstalled.",
      });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setLoading(false);
    }
  }

  async function copySnippet() {
    if (!snippet) return;
    try {
      await navigator.clipboard.writeText(snippet.snippet);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    }
  }

  async function openLog() {
    try {
      await openMcpLog();
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    }
  }

  return (
    <SettingsPanel
      description="Claude Desktop can call Trace through the local MCP server."
      icon={Server}
      id="mcp-server"
      status={
        <StatusPill tone={installState?.last_error ? "bad" : installState?.installed ? "ok" : "warn"}>
          {installState?.installed ? "Installed" : "Not installed"}
        </StatusPill>
      }
      title="MCP server"
    >
      {message ? <Feedback message={message} /> : null}
      <div className="flex flex-wrap gap-2">
        <button className="btn" disabled={loading} onClick={() => void reinstall()} type="button">
          {loading ? <Loader2 className="animate-spin" size={14} /> : <RefreshCw size={14} />}
          Reinstall
        </button>
        <button className="btn" onClick={() => void openLog()} type="button">
          <Eye size={14} />
          View log
        </button>
        <button className="btn" disabled={loading} onClick={() => void load()} type="button">
          <RefreshCw size={14} />
          Refresh state
        </button>
      </div>

      <details className="rounded-xl border border-zinc-100 bg-zinc-50/70">
        <summary className="cursor-pointer px-3 py-2.5 text-[12px] font-semibold text-zinc-700">
          Raw install details and Claude snippet
        </summary>
        <div className="space-y-4 border-t border-zinc-100 px-3 py-3">
          {installState ? (
            <dl className="space-y-2 text-[12px]">
              <PathRow label="Binary" value={installState.binary_path} />
              <PathRow label="Database" value={installState.database_path} />
              <PathRow label="Log" value={installState.log_path} />
              {installState.last_error ? (
                <div className="rounded-lg bg-amber-50 px-3 py-2 text-amber-800">
                  {installState.last_error}
                </div>
              ) : null}
            </dl>
          ) : null}

          {snippet ? (
            <div className="space-y-2">
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-[11px] text-zinc-500">
                  Paste into <span className="font-mono text-zinc-700">{snippet.config_path}</span>
                </p>
                <button className="btn text-[11px]" onClick={() => void copySnippet()} type="button">
                  {copied ? <Check className="text-emerald-600" size={13} /> : <Clipboard size={13} />}
                  {copied ? "Copied" : "Copy"}
                </button>
              </div>
              <pre className="max-h-52 overflow-auto rounded-lg bg-zinc-950 p-3 text-[11px] leading-5 text-zinc-100">
                {snippet.snippet}
              </pre>
            </div>
          ) : null}
        </div>
      </details>
    </SettingsPanel>
  );
}

function GmailSyncPanel() {
  const [status, setStatus] = useState<GmailStatus | null>(null);
  const [saving, setSaving] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [message, setMessage] = useState<Message | null>(null);

  useEffect(() => {
    gmailStatus().then(setStatus).catch((error) => setMessage({ ok: false, text: String(error) }));
  }, []);

  async function save(input: GmailSyncSettingsInput) {
    setSaving(true);
    setMessage(null);
    try {
      const settings = await gmailUpdateSyncSettings(input);
      setStatus((current) => (current ? { ...current, settings } : current));
      setMessage({ ok: true, text: "Gmail sync setting saved." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setSaving(false);
    }
  }

  async function syncNow() {
    setSyncing(true);
    setMessage(null);
    try {
      const report = await gmailSyncNow();
      const nextStatus = await gmailStatus();
      setStatus(nextStatus);
      setMessage({
        ok: true,
        text: `Synced ${report.synced_threads} relevant threads and ${report.synced_messages} messages. AI analyzed ${report.ai_analyzed_threads}; backfilled ${report.backfilled_threads} older threads.`,
      });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setSyncing(false);
    }
  }

  return (
    <SettingsPanel
      description="Canonical Work Mail sync and analysis behavior. Contextual Email shortcuts reuse these controls."
      icon={Mail}
      id="gmail-sync"
      status={
        <StatusPill tone={status?.connected ? "ok" : "muted"}>
          {status?.connected ? "Connected" : "Needs Gmail"}
        </StatusPill>
      }
      title="Gmail sync"
    >
      {message ? <Feedback message={message} /> : null}
      {status?.connected && status.settings ? (
        <>
          <DetailStrip
            items={[
              ["Account", status.settings.account_email ?? "Unknown"],
              ["Last sync", status.settings.last_sync_completed_at ?? "Never"],
              [
                "Backfill",
                status.settings.backfill_completed_at
                  ? `Complete ${status.settings.backfill_completed_at}`
                  : status.settings.last_backfill_at ?? "Not started",
              ],
            ]}
          />
          {status.settings.last_error ? (
            <div className="rounded-xl bg-red-50 px-3 py-2 text-[12px] text-red-700">
              {status.settings.last_error}
            </div>
          ) : null}
          <GmailSyncSettingsControls disabled={saving} onChange={(input) => void save(input)} settings={status.settings} />
          <div className="flex flex-wrap gap-2">
            <button className="btn btn-primary" disabled={syncing} onClick={() => void syncNow()} type="button">
              {syncing ? <Loader2 className="animate-spin" size={14} /> : <RefreshCw size={14} />}
              Sync now
            </button>
            <Link className="btn" to="/email">
              Open Work Mail
            </Link>
          </div>
        </>
      ) : (
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <p className="text-sm text-zinc-500">
            Connect Gmail first, then this page owns the complete sync surface.
          </p>
          <Link className="btn btn-primary" to="/settings/connections?focus=gmail-account">
            Connect Gmail
          </Link>
        </div>
      )}
    </SettingsPanel>
  );
}

function MemoryPolicyPanel() {
  const [settings, setSettings] = useState<MemorySettings | null>(null);
  const [draft, setDraft] = useState<UpdateMemorySettingsInput | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getMemorySettings()
      .then((next) => {
        setSettings(next);
        setDraft(memoryDraft(next));
      })
      .catch((error) => setMessage({ ok: false, text: String(error) }));
  }, []);

  async function save() {
    if (!draft) return;
    setSaving(true);
    setMessage(null);
    try {
      const next = await updateMemorySettings(draft);
      setSettings(next);
      setDraft(memoryDraft(next));
      setMessage({ ok: true, text: "Memory policy saved." });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setSaving(false);
    }
  }

  return (
    <SettingsPanel
      description="Control automatic memory capture and retrieval behavior without managing memory content here."
      icon={Brain}
      id="memory-policy"
      status={<StatusPill tone={draft?.enabled ? "ok" : "warn"}>{draft?.enabled ? "On" : "Off"}</StatusPill>}
      title="Memory policy"
    >
      {message ? <Feedback message={message} /> : null}
      {draft ? (
        <>
          <div className="grid gap-2 md:grid-cols-2">
            <MemoryToggle
              checked={draft.enabled}
              description="Enable stored work memory and recall in Trace."
              label="Memory enabled"
              onChange={(enabled) => setDraft({ ...draft, enabled })}
            />
            <MemoryToggle
              checked={draft.auto_extract_enabled}
              description="Extract durable facts from supported work activity."
              label="Automatic extraction"
              onChange={(auto_extract_enabled) => setDraft({ ...draft, auto_extract_enabled })}
            />
            <MemoryToggle
              checked={draft.work_related_only}
              description="Bias automatic memory to work-relevant content."
              label="Work-related only"
              onChange={(work_related_only) => setDraft({ ...draft, work_related_only })}
            />
            <MemoryToggle
              checked={draft.preserve_continuity}
              description="Keep useful historical context available longer."
              label="Preserve continuity"
              onChange={(preserve_continuity) => setDraft({ ...draft, preserve_continuity })}
            />
            <MemoryToggle
              checked={draft.require_confirmation}
              description="Require confirmation before memory writes when policy asks for it."
              label="Require confirmation"
              onChange={(require_confirmation) => setDraft({ ...draft, require_confirmation })}
            />
            <label className="rounded-xl border border-zinc-200 px-3 py-2.5">
              <span className="block text-[12px] font-semibold text-zinc-800">Retrieval limit</span>
              <span className="mt-0.5 block text-[11px] leading-4 text-zinc-400">
                Maximum memories returned for a recall step.
              </span>
              <input
                className="field-control mt-2"
                max={40}
                min={1}
                onChange={(event) =>
                  setDraft({ ...draft, retrieval_limit: Number(event.currentTarget.value) })
                }
                type="number"
                value={draft.retrieval_limit}
              />
            </label>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-2 pt-1">
            <p className="text-[11px] text-zinc-400">
              {settings ? `Last updated ${settings.updated_at}` : "Loading policy state..."}
            </p>
            <div className="flex flex-wrap gap-2">
              <Link className="btn" to="/context">
                Open Memory
              </Link>
              <button className="btn btn-primary" disabled={saving} onClick={() => void save()} type="button">
                {saving ? <Loader2 className="animate-spin" size={14} /> : <Check size={14} />}
                Save policy
              </button>
            </div>
          </div>
        </>
      ) : (
        <div className="space-y-2">
          <div className="skeleton h-14 rounded-xl" />
          <div className="skeleton h-14 rounded-xl" />
        </div>
      )}
    </SettingsPanel>
  );
}

function SettingsPanel({
  children,
  description,
  icon: Icon,
  id,
  status,
  title,
}: {
  children: ReactNode;
  description: string;
  icon: LucideIcon;
  id: string;
  status?: ReactNode;
  title: string;
}) {
  return (
    <section
      className="scroll-mt-24 overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] outline-none"
      id={id}
      tabIndex={-1}
    >
      <div className="flex flex-col gap-3 border-b border-zinc-100 px-5 py-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex items-start gap-3">
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-100 text-zinc-600">
            <Icon size={15} />
          </div>
          <div>
            <h3 className="text-[13px] font-semibold text-zinc-900">{title}</h3>
            <p className="mt-0.5 text-[11px] leading-5 text-zinc-400">{description}</p>
          </div>
        </div>
        {status}
      </div>
      <div className="space-y-4 px-5 py-4">{children}</div>
    </section>
  );
}

function Anchor({ children, id }: { children: ReactNode; id: string }) {
  return (
    <div className="scroll-mt-24 outline-none" id={id} tabIndex={-1}>
      {children}
    </div>
  );
}

function OverviewCard({
  description,
  icon: Icon,
  id,
  label,
  loading,
  status,
  to,
  tone,
}: OverviewCardProps & { loading: boolean }) {
  return (
    <Link
      className="group rounded-2xl border border-zinc-100 bg-white px-4 py-4 shadow-[0_2px_12px_rgba(0,0,0,0.04)] transition-colors hover:border-zinc-200 hover:bg-zinc-50/70"
      id={id}
      to={to}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-zinc-100 text-zinc-600 transition-colors group-hover:bg-white">
            <Icon size={16} />
          </div>
          <div>
            <p className="text-[13px] font-semibold text-zinc-950">{label}</p>
            <p className="mt-0.5 text-[11px] leading-5 text-zinc-400">{description}</p>
          </div>
        </div>
        {loading ? <span className="skeleton h-6 w-20 rounded-full" /> : <StatusPill tone={tone}>{status}</StatusPill>}
      </div>
    </Link>
  );
}

function StatusPill({
  children,
  tone,
}: {
  children: ReactNode;
  tone: "bad" | "muted" | "ok" | "warn";
}) {
  const className =
    tone === "ok"
      ? "bg-emerald-50 text-emerald-700"
      : tone === "bad"
        ? "bg-red-50 text-red-700"
        : tone === "warn"
          ? "bg-amber-50 text-amber-700"
          : "bg-zinc-100 text-zinc-500";
  return (
    <span className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-1 text-[11px] font-semibold ${className}`}>
      {children}
    </span>
  );
}

function Feedback({ message }: { message: Message }) {
  return (
    <div
      className={[
        "flex items-start gap-2 rounded-xl px-3 py-2 text-[12px]",
        message.ok ? "bg-emerald-50 text-emerald-800" : "bg-red-50 text-red-700",
      ].join(" ")}
    >
      {message.ok ? <CheckCircle2 className="mt-0.5 shrink-0" size={13} /> : <XCircle className="mt-0.5 shrink-0" size={13} />}
      <span>{message.text}</span>
    </div>
  );
}

function DetailStrip({ items }: { items: Array<[string, string]> }) {
  return (
    <dl className="grid gap-2 rounded-xl border border-zinc-100 bg-zinc-50 p-3 text-[12px] text-zinc-600 sm:grid-cols-2">
      {items.map(([label, value]) => (
        <PathRow key={label} label={label} value={value} />
      ))}
    </dl>
  );
}

function PathRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-0 items-baseline gap-2">
      <dt className="w-20 shrink-0 text-zinc-400">{label}</dt>
      <dd className="break-all font-mono text-[11px] text-zinc-600">{value}</dd>
    </div>
  );
}

function MemoryToggle({
  checked,
  description,
  label,
  onChange,
}: {
  checked: boolean;
  description: string;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-start justify-between gap-3 rounded-xl border border-zinc-200 px-3 py-2.5">
      <span>
        <span className="block text-[12px] font-semibold text-zinc-800">{label}</span>
        <span className="mt-0.5 block text-[11px] leading-4 text-zinc-400">{description}</span>
      </span>
      <input
        checked={checked}
        className="mt-0.5 h-4 w-4 shrink-0 accent-zinc-900"
        onChange={(event) => onChange(event.currentTarget.checked)}
        type="checkbox"
      />
    </label>
  );
}

interface Message {
  ok: boolean;
  text: string;
}

interface OverviewSnapshot {
  budget: BudgetStatus | null;
  calendar: GCalStatus | null;
  drive: DriveStatus | null;
  embeddings: EmbeddingProgress | null;
  gemini: GeminiKeyStatus | null;
  gmail: GmailStatus | null;
  mcp: McpInstallState | null;
}

interface OverviewCardProps {
  description: string;
  icon: LucideIcon;
  id: string;
  label: string;
  status: string;
  to: string;
  tone: "bad" | "muted" | "ok" | "warn";
}

function pageForPath(pathname: string) {
  if (pathname === "/settings" || pathname === "/settings/") {
    return pageById("overview");
  }
  if (pathname === "/settings/mcp") {
    return pageById("connections");
  }
  return (
    SETTINGS_PAGES.find((page) => page.id !== "overview" && pathname.startsWith(page.path)) ??
    pageById("overview")
  );
}

function pageById(id: SettingsPageId) {
  return SETTINGS_PAGES.find((page) => page.id === id) ?? SETTINGS_PAGES[0];
}

function settledValue<T>(result: PromiseSettledResult<T>) {
  return result.status === "fulfilled" ? result.value : null;
}

function budgetStatus(status: BudgetStatus | null | undefined) {
  if (!status) return "Unavailable";
  if (status.block_active) return "Blocked";
  if (status.alert_state === "exceeded") return "Over limit";
  if (status.alert_state === "warning") return "Near limit";
  return "Healthy";
}

function budgetTone(status: BudgetStatus | null | undefined): OverviewCardProps["tone"] {
  if (!status) return "muted";
  if (status.block_active || status.alert_state === "exceeded") return "bad";
  if (status.alert_state === "warning") return "warn";
  return "ok";
}

function budgetDescription(status: BudgetStatus | null | undefined) {
  if (!status) return "Current spend status is unavailable.";
  return `Today $${status.daily_spent_usd.toFixed(2)}. Month $${status.monthly_spent_usd.toFixed(2)}.`;
}

function embeddingStatus(progress: EmbeddingProgress | null | undefined) {
  if (!progress) return "Unavailable";
  if (progress.pending + progress.stale > 0) return "Work queued";
  return "Current";
}

function embeddingDescription(progress: EmbeddingProgress | null | undefined) {
  if (!progress) return "Semantic retrieval status is unavailable.";
  return `${progress.embedded}/${progress.total} embedded. ${progress.pending} pending, ${progress.stale} stale.`;
}

function memoryDraft(settings: MemorySettings): UpdateMemorySettingsInput {
  return {
    auto_extract_enabled: settings.auto_extract_enabled,
    enabled: settings.enabled,
    preserve_continuity: settings.preserve_continuity,
    require_confirmation: settings.require_confirmation,
    retrieval_limit: settings.retrieval_limit,
    work_related_only: settings.work_related_only,
  };
}
