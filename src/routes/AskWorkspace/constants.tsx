// Shared constants for AskWorkspace and its submodules. Anything with JSX
// (icons embedded in the value) lives here so submodules can import without
// pulling the whole route. String-only constants are next to their consumers.

import {
  BookOpen,
  Brain,
  CalendarDays,
  CheckCircle2,
  CircleAlert,
  CircleDot,
  Database,
  FileText,
  GitBranch,
  Globe,
  Inbox,
  KanbanSquare,
  Layers3,
  ListTodo,
  Mail,
  MessageSquareText,
  Mic,
  Paperclip,
  PenLine,
  RefreshCw,
  Sparkles,
  UserRound,
  UsersRound,
  Wrench,
} from "lucide-react";
import type { ReactNode } from "react";
import type {
  AgentMode,
  ComposerCommand,
  MemoryType,
  MentionTarget,
  PermissionMode,
  ToolSpec,
} from "./state";

export const AGENT_MODES: { key: AgentMode; label: string; icon: ReactNode; title: string }[] = [
  { key: "ask", label: "Ask", icon: <Sparkles size={12} />, title: "Concise answer mode" },
  { key: "research", label: "Research", icon: <BookOpen size={12} />, title: "Multi-step source-backed mode" },
  { key: "act", label: "Act", icon: <Wrench size={12} />, title: "Tool-using action mode" },
];

export const AGENT_MODE_CONTEXT: Record<AgentMode, string> = {
  ask: "Answer directly and use tools only when they materially improve the answer. Keep the final response concise.",
  research:
    "Run a multi-step research pass across workspace records, past conversations, email, captures, meetings, and related work graph context. Prefer source-backed synthesis and return references.",
  act:
    "Use available read/write tools when appropriate, but ask the user before destructive, ambiguous, or high-impact changes. Surface any needed clarification as an ask_user_question.",
};

export const PERMISSION_MODE_CONTEXT: Record<PermissionMode, string> = {
  confirm:
    "Confirm before write tools, irreversible changes, email actions, or any action with ambiguous user intent.",
  auto_read:
    "Read-only tools can run without asking. Confirm before all write, email-linking, state-changing, or destructive actions.",
  auto_safe:
    "Read-only and low-risk organizational actions may proceed. Confirm destructive, external, irreversible, or ambiguous actions.",
};

export const MEMORY_TYPE_META: Record<MemoryType, { label: string; description: string; icon: ReactNode }> = {
  user: {
    label: "User",
    description: "Role, preferences, responsibilities, and working style.",
    icon: <UserRound size={13} />,
  },
  feedback: {
    label: "Feedback",
    description: "Reusable guidance on how Trace should approach work.",
    icon: <MessageSquareText size={13} />,
  },
  project: {
    label: "Project",
    description: "Non-obvious goals, decisions, deadlines, and coordination context.",
    icon: <KanbanSquare size={13} />,
  },
  reference: {
    label: "Reference",
    description: "Pointers to where up-to-date information lives.",
    icon: <BookOpen size={13} />,
  },
};

