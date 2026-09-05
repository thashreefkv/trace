import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type PointerEvent,
  type WheelEvent,
} from "react";
import {
  forceCollide,
  forceLink,
  forceManyBody,
  forceRadial,
  forceSimulation,
  forceX,
  forceY,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import {
  Brain,
  ChevronDown,
  Clipboard,
  Eye,
  History,
  Maximize2,
  Minus,
  Network,
  Pencil,
  Pin,
  Plus,
  RefreshCw,
  Search,
  Shield,
  SlidersHorizontal,
  Sparkles,
  ThumbsDown,
  ThumbsUp,
  Trash2,
  X,
  Zap,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { safeExternalUrl } from "../lib/urlSafety";
import {
  consolidateMemories,
  createMemory,
  deleteMemory,
  embedMemoriesNow,
  extractMemoriesFromConversation,
  getBrainGraph,
  getBrainStatus,
  getMemorySettings,
  listConversations,
  listMemories,
  listMemoryEvents,
  rebuildBrain,
  recordMemoryFeedback,
  retrieveMemories,
  runBrainTemplate,
  updateMemory,
  updateMemorySettings,
} from "../lib/ipc";
import type {
  Conversation,
  BrainTemplateKind,
  BrainTemplateResult,
  BrainStatus,
  MemoryEvent,
  MemoryFeedback,
  MemoryKind,
  MemoryRecord,
  MemoryRetrievalResult,
  MemorySensitivity,
  MemorySettings,
  WorkGraph,
  WorkGraphEdge,
  WorkGraphNode,
} from "../lib/types";
import { formatDateTime } from "../lib/format";
import { recordBrainImpression, recordBrainSignal } from "../lib/brainSignals";

const graphViewportWidth = 1280;
const graphViewportHeight = 760;
const graphFitPadding = 96;

type GraphLayoutMode = "force" | "hierarchy" | "radial";
type GraphDensity = "compact" | "balanced" | "spread";
type GraphLabelMode = "smart" | "selected" | "all";

const brainTemplateOptions: Array<{ value: BrainTemplateKind; label: string }> = [
  { value: "focus_today", label: "Focus today" },
  { value: "blocked_work", label: "Blocked" },
  { value: "email_followups", label: "Follow-ups" },
  { value: "stale_work", label: "Stale work" },
  { value: "stakeholder_context", label: "Stakeholders" },
];

interface BrainPreviewTarget {
  id: string;
  kind: string;
  label: string;
  subtitle: string | null;
  status: string | null;
  url: string | null;
  node: WorkGraphNode | null;
}

interface BrainPreviewState {
  template: string;
  row: Record<string, unknown>;
  rowId: string;
  node: WorkGraphNode | null;
  target: BrainPreviewTarget | null;
  related: WorkGraphNode[];
}

const baseKindLabels: Record<string, string> = {
  initiative: "Initiatives",
  deliverable: "Deliverables",
  stakeholder: "Stakeholders",
  conversation: "Conversations",
  capture: "Captures",
  memory: "Memory",
  task: "Tasks",
  note: "Notes",
  label: "Labels",
  meeting: "Meetings",
  meeting_action: "Actions",
  initiative_note: "Initiative notes",
  week_day: "Week plan",
  email_thread: "Email threads",
  email_participant: "Email people",
  email_draft: "Drafts",
  email_suggestion: "Email suggestions",
  ask_chat: "Ask chats",
  work_intake_suggestion: "Work intake",
  blocker: "Blockers",
};

const baseKindColors: Record<string, { fill: string; stroke: string; text: string }> = {
  initiative: { fill: "#e0f2fe", stroke: "#0284c7", text: "#075985" },
  deliverable: { fill: "#ecfdf5", stroke: "#059669", text: "#065f46" },
  stakeholder: { fill: "#fff7ed", stroke: "#ea580c", text: "#9a3412" },
  conversation: { fill: "#f5f3ff", stroke: "#7c3aed", text: "#5b21b6" },
  capture: { fill: "#fefce8", stroke: "#ca8a04", text: "#854d0e" },
  memory: { fill: "#ccfbf1", stroke: "#0f766e", text: "#115e59" },
  task: { fill: "#f0fdf4", stroke: "#16a34a", text: "#166534" },
  note: { fill: "#fafafa", stroke: "#737373", text: "#404040" },
  meeting: { fill: "#fef2f2", stroke: "#dc2626", text: "#991b1b" },
  meeting_action: { fill: "#fff1f2", stroke: "#e11d48", text: "#9f1239" },
  email_thread: { fill: "#eff6ff", stroke: "#2563eb", text: "#1e40af" },
  email_participant: { fill: "#f0f9ff", stroke: "#0ea5e9", text: "#0369a1" },
  ask_chat: { fill: "#eef2ff", stroke: "#4f46e5", text: "#3730a3" },
  blocker: { fill: "#fee2e2", stroke: "#ef4444", text: "#991b1b" },
};

const fallbackKindColors = [
  { fill: "#f0fdfa", stroke: "#0d9488", text: "#115e59" },
  { fill: "#fdf4ff", stroke: "#c026d3", text: "#86198f" },
  { fill: "#f8fafc", stroke: "#64748b", text: "#334155" },
  { fill: "#fefce8", stroke: "#ca8a04", text: "#854d0e" },
  { fill: "#fff7ed", stroke: "#f97316", text: "#9a3412" },
];

const memoryKindLabels: Record<MemoryKind, string> = {
  episodic: "Episodic",
  semantic: "Semantic",
  procedural: "Procedural",
};

export function WorkContextGraph() {
  const navigate = useNavigate();
  const [graph, setGraph] = useState<WorkGraph | null>(null);
  const [memories, setMemories] = useState<MemoryRecord[]>([]);
  const [settings, setSettings] = useState<MemorySettings | null>(null);
  const [query, setQuery] = useState("");
  const [memoryQuery, setMemoryQuery] = useState("");
  const [retrieval, setRetrieval] = useState<MemoryRetrievalResult | null>(null);
  const [feedbackBusy, setFeedbackBusy] = useState(false);
  const [feedbackSent, setFeedbackSent] = useState<MemoryFeedback | null>(null);
  const [events, setEvents] = useState<MemoryEvent[]>([]);
  const [eventsOpen, setEventsOpen] = useState(false);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [extractConvId, setExtractConvId] = useState<string>("");
  const [extractBusy, setExtractBusy] = useState(false);
  const [memorySearch, setMemorySearch] = useState("");
  const [memoriesCollapsed, setMemoriesCollapsed] = useState(true);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingMemoryId, setEditingMemoryId] = useState<string | null>(null);
  const [draftKind, setDraftKind] = useState<MemoryKind>("semantic");
  const [draftTitle, setDraftTitle] = useState("");
  const [draftBody, setDraftBody] = useState("");
  const [draftTags, setDraftTags] = useState("");
  const [draftSensitivity, setDraftSensitivity] = useState<MemorySensitivity>("normal");
  const [draftPinned, setDraftPinned] = useState(false);
  const [includeDismissedCaptures, setIncludeDismissedCaptures] = useState(false);
  const [includeKilledDeliverables, setIncludeKilledDeliverables] = useState(false);
  const [activeKinds, setActiveKinds] = useState<Set<string>>(() => new Set());
  const [activeRelations, setActiveRelations] = useState<Set<string>>(() => new Set());
  const [brainStatus, setBrainStatus] = useState<BrainStatus | null>(null);
  const [brainBusy, setBrainBusy] = useState(false);
  const [activeBrainTemplate, setActiveBrainTemplate] =
    useState<BrainTemplateKind>("focus_today");
  const [brainTemplate, setBrainTemplate] = useState<BrainTemplateResult | null>(null);
  const [brainTemplateBusy, setBrainTemplateBusy] = useState(false);
  const [brainTemplateFeedback, setBrainTemplateFeedback] = useState<Record<string, string>>({});
  const [brainPreview, setBrainPreview] = useState<BrainPreviewState | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [focusSelected, setFocusSelected] = useState(false);
  const [layoutMode, setLayoutMode] = useState<GraphLayoutMode>("force");
  const [graphDensity, setGraphDensity] = useState<GraphDensity>("balanced");
  const [labelMode, setLabelMode] = useState<GraphLabelMode>("smart");
  const [sceneLimit, setSceneLimit] = useState(360);
  const [graphTransform, setGraphTransform] = useState<GraphTransform>({
    x: 0,
    y: 0,
    k: 1,
  });
  const [pinnedPositions, setPinnedPositions] = useState<Record<string, Point>>({});
  const [message, setMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const graphFiltersInitialized = useRef(false);
  const graphSvgRef = useRef<SVGSVGElement | null>(null);
  const graphPointerRef = useRef<GraphPointerState | null>(null);

  useEffect(() => {
    void loadGraph();
    void loadBrainStatus();
    void loadBrainTemplate(activeBrainTemplate);
    void loadMemoryState();
    void loadEvents();
    void loadConversations();
  }, [includeDismissedCaptures, includeKilledDeliverables]);

  useEffect(() => {
    if (!brainTemplate) return;
    brainTemplate.rows.slice(0, 16).forEach((row, index) => {
      const itemId = rowString(row, "id");
      if (!itemId) return;
      recordBrainImpression({
        template: brainTemplate.template,
        itemId,
        itemKind: rowString(row, "kind"),
        eventType: "shown",
        context: {
          rank: index + 1,
          score: rowNumber(row, "brain_rl_score"),
          source: "brain_template_panel",
        },
      });
    });
  }, [brainTemplate]);

  useEffect(() => {
    function handleExtracted(event: Event) {
      const detail = (event as CustomEvent<{ created: number; updated: number; source_kind: string }>)
        .detail;
      const total = (detail?.created ?? 0) + (detail?.updated ?? 0);
      if (total > 0) {
        setMessage(
          `Memory auto-extracted ${total} fact${total === 1 ? "" : "s"} from ${detail.source_kind}.`,
        );
      }
      void loadMemoryState();
      void loadEvents();
    }
    window.addEventListener("trace:memory-extracted", handleExtracted);
    return () => window.removeEventListener("trace:memory-extracted", handleExtracted);
  }, []);

  async function loadConversations() {
    try {
      setConversations(await listConversations());
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function runExtraction() {
    if (!extractConvId) return;
    try {
      setExtractBusy(true);
      setError(null);
      setMessage(null);
      const result = await extractMemoriesFromConversation(extractConvId);
      setMessage(
        `Extracted memories: ${result.created_count} new, ${result.updated_count} updated, ${result.skipped_count} skipped.`,
      );
      await Promise.all([loadMemoryState(), loadGraph(), loadEvents()]);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setExtractBusy(false);
    }
  }

  async function loadGraph() {
    try {
      setError(null);
      setIsLoading(true);
      const nextGraph = await getBrainGraph({
        include_dismissed_captures: includeDismissedCaptures,
        include_killed_deliverables: includeKilledDeliverables,
      });
      setGraph(nextGraph);
      if (!graphFiltersInitialized.current) {
        const defaultKinds = new Set(
          nextGraph.nodes
            .filter((node) => !node.hidden_by_default)
            .map((node) => node.kind),
        );
        if (defaultKinds.size === 0) {
          nextGraph.nodes.forEach((node) => defaultKinds.add(node.kind));
        }
        const defaultRelations = new Set(nextGraph.edges.map((edge) => edge.kind));
        setActiveKinds(defaultKinds);
        setActiveRelations(defaultRelations);
        graphFiltersInitialized.current = true;
      }
      void loadBrainStatus();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsLoading(false);
    }
  }

  async function loadBrainStatus() {
    try {
      setBrainStatus(await getBrainStatus());
    } catch {
      // Status is supplemental; graph load errors are surfaced separately.
    }
  }

  async function loadBrainTemplate(template: BrainTemplateKind = activeBrainTemplate) {
    try {
      setBrainTemplateBusy(true);
      setBrainTemplateFeedback({});
      const result = await runBrainTemplate({ template, limit: 48 });
      setBrainTemplate(result);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBrainTemplateBusy(false);
    }
  }

  function selectBrainTemplate(template: BrainTemplateKind) {
    setActiveBrainTemplate(template);
    void loadBrainTemplate(template);
  }

  function handleBrainRowPreview(row: Record<string, unknown>) {
    if (!brainTemplate) return;
    setBrainPreview(resolveBrainPreview(brainTemplate, row));
  }

  function openBrainPreviewTarget() {
    if (!brainPreview?.target) return;
    const target = brainPreview.target;
    const itemId = brainPreview.rowId || target.id;
    void recordBrainSignal({
      template: brainPreview.template,
      itemId,
      itemKind: target.kind || rowString(brainPreview.row, "kind"),
      eventType: "opened",
      context: {
        opened_node_id: target.node?.id ?? target.id,
        score: rowNumber(brainPreview.row, "brain_rl_score"),
        source: "brain_preview",
      },
    });
    setBrainPreview(null);
    if (target.url) {
      openGraphUrl(target.url);
      return;
    }
    if (target.node) {
      setSelectedId(target.node.id);
    }
  }

  async function handleBrainRowFeedback(
    row: Record<string, unknown>,
    eventType: "useful" | "wrong" | "ignored",
  ) {
    const itemId = rowString(row, "id");
    if (!itemId || !brainTemplate) return;
    setBrainTemplateFeedback((current) => ({ ...current, [itemId]: eventType }));
    await recordBrainSignal({
      template: brainTemplate.template,
      itemId,
      itemKind: rowString(row, "kind"),
      eventType,
      context: {
        score: rowNumber(row, "brain_rl_score"),
        source: "brain_template_panel",
      },
    });
  }

  async function runBrainRebuild() {
    try {
      setError(null);
      setMessage(null);
      setBrainBusy(true);
      const status = await rebuildBrain();
      setBrainStatus(status);
      setMessage(`Rebuilt brain graph: ${status.node_count} nodes, ${status.edge_count} relations.`);
      await loadGraph();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setBrainBusy(false);
    }
  }

  async function loadMemoryState() {
    try {
      const [nextSettings, nextMemories] = await Promise.all([
        getMemorySettings(),
        listMemories({ include_archived: false }),
      ]);
      setSettings(nextSettings);
      setMemories(nextMemories);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function runConsolidation() {
    try {
      setError(null);
      setMessage(null);
      const result = await consolidateMemories();
      setMessage(
        `Consolidated memory: ${result.created_count} created, ${result.updated_count} updated, ${result.archived_count} archived.`,
      );
      await Promise.all([loadMemoryState(), loadGraph()]);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function loadEvents() {
    try {
      setEvents(await listMemoryEvents({ limit: 60 }));
    } catch (caught) {
      // Events are non-critical; surface but don't block.
      setError(String(caught));
    }
  }

  async function runRetrieval() {
    const nextQuery = memoryQuery.trim();
    if (!nextQuery) {
      setRetrieval(null);
      setFeedbackSent(null);
      return;
    }
    try {
      setError(null);
      setFeedbackSent(null);
      setRetrieval(
        await retrieveMemories({
          query: nextQuery,
          limit: 16,
          source_kind: "memory_panel",
          include_pinned: true,
        }),
      );
      await loadMemoryState();
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function sendFeedback(value: MemoryFeedback) {
    if (!retrieval?.retrieval_id) return;
    try {
      setFeedbackBusy(true);
      await recordMemoryFeedback({ retrieval_id: retrieval.retrieval_id, feedback: value });
      setFeedbackSent(value);
      await loadEvents();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setFeedbackBusy(false);
    }
  }

  async function runEmbedAll() {
    try {
      setError(null);
      setMessage(null);
      const count = await embedMemoriesNow();
      setMessage(
        count > 0 ? `Embedded ${count} memories with Gemini.` : "All active memories already embedded.",
      );
      await loadEvents();
    } catch (caught) {
      setError(String(caught));
    }
  }

  function resetDraftMemory() {
    setDraftKind("semantic");
    setDraftTitle("");
    setDraftBody("");
    setDraftTags("");
    setDraftSensitivity("normal");
    setDraftPinned(false);
  }

  function openNewMemoryEditor() {
    setEditingMemoryId(null);
    resetDraftMemory();
    setEditorOpen(true);
  }

  function openEditMemoryEditor(memory: MemoryRecord) {
    setEditingMemoryId(memory.id);
    setDraftKind(memory.kind);
    setDraftTitle(memory.title);
    setDraftBody(memory.body);
    setDraftTags(memory.tags.join(", "));
    setDraftSensitivity(memory.sensitivity);
    setDraftPinned(memory.pinned);
    setEditorOpen(true);
  }

  function closeMemoryEditor() {
    setEditorOpen(false);
    setEditingMemoryId(null);
    resetDraftMemory();
  }

  async function saveDraftMemory(event: FormEvent) {
    event.preventDefault();
    const title = draftTitle.trim();
    const body = draftBody.trim();
    if (!title || !body) {
      return;
    }
    try {
      setError(null);
      const tags = draftTags
        .split(",")
        .map((tag) => tag.trim())
        .filter(Boolean);

      if (editingMemoryId) {
        const current = memories.find((memory) => memory.id === editingMemoryId);
        await updateMemory(editingMemoryId, {
          kind: draftKind,
          status: current?.status ?? "active",
          title,
          body,
          scope: current?.scope ?? "global",
          tags,
          confidence: current?.confidence ?? 0.95,
          importance: current?.importance ?? 0.85,
          sensitivity: draftSensitivity,
          pinned: draftPinned,
          ...(current?.expires_at ? { expires_at: current.expires_at } : {}),
        });
        setMessage("Updated memory.");
      } else {
        await createMemory({
          kind: draftKind,
          title,
          body,
          scope: "global",
          tags,
          confidence: 0.95,
          importance: 0.85,
          sensitivity: draftSensitivity,
          pinned: draftPinned,
        });
        setMessage("Saved memory.");
      }

      closeMemoryEditor();
      await Promise.all([loadMemoryState(), loadGraph(), loadEvents()]);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function removeMemory(id: string) {
    try {
      setError(null);
      await deleteMemory(id);
      setMessage("Deleted memory.");
      await Promise.all([loadMemoryState(), loadGraph()]);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function setMemoryEnabled(enabled: boolean) {
    if (!settings) {
      return;
    }
    try {
      const nextSettings = await updateMemorySettings({ ...settings, enabled });
      setSettings(nextSettings);
    } catch (caught) {
      setError(String(caught));
    }
  }

  const visibleNodes = useMemo(() => {
    if (!graph) {
      return [];
    }

    const normalizedQuery = query.trim().toLowerCase();
    const baseNodes = graph.nodes.filter((node) => {
      if (activeKinds.size === 0 || !activeKinds.has(node.kind)) {
        return false;
      }
      if (!normalizedQuery) {
        return true;
      }

      return [
        node.kind,
        node.label,
        node.subtitle ?? "",
        node.status ?? "",
        node.context,
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery);
    });

    const focusedNodes =
      focusSelected && selectedId
        ? baseNodes.filter((node) => focusedGraphNodeIds(graph.edges, selectedId).has(node.id))
        : baseNodes;

    return limitGraphNodes(focusedNodes, graph.edges, selectedId, sceneLimit);
  }, [activeKinds, focusSelected, graph, query, sceneLimit, selectedId]);

  const visibleNodeIds = useMemo(
    () => new Set(visibleNodes.map((node) => node.id)),
    [visibleNodes],
  );

  const visibleEdges = useMemo(() => {
    if (!graph) {
      return [];
    }

    return graph.edges.filter(
      (edge) =>
        visibleNodeIds.has(edge.source) &&
        visibleNodeIds.has(edge.target) &&
        (activeRelations.size === 0 || activeRelations.has(edge.kind)),
    );
  }, [activeRelations, graph, visibleNodeIds]);

  const layout = useMemo(
    () =>
      layoutGraph(visibleNodes, visibleEdges, {
        density: graphDensity,
        mode: layoutMode,
        pinnedPositions,
        selectedId,
      }),
    [graphDensity, layoutMode, pinnedPositions, selectedId, visibleEdges, visibleNodes],
  );
  const selectedNode = selectedId ? visibleNodes.find((node) => node.id === selectedId) ?? null : null;
  const selectedEdge = selectedEdgeId
    ? visibleEdges.find((edge) => edge.id === selectedEdgeId) ?? null
    : null;
  const visibleMemories = useMemo(() => {
    const normalizedQuery = memorySearch.trim().toLowerCase();
    const sorted = [...memories].sort((first, second) => {
      if (first.pinned !== second.pinned) {
        return first.pinned ? -1 : 1;
      }
      return second.updated_at.localeCompare(first.updated_at);
    });

    if (!normalizedQuery) {
      return sorted;
    }

    return sorted.filter((memory) =>
      [
        memory.title,
        memory.body,
        memory.kind,
        memory.scope,
        memory.source,
        memory.tags.join(" "),
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [memories, memorySearch]);
  const pinnedCount = useMemo(
    () => memories.filter((memory) => memory.pinned).length,
    [memories],
  );
  const allKinds = useMemo(
    () => Array.from(new Set(graph?.nodes.map((node) => node.kind) ?? [])).sort(),
    [graph],
  );
  const allRelations = useMemo(
    () => Array.from(new Set(graph?.edges.map((edge) => edge.kind) ?? [])).sort(),
    [graph],
  );

  const selectedEdges = useMemo(() => {
    if (!selectedNode) {
      return [];
    }

    return visibleEdges.filter(
      (edge) => edge.source === selectedNode.id || edge.target === selectedNode.id,
    );
  }, [selectedNode, visibleEdges]);

  const selectedNeighborhood = useMemo(
    () => (selectedNode ? focusedGraphNodeIds(visibleEdges, selectedNode.id) : new Set<string>()),
    [selectedNode, visibleEdges],
  );

  const sceneSignature = useMemo(
    () =>
      [
        layoutMode,
        graphDensity,
        sceneLimit,
        visibleNodes.map((node) => node.id).join("|"),
        visibleEdges.map((edge) => edge.id).join("|"),
      ].join("::"),
    [graphDensity, layoutMode, sceneLimit, visibleEdges, visibleNodes],
  );

  useEffect(() => {
    if (selectedId && !visibleNodeIds.has(selectedId)) {
      setSelectedId(null);
      setFocusSelected(false);
    }
  }, [selectedId, visibleNodeIds]);

  useEffect(() => {
    if (selectedEdgeId && !visibleEdges.some((edge) => edge.id === selectedEdgeId)) {
      setSelectedEdgeId(null);
    }
  }, [selectedEdgeId, visibleEdges]);

  useEffect(() => {
    setGraphTransform(fitGraphToViewport(layout.bounds));
  }, [sceneSignature]);

  function toggleKind(kind: string) {
    setActiveKinds((current) => {
      const next = new Set(current);
      if (next.has(kind)) {
        next.delete(kind);
      } else {
        next.add(kind);
      }
      return next;
    });
  }

  function toggleRelation(kind: string) {
    setActiveRelations((current) => {
      const next = new Set(current);
      if (next.has(kind)) {
        next.delete(kind);
      } else {
        next.add(kind);
      }
      return next;
    });
  }

  function openNode(node: WorkGraphNode, template: string = "brain_graph", sourceItemId?: string) {
    void recordBrainSignal({
      template,
      itemId: sourceItemId ?? node.id,
      itemKind: node.kind,
      eventType: "opened",
      context: {
        source: "graph_node",
        opened_node_id: node.id,
        score: brainRlScore(node),
      },
    });
    if (!node.url) {
      setSelectedId(node.id);
      return;
    }

    openGraphUrl(node.url);
  }

  function openGraphUrl(url: string) {
    if (url.startsWith("/")) {
      navigate(url);
    } else {
      const safeUrl = safeExternalUrl(url);
      if (safeUrl) window.open(safeUrl, "_blank", "noopener,noreferrer");
    }
  }

  async function copyAiContext() {
    if (!graph) {
      return;
    }

    const selectedContext = selectedNode
      ? `\n\nSelected node:\n${selectedNode.context}\n\nAdjacent edges:\n${selectedEdges
          .map((edge) => `- ${edge.kind}: ${edge.source} -> ${edge.target} (${edge.label})`)
          .join("\n")}`
      : "";

    await navigator.clipboard.writeText(`${graph.ai_context}${selectedContext}`);
    setMessage("Copied graph context for AI.");
  }

  async function copySelectedProperties() {
    const selected = selectedEdge ?? selectedNode;
    if (!selected) {
      return;
    }
    await navigator.clipboard.writeText(JSON.stringify(selected.properties ?? {}, null, 2));
    setMessage("Copied selected graph properties.");
  }

  function selectNode(node: WorkGraphNode) {
    void recordBrainSignal({
      template: "brain_graph",
      itemId: node.id,
      itemKind: node.kind,
      eventType: "clicked",
      context: {
        source: "graph_node",
        score: brainRlScore(node),
      },
    });
    setSelectedId(node.id);
    setSelectedEdgeId(null);
  }

  function selectEdge(edge: WorkGraphEdge) {
    setSelectedEdgeId(edge.id);
    setSelectedId(null);
    setFocusSelected(false);
  }

  function resetPinnedLayout() {
    setPinnedPositions({});
    setGraphTransform(fitGraphToViewport(layout.bounds));
  }

  function fitScene() {
    setGraphTransform(fitGraphToViewport(layout.bounds));
  }

  function zoomScene(multiplier: number) {
    setGraphTransform((current) => ({
      ...current,
      k: clamp(current.k * multiplier, 0.12, 3.8),
    }));
  }

  function handleGraphWheel(event: WheelEvent<SVGSVGElement>) {
    event.preventDefault();
    const viewportPoint = viewportPointFromPointer(event, graphSvgRef.current);
    if (!viewportPoint) {
      return;
    }
    setGraphTransform((current) => {
      const nextK = clamp(current.k * (event.deltaY > 0 ? 0.88 : 1.12), 0.12, 3.8);
      const graphPoint = {
        x: (viewportPoint.x - current.x) / current.k,
        y: (viewportPoint.y - current.y) / current.k,
      };
      return {
        k: nextK,
        x: viewportPoint.x - graphPoint.x * nextK,
        y: viewportPoint.y - graphPoint.y * nextK,
      };
    });
  }

  function handleGraphBackgroundPointerDown(event: PointerEvent<SVGRectElement>) {
    const viewportPoint = viewportPointFromPointer(event, graphSvgRef.current);
    if (!viewportPoint) {
      return;
    }
    setSelectedEdgeId(null);
    graphPointerRef.current = {
      kind: "pan",
      pointerId: event.pointerId,
      startViewport: viewportPoint,
      startTransform: graphTransform,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function handleNodePointerDown(node: WorkGraphNode, event: PointerEvent<SVGGElement>) {
    const viewportPoint = viewportPointFromPointer(event, graphSvgRef.current);
    const point = layout.points[node.id];
    if (!viewportPoint || !point) {
      return;
    }
    event.stopPropagation();
    selectNode(node);
    graphPointerRef.current = {
      kind: "node",
      pointerId: event.pointerId,
      nodeId: node.id,
      offset: {
        x: point.x - (viewportPoint.x - graphTransform.x) / graphTransform.k,
        y: point.y - (viewportPoint.y - graphTransform.y) / graphTransform.k,
      },
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function handleGraphPointerMove(event: PointerEvent<SVGSVGElement>) {
    const state = graphPointerRef.current;
    if (!state || state.pointerId !== event.pointerId) {
      return;
    }
    const viewportPoint = viewportPointFromPointer(event, graphSvgRef.current);
    if (!viewportPoint) {
      return;
    }
    if (state.kind === "pan") {
      setGraphTransform({
        ...state.startTransform,
        x: state.startTransform.x + viewportPoint.x - state.startViewport.x,
        y: state.startTransform.y + viewportPoint.y - state.startViewport.y,
      });
      return;
    }
    setPinnedPositions((current) => ({
      ...current,
      [state.nodeId]: {
        x: (viewportPoint.x - graphTransform.x) / graphTransform.k + state.offset.x,
        y: (viewportPoint.y - graphTransform.y) / graphTransform.k + state.offset.y,
      },
    }));
  }

  function handleGraphPointerUp(event: PointerEvent<SVGSVGElement>) {
    const state = graphPointerRef.current;
    if (state?.pointerId === event.pointerId) {
      graphPointerRef.current = null;
    }
  }

  return (
    <div className="mx-auto min-h-full max-w-7xl px-5 py-6">
      <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="mb-1 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-zinc-500 dark:text-neutral-400">
            <Brain aria-hidden="true" size={14} />
            Memory
          </p>
          <h1 className="text-2xl font-semibold tracking-normal text-zinc-950 dark:text-neutral-50">
            What Trace remembers
          </h1>
        </div>
        <div className="flex items-center gap-2">
          <ToggleSwitch
            checked={settings?.enabled ?? true}
            label={(settings?.enabled ?? true) ? "On" : "Off"}
            onChange={(enabled) => void setMemoryEnabled(enabled)}
          />
          <button className="btn" onClick={() => void loadMemoryState()} type="button">
            <RefreshCw aria-hidden="true" size={16} />
            Refresh
          </button>
        </div>
      </div>

      {error ? <div className="mb-4 notice notice-error">{error}</div> : null}
      {message ? <div className="mb-4 notice notice-success">{message}</div> : null}

      <section className="rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
        <div className="border-b border-zinc-100 p-4 dark:border-zinc-700 sm:p-5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 className="text-base font-semibold text-zinc-950 dark:text-neutral-50">
                Memory summary
              </h2>
              <p className="mt-1 text-sm text-zinc-500">
                {memories.length} saved {memories.length === 1 ? "memory" : "memories"}
                {pinnedCount > 0 ? `, ${pinnedCount} pinned` : ""}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <button
                className="btn"
                onClick={() => setMemoriesCollapsed((value) => !value)}
                type="button"
              >
                <ChevronDown
                  aria-hidden="true"
                  className={memoriesCollapsed ? "-rotate-90 transition" : "transition"}
                  size={16}
                />
                {memoriesCollapsed ? "Expand" : "Collapse"}
              </button>
              <button className="btn btn-primary" onClick={openNewMemoryEditor} type="button">
                <Plus aria-hidden="true" size={16} />
                Add
              </button>
            </div>
          </div>

          <label className="relative mt-4 block">
            <Search
              aria-hidden="true"
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-neutral-400"
              size={15}
            />
            <input
              className="field-control pl-9"
              onChange={(event) => setMemorySearch(event.currentTarget.value)}
              placeholder="Search memory"
              value={memorySearch}
            />
          </label>
        </div>

        {editorOpen ? (
          <form
            className="border-b border-zinc-100 bg-zinc-50 p-4 dark:border-zinc-700 dark:bg-zinc-950 sm:p-5"
            onSubmit={saveDraftMemory}
          >
            <div className="mb-3 flex items-center justify-between gap-3">
              <h3 className="text-sm font-semibold text-zinc-950 dark:text-neutral-50">
                {editingMemoryId ? "Edit memory" : "Add memory"}
              </h3>
              <button
                aria-label="Close memory editor"
                className="rounded-md p-1 text-neutral-400 transition hover:bg-white hover:text-zinc-900 dark:hover:bg-zinc-900 dark:hover:text-zinc-100"
                onClick={closeMemoryEditor}
                type="button"
              >
                <X aria-hidden="true" size={16} />
              </button>
            </div>
            <div className="grid gap-3 md:grid-cols-[150px_1fr]">
              <select
                className="field-control"
                onChange={(event) => setDraftKind(event.currentTarget.value as MemoryKind)}
                value={draftKind}
              >
                {(Object.keys(memoryKindLabels) as MemoryKind[]).map((kind) => (
                  <option key={kind} value={kind}>
                    {memoryKindLabels[kind]}
                  </option>
                ))}
              </select>
              <input
                className="field-control"
                onChange={(event) => setDraftTitle(event.currentTarget.value)}
                placeholder="Title"
                value={draftTitle}
              />
            </div>
            <textarea
              className="field-control mt-3 min-h-24 resize-y"
              onChange={(event) => setDraftBody(event.currentTarget.value)}
              placeholder="Memory"
              value={draftBody}
            />
            <div className="mt-3 grid gap-3 md:grid-cols-[1fr_160px_auto]">
              <input
                className="field-control"
                onChange={(event) => setDraftTags(event.currentTarget.value)}
                placeholder="Tags"
                value={draftTags}
              />
              <select
                className="field-control"
                onChange={(event) => setDraftSensitivity(event.currentTarget.value as MemorySensitivity)}
                value={draftSensitivity}
              >
                <option value="normal">Normal</option>
                <option value="pii">PII</option>
                <option value="sensitive">Sensitive</option>
              </select>
              <label className="choice-row h-10 items-center">
                <input
                  checked={draftPinned}
                  onChange={(event) => setDraftPinned(event.currentTarget.checked)}
                  type="checkbox"
                />
                <span>Pin</span>
              </label>
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <button className="btn" onClick={closeMemoryEditor} type="button">
                Cancel
              </button>
              <button
                className="btn btn-primary"
                disabled={!draftTitle.trim() || !draftBody.trim()}
                type="submit"
              >
                {editingMemoryId ? "Save changes" : "Save memory"}
              </button>
            </div>
          </form>
        ) : null}

        {memoriesCollapsed ? (
          <div className="px-5 py-4 text-sm text-zinc-500">
            Saved memories hidden.
          </div>
        ) : visibleMemories.length === 0 ? (
          <div className="px-5 py-12 text-center text-sm text-zinc-500">
            {memories.length === 0 ? "No memories saved yet." : "No memories match the search."}
          </div>
        ) : (
          <div className="divide-y divide-neutral-200 dark:divide-neutral-800">
            {visibleMemories.map((memory) => (
              <article className="px-4 py-4 sm:px-5" key={memory.id}>
                <div className="flex items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="mb-1 flex flex-wrap items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-neutral-400">
                      <span>{memoryKindLabels[memory.kind]}</span>
                      <span>{memory.source}</span>
                      <span>{memory.scope}</span>
                      <span>Updated {formatDateTime(memory.updated_at)}</span>
                    </div>
                    <h3 className="flex flex-wrap items-center gap-1.5 break-words text-sm font-semibold text-zinc-950 dark:text-neutral-50">
                      {memory.title}
                      {memory.pinned ? (
                        <span
                          aria-label="Pinned"
                          className="inline-flex h-5 w-5 items-center justify-center rounded-full bg-amber-50 text-amber-700 ring-1 ring-amber-100"
                          title="Pinned"
                        >
                          <Pin aria-hidden="true" size={11} />
                        </span>
                      ) : null}
                      {memory.sensitivity !== "normal" ? (
                        <span className="inline-flex items-center gap-1 rounded-full bg-rose-50 px-1.5 py-0.5 text-[10px] font-medium text-rose-700 ring-1 ring-rose-100">
                          <Shield aria-hidden="true" size={10} />
                          {memory.sensitivity}
                        </span>
                      ) : null}
                    </h3>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      aria-label={`Edit memory ${memory.title}`}
                      className="rounded-md p-1.5 text-neutral-400 transition hover:bg-zinc-50 hover:text-zinc-900 dark:hover:bg-neutral-800 dark:hover:text-zinc-100"
                      onClick={() => openEditMemoryEditor(memory)}
                      type="button"
                    >
                      <Pencil aria-hidden="true" size={15} />
                    </button>
                    {memory.source !== "system" ? (
                      <button
                        aria-label={`Delete memory ${memory.title}`}
                        className="rounded-md p-1.5 text-neutral-400 transition hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/30"
                        onClick={() => void removeMemory(memory.id)}
                        type="button"
                      >
                        <Trash2 aria-hidden="true" size={15} />
                      </button>
                    ) : null}
                  </div>
                </div>
                <p className="mt-2 line-clamp-3 text-sm leading-6 text-neutral-600 dark:text-zinc-300">
                  {memory.body}
                </p>
                {memory.tags.length > 0 ? (
                  <div className="mt-3 flex flex-wrap gap-1.5">
                    {memory.tags.map((tag) => (
                      <span
                        className="rounded-full bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-500 dark:bg-neutral-800 dark:text-zinc-300"
                        key={tag}
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                ) : null}
              </article>
            ))}
          </div>
        )}
      </section>

      <div className="mt-4 space-y-3">
        <BrainTemplatePanel
          activeTemplate={activeBrainTemplate}
          busy={brainTemplateBusy}
          feedback={brainTemplateFeedback}
          onFeedback={(row, eventType) => void handleBrainRowFeedback(row, eventType)}
          onPreview={handleBrainRowPreview}
          onRefresh={() => void loadBrainTemplate()}
          onSelectTemplate={selectBrainTemplate}
          result={brainTemplate}
        />
        {brainPreview ? (
          <BrainPreviewModal
            onClose={() => setBrainPreview(null)}
            onOpenTarget={openBrainPreviewTarget}
            preview={brainPreview}
          />
        ) : null}

        <details className="group rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
          <summary className="flex cursor-pointer list-none items-center justify-between gap-3 p-4 text-sm font-semibold text-zinc-950 dark:text-neutral-50">
            <span className="flex items-center gap-2">
              <SlidersHorizontal aria-hidden="true" size={16} />
              Memory tools
            </span>
            <ChevronDown
              aria-hidden="true"
              className="text-neutral-400 transition group-open:rotate-180"
              size={16}
            />
          </summary>
          <div className="border-t border-zinc-100 p-4 dark:border-zinc-700 sm:p-5">
            <div className="grid gap-4 lg:grid-cols-2">
              <div className="space-y-3">
                <div>
                  <p className="field-label">Generate</p>
                  <div className="mt-2 flex flex-wrap gap-2">
                    <button
                      className="btn"
                      onClick={() => void runConsolidation()}
                      type="button"
                    >
                      <Sparkles aria-hidden="true" size={16} />
                      Consolidate
                    </button>
                    <button
                      className="btn"
                      onClick={() => void runEmbedAll()}
                      title="Generate Gemini embeddings for active memory"
                      type="button"
                    >
                      <Zap aria-hidden="true" size={15} />
                      Embed
                    </button>
                    <button
                      className="btn"
                      disabled={brainBusy}
                      onClick={() => void runBrainRebuild()}
                      title="Rebuild the local Kuzu brain graph from SQLite"
                      type="button"
                    >
                      <Network aria-hidden="true" size={15} />
                      {brainBusy ? "Rebuilding" : "Rebuild brain"}
                    </button>
                  </div>
                  {brainStatus ? (
                    <p className="mt-2 text-xs text-neutral-400">
                      Brain graph: {brainStatus.node_count} nodes, {brainStatus.edge_count} relations
                      {brainStatus.generated_at ? ` · ${formatDateTime(brainStatus.generated_at)}` : ""}
                    </p>
                  ) : null}
                </div>
                <div>
                  <p className="field-label">Extract from conversation</p>
                  <div className="mt-2 flex gap-2">
                    <select
                      className="field-control min-w-0 flex-1"
                      onChange={(event) => setExtractConvId(event.currentTarget.value)}
                      value={extractConvId}
                    >
                      <option value="">Pick a conversation</option>
                      {conversations.map((conv) => (
                        <option key={conv.id} value={conv.id}>
                          {conv.title ?? "Untitled"} · {conv.occurred_at ?? conv.ingested_at?.slice(0, 10) ?? "-"}
                        </option>
                      ))}
                    </select>
                    <button
                      className="btn"
                      disabled={!extractConvId || extractBusy}
                      onClick={() => void runExtraction()}
                      type="button"
                    >
                      {extractBusy ? "Extracting" : "Extract"}
                    </button>
                  </div>
                </div>
              </div>

              <div>
                <p className="field-label">Recall test</p>
                <div className="mt-2 flex gap-2">
                  <input
                    className="field-control"
                    onChange={(event) => setMemoryQuery(event.currentTarget.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        void runRetrieval();
                      }
                    }}
                    placeholder="Ask memory to recall..."
                    value={memoryQuery}
                  />
                  <button className="btn" onClick={() => void runRetrieval()} type="button">
                    <Search aria-hidden="true" size={16} />
                    Retrieve
                  </button>
                </div>
                {retrieval ? (
                  <RetrievalDiagnostics
                    busy={feedbackBusy}
                    feedbackSent={feedbackSent}
                    onFeedback={(value) => void sendFeedback(value)}
                    retrieval={retrieval}
                  />
                ) : (
                  <p className="mt-3 text-xs text-neutral-400">No recall test run.</p>
                )}
              </div>
            </div>
          </div>
        </details>

        <details
          className="group rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900"
          onToggle={(event) => setEventsOpen(event.currentTarget.open)}
        >
          <summary className="flex cursor-pointer list-none items-center justify-between gap-3 p-4 text-sm font-semibold text-zinc-950 dark:text-neutral-50">
            <span className="flex items-center gap-2">
              <History aria-hidden="true" size={16} />
              Activity
            </span>
            <span className="flex items-center gap-2 text-xs font-medium text-zinc-500">
              {events.length} events
              <ChevronDown
                aria-hidden="true"
                className="text-neutral-400 transition group-open:rotate-180"
                size={16}
              />
            </span>
          </summary>
          {eventsOpen ? (
            <div className="max-h-72 overflow-y-auto border-t border-zinc-100 dark:border-zinc-700">
              {events.length === 0 ? (
                <p className="px-4 py-5 text-sm text-zinc-500">No memory activity yet.</p>
              ) : (
                <ul className="divide-y divide-neutral-200 dark:divide-neutral-800">
                  {events.map((event) => (
                    <li className="px-4 py-3 text-xs text-neutral-600 dark:text-zinc-300" key={event.id}>
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-semibold uppercase tracking-wide text-zinc-700 dark:text-neutral-200">
                          {event.action}
                        </span>
                        <span className="text-neutral-400">{formatDateTime(event.created_at)}</span>
                      </div>
                      {event.memory_id ? (
                        <div className="mt-1 truncate font-mono text-[11px] text-neutral-400">
                          {event.memory_id}
                        </div>
                      ) : null}
                      {event.detail_json && event.detail_json !== "{}" ? (
                        <pre className="mt-1 whitespace-pre-wrap font-mono text-[11px] text-zinc-500">
                          {prettifyEventDetail(event.detail_json)}
                        </pre>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ) : null}
        </details>

        <details className="group rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
          <summary className="flex cursor-pointer list-none items-center justify-between gap-3 p-4 text-sm font-semibold text-zinc-950 dark:text-neutral-50">
            <span className="flex items-center gap-2">
              <Network aria-hidden="true" size={16} />
              Brain graph
            </span>
            <span className="flex items-center gap-2 text-xs font-medium text-zinc-500">
              {visibleNodes.length} nodes
              <ChevronDown
                aria-hidden="true"
                className="text-neutral-400 transition group-open:rotate-180"
                size={16}
              />
            </span>
          </summary>
          <div className="border-t border-zinc-100 p-4 dark:border-zinc-700 sm:p-5">
            <div className="mb-4 flex flex-wrap items-center gap-3">
              <label className="relative block min-w-72 flex-1">
                <Search
                  aria-hidden="true"
                  className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-neutral-400"
                  size={15}
                />
                <input
                  className="field-control pl-9"
                  onChange={(event) => setQuery(event.currentTarget.value)}
                  placeholder="Search graph"
                  value={query}
                />
              </label>
              <button className="btn" disabled={isLoading} onClick={() => void loadGraph()} type="button">
                <RefreshCw aria-hidden="true" size={16} />
                Refresh
              </button>
              <button className="btn" disabled={brainBusy} onClick={() => void runBrainRebuild()} type="button">
                <Network aria-hidden="true" size={16} />
                Rebuild
              </button>
              <select
                aria-label="Graph layout"
                className="field-control h-8 w-auto py-0 text-xs"
                onChange={(event) => setLayoutMode(event.currentTarget.value as GraphLayoutMode)}
                value={layoutMode}
              >
                <option value="force">Force layout</option>
                <option value="hierarchy">Hierarchical</option>
                <option value="radial">Radial</option>
              </select>
              <select
                aria-label="Graph density"
                className="field-control h-8 w-auto py-0 text-xs"
                onChange={(event) => setGraphDensity(event.currentTarget.value as GraphDensity)}
                value={graphDensity}
              >
                <option value="compact">Compact</option>
                <option value="balanced">Balanced</option>
                <option value="spread">Spread</option>
              </select>
              <select
                aria-label="Graph labels"
                className="field-control h-8 w-auto py-0 text-xs"
                onChange={(event) => setLabelMode(event.currentTarget.value as GraphLabelMode)}
                value={labelMode}
              >
                <option value="smart">Smart labels</option>
                <option value="selected">Selected labels</option>
                <option value="all">All labels</option>
              </select>
              <select
                aria-label="Scene limit"
                className="field-control h-8 w-auto py-0 text-xs"
                onChange={(event) => setSceneLimit(Number(event.currentTarget.value))}
                value={sceneLimit}
              >
                <option value={160}>160 nodes</option>
                <option value={360}>360 nodes</option>
                <option value={720}>720 nodes</option>
                <option value={2000}>All visible</option>
              </select>
              <label className="choice-row">
                <input
                  checked={includeKilledDeliverables}
                  className="mt-1"
                  onChange={(event) => setIncludeKilledDeliverables(event.currentTarget.checked)}
                  type="checkbox"
                />
                <span>Killed</span>
              </label>
              <label className="choice-row">
                <input
                  checked={includeDismissedCaptures}
                  className="mt-1"
                  onChange={(event) => setIncludeDismissedCaptures(event.currentTarget.checked)}
                  type="checkbox"
                />
                <span>Dismissed</span>
              </label>
              <label className="choice-row">
                <input
                  checked={focusSelected}
                  className="mt-1"
                  disabled={!selectedId}
                  onChange={(event) => setFocusSelected(event.currentTarget.checked)}
                  type="checkbox"
                />
                <span>Focus</span>
              </label>
            </div>

            <div className="mb-3 flex flex-wrap gap-2">
              {allKinds.map((kind) => (
                <button
                  className={[
                    "h-8 rounded-md border px-3 text-xs font-semibold transition",
                    activeKinds.has(kind)
                      ? "border-neutral-900 bg-zinc-900 text-white dark:border-neutral-100 dark:bg-zinc-100 dark:text-zinc-950"
                      : "border-zinc-100 bg-white text-zinc-500 hover:text-zinc-900 dark:border-zinc-700 dark:bg-zinc-900",
                  ].join(" ")}
                  key={kind}
                  onClick={() => toggleKind(kind)}
                  type="button"
                >
                  {kindLabel(kind)}
                </button>
              ))}
            </div>
            <div className="mb-4 flex flex-wrap gap-2">
              {allRelations.map((kind) => (
                <button
                  className={[
                    "h-7 rounded-md border px-2.5 text-[11px] font-semibold transition",
                    activeRelations.has(kind)
                      ? "border-neutral-300 bg-zinc-100 text-zinc-700 dark:border-neutral-700 dark:bg-neutral-800 dark:text-zinc-100"
                      : "border-zinc-100 bg-white text-neutral-400 hover:text-neutral-800 dark:border-zinc-700 dark:bg-zinc-900",
                  ].join(" ")}
                  key={kind}
                  onClick={() => toggleRelation(kind)}
                  type="button"
                >
                  {relationLabel(kind)}
                </button>
              ))}
            </div>

            <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
              <section className="relative min-h-[620px] overflow-hidden rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
                {isLoading ? (
                  <div className="flex h-full min-h-[620px] items-center justify-center text-sm text-zinc-500">
                    Loading graph...
                  </div>
                ) : visibleNodes.length === 0 ? (
                  <div className="flex h-full min-h-[620px] items-center justify-center text-sm text-zinc-500">
                    No graph nodes match the current filters.
                  </div>
                ) : (
                  <>
                    <svg
                      aria-label="Brain graph"
                      className="h-full min-h-[620px] w-full cursor-grab bg-white active:cursor-grabbing"
                      onPointerCancel={handleGraphPointerUp}
                      onPointerMove={handleGraphPointerMove}
                      onPointerUp={handleGraphPointerUp}
                      onWheel={handleGraphWheel}
                      ref={graphSvgRef}
                      role="img"
                      viewBox={`0 0 ${graphViewportWidth} ${graphViewportHeight}`}
                    >
                      <defs>
                        <marker
                          id="brain-edge-arrow"
                          markerHeight="8"
                          markerWidth="8"
                          orient="auto"
                          refX="7"
                          refY="3.5"
                        >
                          <path d="M0,0 L7,3.5 L0,7 Z" fill="#a1a1aa" />
                        </marker>
                        <pattern height="36" id="brain-grid" patternUnits="userSpaceOnUse" width="36">
                          <path d="M 36 0 L 0 0 0 36" fill="none" stroke="#f1f5f9" strokeWidth="1" />
                        </pattern>
                      </defs>
                      <rect
                        fill="url(#brain-grid)"
                        height={graphViewportHeight}
                        onPointerDown={handleGraphBackgroundPointerDown}
                        width={graphViewportWidth}
                      />
                      <g transform={`translate(${graphTransform.x} ${graphTransform.y}) scale(${graphTransform.k})`}>
                        {visibleEdges.map((edge) => (
                          <GraphEdge
                            edge={edge}
                            key={edge.id}
                            layout={layout.points}
                            onSelect={() => selectEdge(edge)}
                            selectedEdgeId={selectedEdge?.id ?? null}
                            selectedNodeId={selectedNode?.id ?? null}
                          />
                        ))}
                        {visibleNodes.map((node) => {
                          const isEdgeEndpoint = Boolean(
                            selectedEdge && (selectedEdge.source === node.id || selectedEdge.target === node.id),
                          );
                          return (
                            <GraphNode
                              isDimmed={Boolean(
                                (selectedNode && !selectedNeighborhood.has(node.id)) ||
                                  (selectedEdge && !isEdgeEndpoint),
                              )}
                              isPinned={Boolean(pinnedPositions[node.id])}
                              isSelected={node.id === selectedNode?.id}
                              key={node.id}
                              labelMode={labelMode}
                              node={node}
                              onDoubleClick={() => {
                                setSelectedId(node.id);
                                setFocusSelected(true);
                              }}
                              onPointerDown={(event) => handleNodePointerDown(node, event)}
                              onSelect={() => selectNode(node)}
                              point={layout.points[node.id]}
                            />
                          );
                        })}
                      </g>
                      <GraphMiniMap
                        bounds={layout.bounds}
                        nodes={visibleNodes}
                        points={layout.points}
                        transform={graphTransform}
                      />
                    </svg>
                    <div className="absolute bottom-3 right-3 flex items-center gap-1 rounded-xl border border-zinc-100 bg-white/95 p-1 shadow-sm backdrop-blur dark:border-zinc-700 dark:bg-zinc-950/90">
                      <button className="btn h-8 w-8 px-0" onClick={() => zoomScene(1.18)} title="Zoom in" type="button">
                        <Plus aria-hidden="true" size={15} />
                      </button>
                      <button className="btn h-8 w-8 px-0" onClick={() => zoomScene(0.84)} title="Zoom out" type="button">
                        <Minus aria-hidden="true" size={15} />
                      </button>
                      <button className="btn h-8 w-8 px-0" onClick={fitScene} title="Fit graph" type="button">
                        <Maximize2 aria-hidden="true" size={15} />
                      </button>
                      <button className="btn h-8 px-3" onClick={resetPinnedLayout} title="Unpin dragged nodes" type="button">
                        Reset
                      </button>
                    </div>
                    <div className="absolute left-3 top-3 flex flex-wrap gap-2 rounded-xl border border-zinc-100 bg-white/95 px-3 py-2 text-xs text-neutral-600 shadow-sm backdrop-blur dark:border-zinc-700 dark:bg-zinc-950/90">
                      <span>{visibleNodes.length} nodes</span>
                      <span>{visibleEdges.length} relationships</span>
                      <span>{Math.round(graphTransform.k * 100)}%</span>
                    </div>
                  </>
                )}
              </section>

              <aside className="h-fit rounded-xl border border-zinc-100 bg-white p-5 dark:border-zinc-700 dark:bg-zinc-900">
                <div className="mb-4 flex items-center gap-2">
                  <Eye aria-hidden="true" size={16} />
                  <h2 className="text-sm font-semibold">Inspector</h2>
                </div>
                {selectedEdge ? (
                  <div className="space-y-4">
                    <div>
                      <p className="field-label">Relationship</p>
                      <h3 className="mt-1 break-words text-base font-semibold">{selectedEdge.label}</h3>
                      <p className="mt-2 text-xs leading-5 text-zinc-500">
                        {nodeLabelById(graph, selectedEdge.source)} {"->"}{" "}
                        {nodeLabelById(graph, selectedEdge.target)}
                      </p>
                    </div>
                    <PropertyGrid value={selectedEdge.properties} />
                    <button className="btn w-full" onClick={() => void copySelectedProperties()} type="button">
                      <Clipboard aria-hidden="true" size={16} />
                      Copy properties
                    </button>
                  </div>
                ) : selectedNode ? (
                  <div className="space-y-4">
                    <div>
                      <p className="field-label">{kindLabel(selectedNode.kind)}</p>
                      <h3 className="mt-1 break-words text-base font-semibold">{selectedNode.label}</h3>
                      {selectedNode.subtitle ? (
                        <p className="mt-2 line-clamp-6 text-sm leading-6 text-neutral-600 dark:text-zinc-300">
                          {selectedNode.subtitle}
                        </p>
                      ) : null}
                    </div>
                    <div>
                      <p className="field-label">AI context</p>
                      <p className="mt-1 whitespace-pre-wrap rounded-md bg-zinc-50 p-3 text-xs leading-5 text-zinc-600">
                        {selectedNode.context}
                      </p>
                    </div>
                    <dl className="grid gap-2 text-xs text-zinc-500">
                      {selectedNode.status ? (
                        <div>
                          <dt className="field-label">Status</dt>
                          <dd className="mt-1">{selectedNode.status}</dd>
                        </div>
                      ) : null}
                      {selectedNode.updated_at ? (
                        <div>
                          <dt className="field-label">Updated</dt>
                          <dd className="mt-1">{formatDateTime(selectedNode.updated_at)}</dd>
                        </div>
                      ) : null}
                    </dl>
                    {selectedNode.url ? (
                      <button className="btn btn-primary w-full" onClick={() => openNode(selectedNode)} type="button">
                        Open
                      </button>
                    ) : null}
                    <button className="btn w-full" onClick={() => void copyAiContext()} type="button">
                      <Clipboard aria-hidden="true" size={16} />
                      Copy AI context
                    </button>
                    <button className="btn w-full" onClick={() => void copySelectedProperties()} type="button">
                      <Clipboard aria-hidden="true" size={16} />
                      Copy properties
                    </button>
                    <PropertyGrid value={selectedNode.properties} />
                    {selectedEdges.length > 0 ? (
                      <div>
                        <p className="field-label mb-2">Related</p>
                        <div className="space-y-1.5">
                          {selectedEdges.slice(0, 8).map((edge) => (
                            <p
                              className="rounded-md bg-zinc-50 px-2 py-1.5 text-xs text-zinc-500"
                              key={edge.id}
                            >
                              {edge.label} · {relatedLabel(edge, selectedNode.id, graph)}
                            </p>
                          ))}
                        </div>
                      </div>
                    ) : null}
                  </div>
                ) : (
                  <p className="text-sm text-zinc-500">
                    Select a node or relationship in the graph.
                  </p>
                )}
                <div className="mt-6 border-t border-zinc-100 pt-4 text-xs text-zinc-500 dark:border-zinc-700">
                  {visibleNodes.length} nodes, {visibleEdges.length} edges
                  {graph ? <span> · Generated {formatDateTime(graph.generated_at)}</span> : null}
                </div>
              </aside>
            </div>
          </div>
        </details>
      </div>
    </div>
  );
}

function BrainTemplatePanel({
  activeTemplate,
  busy,
  feedback,
  onFeedback,
  onPreview,
  onRefresh,
  onSelectTemplate,
  result,
}: {
  activeTemplate: BrainTemplateKind;
  busy: boolean;
  feedback: Record<string, string>;
  onFeedback: (row: Record<string, unknown>, eventType: "useful" | "wrong" | "ignored") => void;
  onPreview: (row: Record<string, unknown>) => void;
  onRefresh: () => void;
  onSelectTemplate: (template: BrainTemplateKind) => void;
  result: BrainTemplateResult | null;
}) {
  const rows = result?.rows.slice(0, 12) ?? [];
  return (
    <details className="group rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-3 p-4 text-sm font-semibold text-zinc-950 dark:text-neutral-50">
        <span className="flex items-center gap-2">
          <Brain aria-hidden="true" size={16} />
          Brain brief
        </span>
        <span className="flex items-center gap-2 text-xs font-medium text-zinc-500">
          {busy ? "Loading" : `${rows.length} items`}
          <ChevronDown
            aria-hidden="true"
            className="text-neutral-400 transition group-open:rotate-180"
            size={16}
          />
        </span>
      </summary>
      <div className="border-t border-zinc-100 p-4 dark:border-zinc-700 sm:p-5">
        <div className="mb-4 flex flex-wrap items-center gap-2">
          {brainTemplateOptions.map((option) => (
            <button
              className={[
                "h-8 rounded-md border px-3 text-xs font-semibold transition",
                activeTemplate === option.value
                  ? "border-neutral-900 bg-zinc-900 text-white dark:border-neutral-100 dark:bg-zinc-100 dark:text-zinc-950"
                  : "border-zinc-100 bg-white text-zinc-500 hover:text-zinc-900 dark:border-zinc-700 dark:bg-zinc-900",
              ].join(" ")}
              key={option.value}
              onClick={() => onSelectTemplate(option.value)}
              type="button"
            >
              {option.label}
            </button>
          ))}
          <button className="btn ml-auto" disabled={busy} onClick={onRefresh} type="button">
            <RefreshCw aria-hidden="true" size={15} />
            Refresh
          </button>
        </div>

        {result ? (
          <div className="mb-3 rounded-md bg-zinc-50 px-3 py-2 text-xs leading-5 text-zinc-500 dark:bg-zinc-950/40">
            {result.summary}
          </div>
        ) : null}

        {busy ? (
          <p className="py-6 text-sm text-zinc-500">Loading brain brief...</p>
        ) : rows.length === 0 ? (
          <p className="py-6 text-sm text-zinc-500">No brain items for this template yet.</p>
        ) : (
          <div className="grid gap-2 lg:grid-cols-2">
            {rows.map((row, index) => {
              const id = rowString(row, "id") || `row-${index}`;
              const kind = rowString(row, "kind");
              const status = rowString(row, "status");
              const score = rowNumber(row, "brain_rl_score");
              const sent = feedback[id];
              return (
                <article
                  className="rounded-xl border border-zinc-100 bg-white p-3 text-sm dark:border-zinc-700 dark:bg-zinc-900"
                  key={`${id}-${index}`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-neutral-400">
                        <span>{kind || "item"}</span>
                        {status ? <span>{status}</span> : null}
                        {score !== null ? <span>score {score.toFixed(2)}</span> : null}
                      </div>
                      <h3 className="mt-1 line-clamp-2 text-sm font-semibold text-zinc-950 dark:text-neutral-50">
                        {rowString(row, "title") || "Untitled"}
                      </h3>
                      {rowString(row, "summary") ? (
                        <p className="mt-1 line-clamp-3 text-xs leading-5 text-zinc-500">
                          {rowString(row, "summary")}
                        </p>
                      ) : null}
                    </div>
                    <button
                      className="btn h-8 w-8 shrink-0 px-0"
                      onClick={() => onPreview(row)}
                      title="Preview item"
                      type="button"
                    >
                      <Eye aria-hidden="true" size={14} />
                    </button>
                  </div>
                  <div className="mt-3 flex items-center gap-1.5">
                    <button
                      className="btn h-7 px-2 text-[11px]"
                      disabled={Boolean(sent)}
                      onClick={() => onFeedback(row, "useful")}
                      title="Mark useful"
                      type="button"
                    >
                      <ThumbsUp aria-hidden="true" size={12} />
                      Useful
                    </button>
                    <button
                      className="btn h-7 px-2 text-[11px]"
                      disabled={Boolean(sent)}
                      onClick={() => onFeedback(row, "wrong")}
                      title="Mark not useful"
                      type="button"
                    >
                      <ThumbsDown aria-hidden="true" size={12} />
                      Not useful
                    </button>
                    <button
                      className="btn h-7 px-2 text-[11px]"
                      disabled={Boolean(sent)}
                      onClick={() => onFeedback(row, "ignored")}
                      title="Tell the model to down-rank similar items"
                      type="button"
                    >
                      Ignore
                    </button>
                    {sent ? (
                      <span className="ml-auto text-[11px] font-medium text-neutral-400">
                        Recorded {sent}
                      </span>
                    ) : null}
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>
    </details>
  );
}

function BrainPreviewModal({
  onClose,
  onOpenTarget,
  preview,
}: {
  onClose: () => void;
  onOpenTarget: () => void;
  preview: BrainPreviewState;
}) {
  const title = rowString(preview.row, "title") || preview.node?.label || "Untitled";
  const summary = rowString(preview.row, "summary") || preview.node?.subtitle || "";
  const kind = rowString(preview.row, "kind") || preview.node?.kind || "item";
  const status = rowString(preview.row, "status") || preview.node?.status || "";
  const reason = rowString(preview.row, "reason");
  const score = rowNumber(preview.row, "brain_rl_score");
  const hasTarget = Boolean(preview.target);

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/20 px-3 py-4 backdrop-blur-[2px] sm:items-center"
      onMouseDown={onClose}
    >
      <div
        aria-modal="true"
        className="flex max-h-[88vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-2xl dark:border-zinc-700 dark:bg-zinc-950"
        onMouseDown={(event) => event.stopPropagation()}
        role="dialog"
      >
        <header className="flex items-start justify-between gap-4 border-b border-zinc-100 p-4 dark:border-zinc-700">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-neutral-400">
              <span>{formatPreviewKind(kind)}</span>
              {status ? <span>{status}</span> : null}
              {score !== null ? <span>score {score.toFixed(2)}</span> : null}
            </div>
            <h2 className="mt-1 text-base font-semibold text-zinc-950 dark:text-neutral-50">
              {title}
            </h2>
            {summary ? (
              <p className="mt-1 text-sm leading-6 text-zinc-500 dark:text-neutral-400">
                {summary}
              </p>
            ) : null}
          </div>
          <button
            aria-label="Close preview"
            className="btn h-8 w-8 shrink-0 px-0"
            onClick={onClose}
            type="button"
          >
            <X aria-hidden="true" size={15} />
          </button>
        </header>

        <div className="space-y-3 overflow-y-auto p-4">
          {reason ? (
            <section className="rounded-xl border border-zinc-100 p-3 dark:border-zinc-700">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-neutral-400">
                Why it appeared
              </h3>
              <p className="mt-1 text-sm text-zinc-700 dark:text-zinc-300">{reason}</p>
            </section>
          ) : null}

          <section className="rounded-xl border border-zinc-100 p-3 dark:border-zinc-700">
            <h3 className="text-xs font-semibold uppercase tracking-wide text-neutral-400">
              Actual item
            </h3>
            {preview.target ? (
              <div className="mt-2">
                <div className="flex flex-wrap items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-neutral-400">
                  <span>{formatPreviewKind(preview.target.kind)}</span>
                  {preview.target.status ? <span>{preview.target.status}</span> : null}
                </div>
                <p className="mt-1 text-sm font-semibold text-zinc-950 dark:text-neutral-50">
                  {preview.target.label}
                </p>
                {preview.target.subtitle ? (
                  <p className="mt-1 text-xs leading-5 text-zinc-500">
                    {preview.target.subtitle}
                  </p>
                ) : null}
              </div>
            ) : (
              <p className="mt-2 text-sm text-zinc-500">
                No linked source item was found in the graph for this brain card.
              </p>
            )}
          </section>

          {preview.related.length > 0 ? (
            <section className="rounded-xl border border-zinc-100 p-3 dark:border-zinc-700">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-neutral-400">
                Nearby context
              </h3>
              <div className="mt-2 grid gap-2 sm:grid-cols-2">
                {preview.related.map((node) => (
                  <div
                    className="rounded-md bg-zinc-50 px-3 py-2 text-sm dark:bg-zinc-900"
                    key={node.id}
                  >
                    <div className="flex flex-wrap items-center gap-2 text-[10px] font-semibold uppercase tracking-wide text-neutral-400">
                      <span>{formatPreviewKind(node.kind)}</span>
                      {node.status ? <span>{node.status}</span> : null}
                    </div>
                    <p className="mt-1 line-clamp-2 font-medium text-neutral-800 dark:text-zinc-100">
                      {node.label}
                    </p>
                    {node.subtitle ? (
                      <p className="mt-1 line-clamp-2 text-xs leading-5 text-zinc-500">
                        {node.subtitle}
                      </p>
                    ) : null}
                  </div>
                ))}
              </div>
            </section>
          ) : null}
        </div>

        <footer className="flex items-center justify-end gap-2 border-t border-zinc-100 p-4 dark:border-zinc-700">
          <button className="btn" onClick={onClose} type="button">
            Close
          </button>
          <button
            className="btn btn-primary"
            disabled={!hasTarget}
            onClick={onOpenTarget}
            type="button"
          >
            <Maximize2 aria-hidden="true" size={14} />
            {preview.target?.url ? "Open actual item" : "Show in graph"}
          </button>
        </footer>
      </div>
    </div>
  );
}

interface Point {
  x: number;
  y: number;
}

interface GraphBounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

interface GraphLayout {
  points: Record<string, Point>;
  bounds: GraphBounds;
}

interface GraphTransform {
  x: number;
  y: number;
  k: number;
}

type GraphPointerState =
  | {
      kind: "pan";
      pointerId: number;
      startViewport: Point;
      startTransform: GraphTransform;
    }
  | {
      kind: "node";
      pointerId: number;
      nodeId: string;
      offset: Point;
    };

interface LayoutNode extends SimulationNodeDatum {
  id: string;
  node: WorkGraphNode;
}

interface LayoutLink extends SimulationLinkDatum<LayoutNode> {
  kind: string;
}

function layoutGraph(
  nodes: WorkGraphNode[],
  edges: WorkGraphEdge[],
  options: {
    density: GraphDensity;
    mode: GraphLayoutMode;
    pinnedPositions: Record<string, Point>;
    selectedId: string | null;
  },
): GraphLayout {
  if (nodes.length === 0) {
    return {
      points: {},
      bounds: { minX: -100, minY: -100, maxX: 100, maxY: 100 },
    };
  }

  const density = densityConfig(options.density);
  const kinds = Array.from(new Set(nodes.map((node) => node.kind))).sort();
  const kindIndex = new Map(kinds.map((kind, index) => [kind, index]));
  const degree = nodeDegree(edges);

  if (options.mode === "hierarchy") {
    const points = layoutHierarchy(nodes, edges, degree, density);
    return withPinnedAndBounds(points, options.pinnedPositions);
  }

  if (options.mode === "radial") {
    const points = layoutRadial(nodes, edges, degree, kindIndex, density);
    return withPinnedAndBounds(points, options.pinnedPositions);
  }

  const simulationNodes: LayoutNode[] = nodes.map((node, index) => {
    const group = kindIndex.get(node.kind) ?? 0;
    const angle = seededAngle(node.id, index);
    const ring = 120 + (index % Math.max(6, Math.ceil(Math.sqrt(nodes.length)))) * density.seedGap;
    return {
      id: node.id,
      node,
      x: Math.cos(angle) * (ring + group * 18),
      y: Math.sin(angle) * (ring + group * 14),
    };
  });
  const simulationLinks: LayoutLink[] = edges.map((edge) => ({
    source: edge.source,
    target: edge.target,
    kind: edge.kind,
  }));

  const simulation = forceSimulation<LayoutNode>(simulationNodes)
    .force(
      "link",
      forceLink<LayoutNode, LayoutLink>(simulationLinks)
        .id((node) => node.id)
        .distance((link) => (link.kind === "CONTAINS" ? density.link * 0.78 : density.link))
        .strength(0.34),
    )
    .force(
      "charge",
      forceManyBody<LayoutNode>().strength((node) => -density.charge - (degree.get(node.id) ?? 0) * 4),
    )
    .force("collide", forceCollide<LayoutNode>().radius((node) => nodeRadius(node.node) + density.padding))
    .force(
      "x",
      forceX<LayoutNode>((node) => {
        const index = kindIndex.get(node.node.kind) ?? 0;
        if (kinds.length <= 1) return 0;
        return (index - (kinds.length - 1) / 2) * density.kindGap;
      }).strength(0.025),
    )
    .force(
      "y",
      forceY<LayoutNode>((node) => {
        const index = kindIndex.get(node.node.kind) ?? 0;
        return ((index % 4) - 1.5) * density.kindGap * 0.42;
      }).strength(0.018),
    )
    .stop();

  for (let index = 0; index < density.ticks; index += 1) {
    simulation.tick();
  }

  const points = simulationNodes.reduce<Record<string, Point>>((accumulator, node) => {
    accumulator[node.id] = { x: node.x ?? 0, y: node.y ?? 0 };
    return accumulator;
  }, {});
  centerPoints(points);
  return withPinnedAndBounds(points, options.pinnedPositions);
}

function densityConfig(density: GraphDensity) {
  switch (density) {
    case "compact":
      return {
        charge: 170,
        columnGap: 220,
        kindGap: 180,
        link: 92,
        padding: 10,
        rowGap: 54,
        seedGap: 12,
        ticks: 170,
      };
    case "spread":
      return {
        charge: 360,
        columnGap: 340,
        kindGap: 320,
        link: 156,
        padding: 24,
        rowGap: 88,
        seedGap: 22,
        ticks: 260,
      };
    default:
      return {
        charge: 260,
        columnGap: 280,
        kindGap: 250,
        link: 122,
        padding: 16,
        rowGap: 68,
        seedGap: 16,
        ticks: 220,
      };
  }
}

function nodeDegree(edges: WorkGraphEdge[]) {
  const degree = new Map<string, number>();
  for (const edge of edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }
  return degree;
}

function layoutHierarchy(
  nodes: WorkGraphNode[],
  edges: WorkGraphEdge[],
  degree: Map<string, number>,
  density: ReturnType<typeof densityConfig>,
) {
  const nodeIds = new Set(nodes.map((node) => node.id));
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, string[]>();

  for (const edge of edges) {
    if (!nodeIds.has(edge.source) || !nodeIds.has(edge.target)) {
      continue;
    }
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + 1);
    const links = outgoing.get(edge.source) ?? [];
    links.push(edge.target);
    outgoing.set(edge.source, links);
  }

  const roots = nodes
    .filter((node) => (incoming.get(node.id) ?? 0) === 0)
    .sort((first, second) => (degree.get(second.id) ?? 0) - (degree.get(first.id) ?? 0));
  const queue = roots.length > 0 ? roots.map((node) => node.id) : [nodes[0].id];
  const depth = new Map<string, number>(queue.map((id) => [id, 0]));

  for (let index = 0; index < queue.length; index += 1) {
    const id = queue[index];
    const nextDepth = (depth.get(id) ?? 0) + 1;
    for (const child of outgoing.get(id) ?? []) {
      if (!depth.has(child) || nextDepth < (depth.get(child) ?? nextDepth)) {
        depth.set(child, nextDepth);
        queue.push(child);
      }
    }
  }

  const fallbackDepthStart = Math.max(1, ...Array.from(depth.values(), (value) => value + 1));
  nodes
    .filter((node) => !depth.has(node.id))
    .sort((first, second) => (degree.get(second.id) ?? 0) - (degree.get(first.id) ?? 0))
    .forEach((node, index) => depth.set(node.id, fallbackDepthStart + (index % 3)));

  const columns = new Map<number, WorkGraphNode[]>();
  for (const node of nodes) {
    const level = depth.get(node.id) ?? 0;
    const column = columns.get(level) ?? [];
    column.push(node);
    columns.set(level, column);
  }

  const points: Record<string, Point> = {};
  const levels = Array.from(columns.keys()).sort((first, second) => first - second);
  for (const level of levels) {
    const column = (columns.get(level) ?? []).sort((first, second) => {
      const degreeDelta = (degree.get(second.id) ?? 0) - (degree.get(first.id) ?? 0);
      return degreeDelta || first.label.localeCompare(second.label);
    });
    const height = Math.max(0, (column.length - 1) * density.rowGap);
    column.forEach((node, index) => {
      points[node.id] = {
        x: (level - (levels.length - 1) / 2) * density.columnGap,
        y: index * density.rowGap - height / 2,
      };
    });
  }

  return points;
}

function layoutRadial(
  nodes: WorkGraphNode[],
  edges: WorkGraphEdge[],
  degree: Map<string, number>,
  kindIndex: Map<string, number>,
  density: ReturnType<typeof densityConfig>,
) {
  const sorted = [...nodes].sort((first, second) => {
    const degreeDelta = (degree.get(second.id) ?? 0) - (degree.get(first.id) ?? 0);
    return degreeDelta || second.weight - first.weight || first.label.localeCompare(second.label);
  });
  const centerId = sorted[0]?.id;
  const simulationNodes: LayoutNode[] = nodes.map((node, index) => {
    const group = kindIndex.get(node.kind) ?? 0;
    const rank = sorted.findIndex((candidate) => candidate.id === node.id);
    const ringIndex = Math.max(0, Math.floor(Math.sqrt(Math.max(0, rank))));
    const angle = seededAngle(node.id, index);
    const radius = node.id === centerId ? 0 : density.link * (1.15 + ringIndex * 0.58 + group * 0.16);
    return {
      id: node.id,
      node,
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
    };
  });
  const simulationLinks: LayoutLink[] = edges.map((edge) => ({
    source: edge.source,
    target: edge.target,
    kind: edge.kind,
  }));

  const simulation = forceSimulation<LayoutNode>(simulationNodes)
    .force(
      "radial",
      forceRadial<LayoutNode>((node) => {
        if (node.id === centerId) return 0;
        const group = kindIndex.get(node.node.kind) ?? 0;
        const connectedness = Math.min(8, degree.get(node.id) ?? 0);
        return density.link * (1.2 + group * 0.22 + Math.max(0, 7 - connectedness) * 0.12);
      }, 0, 0).strength(0.18),
    )
    .force(
      "link",
      forceLink<LayoutNode, LayoutLink>(simulationLinks)
        .id((node) => node.id)
        .distance((link) => (link.kind === "CONTAINS" ? density.link * 0.88 : density.link * 1.12))
        .strength(0.16),
    )
    .force("charge", forceManyBody<LayoutNode>().strength(-density.charge * 0.72))
    .force("collide", forceCollide<LayoutNode>().radius((node) => nodeRadius(node.node) + density.padding))
    .stop();

  for (let index = 0; index < Math.max(120, density.ticks - 40); index += 1) {
    simulation.tick();
  }

  const points = simulationNodes.reduce<Record<string, Point>>((accumulator, node) => {
    accumulator[node.id] = { x: node.x ?? 0, y: node.y ?? 0 };
    return accumulator;
  }, {});
  centerPoints(points);
  return points;
}

function withPinnedAndBounds(points: Record<string, Point>, pinnedPositions: Record<string, Point>): GraphLayout {
  const merged = Object.entries(points).reduce<Record<string, Point>>((accumulator, [id, point]) => {
    accumulator[id] = pinnedPositions[id] ?? point;
    return accumulator;
  }, {});
  return {
    points: merged,
    bounds: expandBounds(boundsForPoints(merged), graphFitPadding),
  };
}

function boundsForPoints(points: Record<string, Point>): GraphBounds {
  const values = Object.values(points);
  if (values.length === 0) {
    return { minX: -100, minY: -100, maxX: 100, maxY: 100 };
  }
  return values.reduce<GraphBounds>(
    (bounds, point) => ({
      minX: Math.min(bounds.minX, point.x),
      minY: Math.min(bounds.minY, point.y),
      maxX: Math.max(bounds.maxX, point.x),
      maxY: Math.max(bounds.maxY, point.y),
    }),
    {
      minX: values[0].x,
      minY: values[0].y,
      maxX: values[0].x,
      maxY: values[0].y,
    },
  );
}

function expandBounds(bounds: GraphBounds, padding: number): GraphBounds {
  return {
    minX: bounds.minX - padding,
    minY: bounds.minY - padding,
    maxX: bounds.maxX + padding,
    maxY: bounds.maxY + padding,
  };
}

function centerPoints(points: Record<string, Point>) {
  const bounds = boundsForPoints(points);
  const centerX = (bounds.minX + bounds.maxX) / 2;
  const centerY = (bounds.minY + bounds.maxY) / 2;
  for (const point of Object.values(points)) {
    point.x -= centerX;
    point.y -= centerY;
  }
}

function seededAngle(id: string, index: number) {
  const hash = Math.abs(hashString(id));
  const goldenAngle = Math.PI * (3 - Math.sqrt(5));
  return (index * goldenAngle + (hash % 360) * (Math.PI / 180)) % (Math.PI * 2);
}

function edgeCurve(edge: WorkGraphEdge, source: Point, target: Point) {
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const length = Math.max(1, Math.hypot(dx, dy));
  const unitX = dx / length;
  const unitY = dy / length;
  const perpX = -unitY;
  const perpY = unitX;
  const offset = (((Math.abs(hashString(edge.id)) % 9) - 4) * 7) / Math.max(1, Math.min(2.8, length / 80));
  const start = {
    x: source.x + unitX * 18,
    y: source.y + unitY * 18,
  };
  const end = {
    x: target.x - unitX * 20,
    y: target.y - unitY * 20,
  };
  const mid = {
    x: (start.x + end.x) / 2 + perpX * offset,
    y: (start.y + end.y) / 2 + perpY * offset,
  };
  return {
    label: mid,
    path: `M ${start.x} ${start.y} Q ${mid.x} ${mid.y} ${end.x} ${end.y}`,
  };
}

function fitGraphToViewport(bounds: GraphBounds): GraphTransform {
  const width = Math.max(1, bounds.maxX - bounds.minX);
  const height = Math.max(1, bounds.maxY - bounds.minY);
  const scale = clamp(
    Math.min(graphViewportWidth / width, graphViewportHeight / height),
    0.12,
    1.8,
  );
  return {
    k: scale,
    x: graphViewportWidth / 2 - ((bounds.minX + bounds.maxX) / 2) * scale,
    y: graphViewportHeight / 2 - ((bounds.minY + bounds.maxY) / 2) * scale,
  };
}

function viewportPointFromPointer(
  event: { clientX: number; clientY: number },
  svg: SVGSVGElement | null,
): Point | null {
  if (!svg) {
    return null;
  }
  const rect = svg.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) {
    return null;
  }
  return {
    x: ((event.clientX - rect.left) / rect.width) * graphViewportWidth,
    y: ((event.clientY - rect.top) / rect.height) * graphViewportHeight,
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

function limitGraphNodes(
  nodes: WorkGraphNode[],
  edges: WorkGraphEdge[],
  selectedId: string | null,
  limit: number,
) {
  if (nodes.length <= limit) {
    return nodes;
  }
  const degree = nodeDegree(edges);
  const selectedNeighborhood = selectedId ? focusedGraphNodeIds(edges, selectedId) : new Set<string>();
  const allowed = new Set(
    [...nodes]
      .sort((first, second) => {
        const selectedDelta = Number(selectedNeighborhood.has(second.id)) - Number(selectedNeighborhood.has(first.id));
        if (selectedDelta) return selectedDelta;
        const scoreDelta =
          (degree.get(second.id) ?? 0) * 3 +
          second.weight * 2 -
          ((degree.get(first.id) ?? 0) * 3 + first.weight * 2);
        return scoreDelta || first.label.localeCompare(second.label);
      })
      .slice(0, limit)
      .map((node) => node.id),
  );
  return nodes.filter((node) => allowed.has(node.id));
}

function GraphEdge({
  edge,
  layout,
  onSelect,
  selectedEdgeId,
  selectedNodeId,
}: {
  edge: WorkGraphEdge;
  layout: Record<string, Point>;
  onSelect: () => void;
  selectedEdgeId: string | null;
  selectedNodeId: string | null;
}) {
  const source = layout[edge.source];
  const target = layout[edge.target];
  if (!source || !target) {
    return null;
  }

  const isSelectedEdge = selectedEdgeId === edge.id;
  const isRelatedToSelectedNode = Boolean(
    selectedNodeId && (edge.source === selectedNodeId || edge.target === selectedNodeId),
  );
  const curve = edgeCurve(edge, source, target);
  const showLabel = isSelectedEdge || isRelatedToSelectedNode;

  return (
    <g className="cursor-pointer" onClick={(event) => {
      event.stopPropagation();
      onSelect();
    }}>
      <path
        d={curve.path}
        fill="none"
        stroke="transparent"
        strokeLinecap="round"
        strokeWidth={12}
      />
      <path
        d={curve.path}
        fill="none"
        markerEnd="url(#brain-edge-arrow)"
        stroke={edgeStroke(edge.kind)}
        strokeLinecap="round"
        strokeOpacity={selectedNodeId && !isRelatedToSelectedNode ? 0.16 : 0.72}
        strokeWidth={isSelectedEdge ? 3 : isRelatedToSelectedNode ? 2.1 : 1.2}
      />
      {showLabel ? (
        <text
          fill="#71717a"
          fontSize="9"
          fontWeight="600"
          paintOrder="stroke"
          pointerEvents="none"
          stroke="#ffffff"
          strokeWidth="4"
          textAnchor="middle"
          x={curve.label.x}
          y={curve.label.y - 5}
        >
          {edge.label}
        </text>
      ) : null}
    </g>
  );
}

function GraphNode({
  isDimmed,
  isSelected,
  node,
  onDoubleClick,
  onPointerDown,
  onSelect,
  point,
  isPinned,
  labelMode,
}: {
  isDimmed: boolean;
  isSelected: boolean;
  isPinned: boolean;
  labelMode: GraphLabelMode;
  node: WorkGraphNode;
  onDoubleClick: () => void;
  onPointerDown: (event: PointerEvent<SVGGElement>) => void;
  onSelect: () => void;
  point?: Point;
}) {
  if (!point) {
    return null;
  }

  const colors = colorsForKind(node.kind);
  const radius = nodeRadius(node);
  const showLabel =
    labelMode === "all" ||
    isSelected ||
    (labelMode === "smart" && !isDimmed && (node.weight >= 5 || radius >= 16));

  return (
    <g
      className="cursor-pointer outline-none"
      onClick={onSelect}
      onDoubleClick={onDoubleClick}
      onPointerDown={onPointerDown}
      role="button"
      tabIndex={0}
    >
      <circle
        cx={point.x}
        cy={point.y}
        fill={colors.fill}
        opacity={isDimmed ? 0.32 : 1}
        r={radius}
        stroke={isSelected ? "#18181b" : colors.stroke}
        strokeWidth={isSelected ? 3 : 2}
      />
      {isPinned ? (
        <circle cx={point.x + radius * 0.72} cy={point.y - radius * 0.72} fill="#18181b" r={3.5} />
      ) : null}
      {showLabel ? (
        <text
          fill={colors.text}
          fontSize="11"
          fontWeight="600"
          opacity={isDimmed ? 0.45 : 1}
          paintOrder="stroke"
          pointerEvents="none"
          stroke="#ffffff"
          strokeWidth="4"
          textAnchor="middle"
          x={point.x}
          y={point.y + radius + 14}
        >
          {shortLabel(node.label)}
        </text>
      ) : null}
    </g>
  );
}

function GraphMiniMap({
  bounds,
  nodes,
  points,
  transform,
}: {
  bounds: GraphBounds;
  nodes: WorkGraphNode[];
  points: Record<string, Point>;
  transform: GraphTransform;
}) {
  const width = 154;
  const height = 104;
  const x = graphViewportWidth - width - 16;
  const y = 16;
  const mapBounds = expandBounds(bounds, 80);
  const scale = Math.min(width / Math.max(1, mapBounds.maxX - mapBounds.minX), height / Math.max(1, mapBounds.maxY - mapBounds.minY));
  const toMini = (point: Point) => ({
    x: x + (point.x - mapBounds.minX) * scale,
    y: y + (point.y - mapBounds.minY) * scale,
  });
  const visibleWorld = {
    minX: (0 - transform.x) / transform.k,
    minY: (0 - transform.y) / transform.k,
    maxX: (graphViewportWidth - transform.x) / transform.k,
    maxY: (graphViewportHeight - transform.y) / transform.k,
  };
  const topLeft = toMini({ x: visibleWorld.minX, y: visibleWorld.minY });
  const bottomRight = toMini({ x: visibleWorld.maxX, y: visibleWorld.maxY });

  return (
    <g opacity={0.9}>
      <rect fill="#ffffff" height={height} rx={6} stroke="#e4e4e7" width={width} x={x} y={y} />
      {nodes.map((node) => {
        const point = points[node.id];
        if (!point) return null;
        const mini = toMini(point);
        return <circle cx={mini.x} cy={mini.y} fill={colorsForKind(node.kind).stroke} key={node.id} r={1.8} />;
      })}
      <rect
        fill="none"
        height={Math.max(8, bottomRight.y - topLeft.y)}
        rx={3}
        stroke="#18181b"
        strokeDasharray="3 3"
        width={Math.max(8, bottomRight.x - topLeft.x)}
        x={topLeft.x}
        y={topLeft.y}
      />
    </g>
  );
}

function PropertyGrid({ value }: { value: Record<string, unknown> }) {
  const entries = Object.entries(value ?? {}).filter(([, entryValue]) => entryValue !== undefined);
  if (entries.length === 0) {
    return null;
  }
  return (
    <div>
      <p className="field-label mb-2">Stored properties</p>
      <div className="max-h-[360px] overflow-auto rounded-xl border border-zinc-100 dark:border-zinc-700">
        {entries.map(([key, entryValue]) => (
          <div className="grid grid-cols-[110px_minmax(0,1fr)] border-b border-neutral-100 text-xs last:border-b-0 dark:border-zinc-700" key={key}>
            <div className="break-words bg-zinc-50 px-2 py-2 font-semibold text-zinc-500 dark:bg-zinc-950">
              {key}
            </div>
            <div className="break-words px-2 py-2 font-mono text-[11px] leading-5 text-zinc-700 dark:text-neutral-200">
              {formatPropertyValue(entryValue)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function focusedGraphNodeIds(edges: WorkGraphEdge[], selectedId: string) {
  const ids = new Set([selectedId]);
  const firstHop = new Set<string>();

  for (const edge of edges) {
    if (edge.source === selectedId) {
      firstHop.add(edge.target);
    }
    if (edge.target === selectedId) {
      firstHop.add(edge.source);
    }
  }

  for (const id of firstHop) {
    ids.add(id);
  }

  for (const edge of edges) {
    if (firstHop.has(edge.source)) {
      ids.add(edge.target);
    }
    if (firstHop.has(edge.target)) {
      ids.add(edge.source);
    }
  }

  return ids;
}

function relatedLabel(edge: WorkGraphEdge, selectedId: string, graph: WorkGraph | null) {
  if (!graph) {
    return edge.id;
  }

  const relatedId = edge.source === selectedId ? edge.target : edge.source;
  return graph.nodes.find((node) => node.id === relatedId)?.label ?? relatedId;
}

function nodeLabelById(graph: WorkGraph | null, id: string) {
  return graph?.nodes.find((node) => node.id === id)?.label ?? id;
}

function formatPropertyValue(value: unknown) {
  if (value === null) {
    return "null";
  }
  if (value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value.length > 1600 ? `${value.slice(0, 1600)}...` : value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  try {
    const formatted = JSON.stringify(value, null, 2);
    return formatted.length > 2400 ? `${formatted.slice(0, 2400)}...` : formatted;
  } catch {
    return String(value);
  }
}

function edgeStroke(kind: string) {
  if (kind.includes("SOURCE") || kind.includes("MEMORY")) {
    return "#14b8a6";
  }
  if (kind.includes("TARGET")) {
    return "#fb923c";
  }
  if (kind.includes("PRODUCED") || kind.includes("GENERATED")) {
    return "#a78bfa";
  }
  if (kind.includes("PROMOTED") || kind.includes("SUGGESTS")) {
    return "#facc15";
  }
  if (kind.includes("BLOCKED")) return "#ef4444";
  return "#d4d4d8";
}

function nodeRadius(node: WorkGraphNode) {
  return Math.max(11, Math.min(25, 9 + node.weight * 1.35));
}

function colorsForKind(kind: string) {
  const known = baseKindColors[kind];
  if (known) return known;
  const index = Math.abs(hashString(kind)) % fallbackKindColors.length;
  return fallbackKindColors[index];
}

function kindLabel(kind: string) {
  return baseKindLabels[kind] ?? titleCase(kind);
}

function relationLabel(kind: string) {
  return titleCase(kind.toLowerCase());
}

function titleCase(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

function hashString(value: string) {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash << 5) - hash + value.charCodeAt(index);
    hash |= 0;
  }
  return hash;
}

function shortLabel(label: string) {
  if (label.length <= 22) {
    return label;
  }

  return `${label.slice(0, 19)}...`;
}

function rowString(row: Record<string, unknown>, key: string) {
  const value = row[key];
  return typeof value === "string" ? value : "";
}

function rowNumber(row: Record<string, unknown>, key: string) {
  const value = row[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function brainRlScore(node: WorkGraphNode) {
  const score = node.properties?.brain_rl;
  if (!score || typeof score !== "object" || Array.isArray(score)) {
    return null;
  }
  const value = (score as Record<string, unknown>).score;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function resolveBrainPreview(
  result: BrainTemplateResult,
  row: Record<string, unknown>,
): BrainPreviewState {
  const rowId = rowString(row, "id");
  const rowUrl = rowString(row, "url");
  const node = result.graph.nodes.find((candidate) => candidate.id === rowId) ?? null;
  const targetNode = node ? findBrainPreviewTargetNode(result.graph, node) : null;
  const target = targetNode
    ? previewTargetFromNode(targetNode)
    : rowUrl
      ? {
          id: rowId || rowUrl,
          kind: rowString(row, "kind") || "item",
          label: rowString(row, "title") || rowUrl,
          subtitle: rowString(row, "summary") || null,
          status: rowString(row, "status") || null,
          url: rowUrl,
          node: null,
        }
      : null;
  const related = node
    ? collectRelatedPreviewNodes(result.graph, node.id, 3)
        .filter((candidate) => candidate.id !== target?.id)
        .sort((left, right) => previewNodePriority(right) - previewNodePriority(left))
        .slice(0, 8)
    : [];

  return {
    template: result.template,
    row,
    rowId,
    node,
    target,
    related,
  };
}

function findBrainPreviewTargetNode(graph: WorkGraph, node: WorkGraphNode) {
  const directSource = findDirectSourceNode(graph, node);
  const deeperSource =
    directSource && directSource.kind === "email_followup"
      ? findDirectSourceNode(graph, directSource)
      : null;
  if (deeperSource && nodePreviewUrl(deeperSource)) {
    return deeperSource;
  }
  if (directSource && nodePreviewUrl(directSource)) {
    return directSource;
  }
  if (!isDerivedBrainNode(node) && nodePreviewUrl(node)) {
    return node;
  }

  const candidates = [node, ...collectRelatedPreviewNodes(graph, node.id, 3)]
    .filter((candidate) => Boolean(nodePreviewUrl(candidate)))
    .sort((left, right) => previewNodePriority(right) - previewNodePriority(left));
  return candidates[0] ?? null;
}

function findDirectSourceNode(graph: WorkGraph, node: WorkGraphNode) {
  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const sourceEdge = graph.edges.find((edge) => {
    if (node.kind === "attention_signal") {
      return edge.kind === "HAS_ATTENTION" && edge.target === node.id;
    }
    if (node.kind === "open_loop") {
      return edge.kind === "HAS_OPEN_LOOP" && edge.target === node.id;
    }
    if (node.kind === "email_followup") {
      return edge.kind === "HAS_FOLLOWUP" && edge.target === node.id;
    }
    return false;
  });
  return sourceEdge ? nodeById.get(sourceEdge.source) ?? null : null;
}

function collectRelatedPreviewNodes(graph: WorkGraph, nodeId: string, maxHops: number) {
  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  const visited = new Set([nodeId]);
  const queue: Array<{ id: string; hops: number }> = [{ id: nodeId, hops: 0 }];
  const related: WorkGraphNode[] = [];

  while (queue.length > 0) {
    const current = queue.shift();
    if (!current || current.hops >= maxHops) continue;
    for (const edge of graph.edges) {
      if (edge.source !== current.id && edge.target !== current.id) continue;
      const nextId = edge.source === current.id ? edge.target : edge.source;
      if (visited.has(nextId)) continue;
      visited.add(nextId);
      const nextNode = nodeById.get(nextId);
      if (!nextNode) continue;
      related.push(nextNode);
      queue.push({ id: nextId, hops: current.hops + 1 });
    }
  }

  return related;
}

function previewTargetFromNode(node: WorkGraphNode): BrainPreviewTarget {
  return {
    id: node.id,
    kind: node.kind,
    label: node.label,
    subtitle: node.subtitle,
    status: node.status,
    url: nodePreviewUrl(node),
    node,
  };
}

function isDerivedBrainNode(node: WorkGraphNode) {
  return ["attention_signal", "open_loop", "inference"].includes(node.kind);
}

function previewNodePriority(node: WorkGraphNode) {
  const order = [
    "email_thread",
    "deliverable",
    "task",
    "meeting",
    "calendar_event",
    "file",
    "trace_folder",
    "email_followup",
    "stakeholder",
    "initiative",
    "capture",
    "ask_chat",
    "meeting_action",
  ];
  const index = order.indexOf(node.kind);
  const base = index === -1 ? 0 : order.length - index;
  const directUrlBonus = node.url ? 4 : 0;
  return base + directUrlBonus + node.weight / 10;
}

function nodePreviewUrl(node: WorkGraphNode) {
  return node.url || inferNodeUrl(node);
}

function inferNodeUrl(node: WorkGraphNode) {
  const payload = previewPayload(node);
  const sourceId = node.entity_id;
  switch (node.kind) {
    case "deliverable":
      return sourceId ? `/deliverables/${sourceId}` : null;
    case "task": {
      const deliverableId = previewString(payload.deliverable_id);
      return deliverableId ? `/deliverables/${deliverableId}` : null;
    }
    case "email_thread":
      return sourceId ? `/email?thread=${encodeURIComponent(sourceId)}` : null;
    case "email_followup": {
      const threadId = previewString(payload.thread_id);
      return threadId ? `/email?thread=${encodeURIComponent(threadId)}` : null;
    }
    case "meeting":
      return sourceId ? `/meetings/${sourceId}` : null;
    case "meeting_action": {
      const meetingId = previewString(payload.meeting_id);
      return meetingId ? `/meetings/${meetingId}` : null;
    }
    case "calendar_event":
      return "/week";
    case "file":
      return sourceId ? `/files?file=${encodeURIComponent(sourceId)}` : "/files";
    case "trace_folder":
      return sourceId ? `/files?folder=${encodeURIComponent(sourceId)}` : "/files";
    case "capture":
      return sourceId ? `/captures?selected=${encodeURIComponent(sourceId)}` : "/captures";
    case "stakeholder":
      return sourceId ? `/stakeholders/${sourceId}` : "/stakeholders";
    case "initiative":
      return sourceId ? `/initiatives/${sourceId}` : "/initiatives";
    case "ask_chat":
      return sourceId ? `/ask?chat=${encodeURIComponent(sourceId)}` : "/ask";
    default:
      return null;
  }
}

function previewPayload(node: WorkGraphNode): Record<string, unknown> {
  const payload = node.properties?.payload;
  return payload && typeof payload === "object" && !Array.isArray(payload)
    ? (payload as Record<string, unknown>)
    : {};
}

function previewString(value: unknown) {
  return typeof value === "string" && value.trim() ? value : "";
}

function formatPreviewKind(kind: string) {
  return kind.replace(/_/g, " ");
}

function ToggleSwitch({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="inline-flex h-8 items-center gap-2 rounded-lg border border-zinc-100 bg-white px-3 text-[13px] font-medium text-neutral-800 shadow-sm dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100">
      <button
        aria-checked={checked}
        aria-label="Toggle memory"
        className={[
          "relative h-5 w-9 rounded-full transition",
          checked ? "bg-zinc-950 dark:bg-zinc-100" : "bg-neutral-200 dark:bg-neutral-700",
        ].join(" ")}
        onClick={() => onChange(!checked)}
        role="switch"
        type="button"
      >
        <span
          className={[
            "absolute top-0.5 h-4 w-4 rounded-full bg-white shadow-sm transition dark:bg-zinc-950",
            checked ? "left-[18px]" : "left-0.5",
          ].join(" ")}
        />
      </button>
      <span>{label}</span>
    </div>
  );
}

function RetrievalDiagnostics({
  busy,
  feedbackSent,
  onFeedback,
  retrieval,
}: {
  busy: boolean;
  feedbackSent: MemoryFeedback | null;
  onFeedback: (value: MemoryFeedback) => void;
  retrieval: MemoryRetrievalResult;
}) {
  const { diagnostics, scored } = retrieval;
  const flags: string[] = [];
  if (diagnostics.semantic_used) {
    flags.push(`semantic (${diagnostics.embedding_model ?? "embed"})`);
  } else if (diagnostics.embedding_error) {
    flags.push("semantic skipped");
  }
  if (diagnostics.lexical_used) flags.push("BM25 lexical");
  if (diagnostics.procedural_pin_count > 0) {
    flags.push(`procedural pin set (${diagnostics.procedural_pin_count})`);
  }

  return (
    <div className="mt-4 space-y-3">
      <div className="rounded-md bg-zinc-50 p-3 text-[11px] text-zinc-500">
        <div className="flex flex-wrap items-center gap-1.5">
          {flags.length > 0 ? (
            flags.map((flag) => (
              <span
                className="rounded-full bg-white px-2 py-0.5 font-medium text-zinc-700 ring-1 ring-neutral-200"
                key={flag}
              >
                {flag}
              </span>
            ))
          ) : (
            <span>No retrieval signals fired.</span>
          )}
        </div>
        {diagnostics.embedding_error ? (
          <p className="mt-2 text-rose-500">Embedding error: {diagnostics.embedding_error}</p>
        ) : null}
      </div>

      <div className="max-h-64 overflow-y-auto rounded-md bg-zinc-50 p-3 text-xs leading-5 text-zinc-600">
        <pre className="whitespace-pre-wrap font-sans">{retrieval.context}</pre>
      </div>

      {scored.length > 0 ? (
        <div className="rounded-xl border border-zinc-100 dark:border-zinc-700">
          <p className="border-b border-zinc-100 bg-zinc-50 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-wide text-zinc-500 dark:border-zinc-700 dark:bg-zinc-950">
            Scoring
          </p>
          <ul className="divide-y divide-neutral-200 dark:divide-neutral-800">
            {scored.map((item) => (
              <li className="px-3 py-2 text-xs" key={item.memory.id}>
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate font-medium text-zinc-700 dark:text-neutral-200">
                    {item.memory.title}
                  </span>
                  <span className="font-mono tabular-nums text-zinc-500">
                    {item.score.toFixed(2)}
                  </span>
                </div>
                <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-neutral-400">
                  <span>sem {item.semantic_score.toFixed(2)}</span>
                  <span>lex {item.lexical_score.toFixed(2)}</span>
                  <span>recency {item.recency_score.toFixed(2)}</span>
                  {item.procedural_pin ? <span>pinned</span> : null}
                </div>
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {retrieval.retrieval_id ? (
        <div className="flex items-center gap-2 text-xs text-zinc-500">
          <span>How was this retrieval?</span>
          <button
            aria-label="Mark retrieval as useful"
            className={[
              "btn h-7 px-2",
              feedbackSent === "useful" ? "border-emerald-500 text-emerald-600" : "",
            ].join(" ")}
            disabled={busy || Boolean(feedbackSent)}
            onClick={() => onFeedback("useful")}
            type="button"
          >
            <ThumbsUp aria-hidden="true" size={13} />
            Useful
          </button>
          <button
            aria-label="Mark retrieval as wrong"
            className={[
              "btn h-7 px-2",
              feedbackSent === "wrong" ? "border-rose-500 text-rose-600" : "",
            ].join(" ")}
            disabled={busy || Boolean(feedbackSent)}
            onClick={() => onFeedback("wrong")}
            type="button"
          >
            <ThumbsDown aria-hidden="true" size={13} />
            Wrong
          </button>
          {feedbackSent ? <span className="text-emerald-600">Recorded.</span> : null}
        </div>
      ) : null}
    </div>
  );
}

function prettifyEventDetail(raw: string): string {
  try {
    const parsed = JSON.parse(raw);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return raw;
  }
}