export const TOOL_SPECS: ToolSpec[] = [
  { name: "get_workspace_summary", label: "Workspace", category: "memory", icon: <Database size={12} /> },
  { name: "retrieve_memory", label: "Durable memory", category: "memory", icon: <Brain size={12} /> },
  { name: "save_memory", label: "Save memory", category: "memory", icon: <Brain size={12} /> },
  { name: "get_work_graph_context", label: "Work graph", category: "memory", icon: <GitBranch size={12} /> },
  { name: "retrieve_brain_context", label: "Brain context", category: "memory", icon: <Brain size={12} /> },
  { name: "query_brain_cypher", label: "Brain query", category: "memory", icon: <GitBranch size={12} /> },
  { name: "get_recent_activity", label: "Recent", category: "memory", icon: <RefreshCw size={12} /> },
  { name: "get_current_week", label: "Week", category: "read", icon: <CalendarDays size={12} /> },
  { name: "search_deliverables", label: "Find work", category: "read", icon: <KanbanSquare size={12} /> },
  { name: "get_deliverable_detail", label: "Work detail", category: "read", icon: <FileText size={12} /> },
  { name: "get_deliverables_by_state", label: "By state", category: "read", icon: <KanbanSquare size={12} /> },
  { name: "get_high_priority_deliverables", label: "Priority", category: "read", icon: <Sparkles size={12} /> },
  { name: "get_blocked_deliverables", label: "Blocked", category: "read", icon: <CircleAlert size={12} /> },
  { name: "list_initiatives", label: "Initiatives", category: "read", icon: <Layers3 size={12} /> },
  { name: "get_initiative_detail", label: "Initiative detail", category: "read", icon: <Layers3 size={12} /> },
  { name: "get_stakeholders", label: "Stakeholders", category: "read", icon: <UsersRound size={12} /> },
  { name: "get_stakeholder_deliverables", label: "Stakeholder work", category: "read", icon: <UsersRound size={12} /> },
  { name: "search_meetings", label: "Meetings", category: "read", icon: <Mic size={12} /> },
  { name: "get_meeting_detail", label: "Meeting detail", category: "read", icon: <Mic size={12} /> },
  { name: "list_pending_tasks", label: "Pending tasks", category: "read", icon: <ListTodo size={12} /> },
  { name: "search_captures", label: "Captures", category: "read", icon: <Inbox size={12} /> },
  { name: "search_conversations", label: "Conversations", category: "read", icon: <MessageSquareText size={12} /> },
  { name: "get_conversation_detail", label: "Conversation detail", category: "read", icon: <MessageSquareText size={12} /> },
  { name: "search_email_threads", label: "Email search", category: "email", icon: <Mail size={12} /> },
  { name: "get_email_category_summary", label: "Email categories", category: "email", icon: <Mail size={12} /> },
  { name: "get_email_thread", label: "Email detail", category: "email", icon: <Mail size={12} /> },
  { name: "create_deliverable_from_email", label: "Email to work", category: "email", icon: <KanbanSquare size={12} /> },
  { name: "link_email_thread_to_deliverable", label: "Link email", category: "email", icon: <Mail size={12} /> },
  { name: "link_email_thread_to_initiative", label: "Link initiative", category: "email", icon: <Mail size={12} /> },
  { name: "capture_email_thread", label: "Capture email", category: "email", icon: <Inbox size={12} /> },
  { name: "add_deliverable_note", label: "Work note", category: "write", icon: <PenLine size={12} /> },
  { name: "add_initiative_note", label: "Initiative note", category: "write", icon: <PenLine size={12} /> },
  { name: "create_capture", label: "Capture", category: "write", icon: <Inbox size={12} /> },
  { name: "update_deliverable_state", label: "Move state", category: "write", icon: <KanbanSquare size={12} /> },
  { name: "set_deliverable_focus", label: "Set focus", category: "write", icon: <Sparkles size={12} /> },
  { name: "add_deliverable_task", label: "Add task", category: "write", icon: <CheckCircle2 size={12} /> },
  { name: "update_task_status", label: "Task status", category: "write", icon: <CheckCircle2 size={12} /> },
  { name: "update_deliverable_metadata", label: "Metadata", category: "write", icon: <Wrench size={12} /> },
  { name: "flag_new_deliverable", label: "Flag candidate", category: "write", icon: <CircleDot size={12} /> },
  { name: "search_files", label: "Search files", category: "files", icon: <Paperclip size={12} /> },
  { name: "list_files_for_entity", label: "Entity files", category: "files", icon: <Paperclip size={12} /> },
  { name: "get_file_detail", label: "File detail", category: "files", icon: <FileText size={12} /> },
  { name: "ask_user_question", label: "Ask user", category: "clarify", icon: <UserRound size={12} /> },
  // Web tools — research mode only
  { name: "search_web", label: "Web search", category: "web", icon: <Globe size={12} /> },
  { name: "fetch_url", label: "Fetch URL", category: "web", icon: <Globe size={12} /> },
];

export const SAMPLE_PROMPTS = [
  "What should I focus on next?",
  "What is blocked right now?",
  "Catch me up on recent work",
  "Which emails need follow-up?",
];

export const COMPOSER_COMMANDS: ComposerCommand[] = [
  { name: "ask", label: "/ask", badge: "mode", description: "Switch to concise answer mode and submit the rest of the prompt." },
  { name: "research", label: "/research", badge: "mode", description: "Run a source-backed workspace research pass." },
  { name: "act", label: "/act", badge: "mode", description: "Use write-capable tools with permission checks." },
  { name: "memory", label: "/memory", badge: "panel", description: "Open the structured memory system." },
  { name: "remember", label: "/remember", badge: "memory", description: "Save the rest of the line as project memory." },
  { name: "forget", label: "/forget", badge: "memory", description: "Open memory so an outdated entry can be removed." },
  { name: "new", label: "/new", badge: "chat", description: "Archive the current chat and start a clean one." },
  { name: "chats", label: "/chats", badge: "chat", description: "Open past chats and resume a saved conversation." },
  { name: "tasks", label: "/tasks", badge: "panel", description: "Open the run and task tracker." },
  { name: "tools", label: "/tools", badge: "panel", description: "Open the tool harness and recent usage." },
  { name: "settings", label: "/settings", badge: "panel", description: "Open permission and agent settings." },
  { name: "history", label: "/history", badge: "panel", description: "Open prompt history and reuse a prior request." },
  { name: "compact", label: "/compact", badge: "session", description: "Keep recent turns and trim older chat state." },
  { name: "clear", label: "/clear", badge: "session", description: "Clear this Ask chat session." },
  { name: "help", label: "/help", badge: "panel", description: "Show available commands." },
];

export const MENTION_TARGETS: MentionTarget[] = [
  {
    token: "@workspace",
    label: "Workspace",
    description: "Overview, current records, and work graph",
    context: "User explicitly mentioned workspace context. Start from workspace overview and related work graph records.",
    icon: <Database size={13} />,
  },
  {
    token: "@memory",
    label: "Memory",
    description: "Saved overall and session memory",
    context: "User explicitly mentioned memory. Prefer relevant durable memory and session continuity, but verify stale claims against current records.",
    icon: <Brain size={13} />,
  },
  {
    token: "@week",
    label: "Week",
    description: "Current week plan and deadlines",
    context: "User explicitly mentioned week. Inspect current week plan, due work, blocked work, and imminent deadlines.",
    icon: <CalendarDays size={13} />,
  },
  {
    token: "@email",
    label: "Email",
    description: "Email threads and follow-ups",
    context: "User explicitly mentioned email. Search email threads and connect follow-ups to workspace work where possible.",
    icon: <Mail size={13} />,
  },
  {
    token: "@meetings",
    label: "Meetings",
    description: "Meeting transcripts and decisions",
    context: "User explicitly mentioned meetings. Search meeting records for decisions, asks, owners, and unresolved items.",
    icon: <Mic size={13} />,
  },
  {
    token: "@captures",
    label: "Captures",
    description: "Loose notes and captured items",
    context: "User explicitly mentioned captures. Search captured notes for open loops and connect them to known work records.",
    icon: <Inbox size={13} />,
  },
  {
    token: "@initiatives",
    label: "Initiatives",
    description: "Initiative context and linked work",
    context: "User explicitly mentioned initiatives. Review initiative-level context and related deliverables.",
    icon: <Layers3 size={13} />,
  },
  {
    token: "@stakeholders",
    label: "Stakeholders",
    description: "People, owners, and stakeholder work",
    context: "User explicitly mentioned stakeholders. Use stakeholder records to identify owners, dependencies, and commitments.",
    icon: <UsersRound size={13} />,
  },
  {
    token: "@blocked",
    label: "Blocked",
    description: "Blocked or waiting work",
    context: "User explicitly mentioned blocked work. Prioritize blockers, waiting-for items, and recommended next actions.",
    icon: <CircleAlert size={13} />,
  },
  {
    token: "@recent",
    label: "Recent",
    description: "Recent activity across Trace",
    context: "User explicitly mentioned recent activity. Inspect latest activity before answering.",
    icon: <RefreshCw size={13} />,
  },
];
