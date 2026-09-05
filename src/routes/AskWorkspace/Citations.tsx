// Markdown rendering + citation handling for Ask turn answers. Extracted from
// AskWorkspace.tsx (E5). Only `MarkdownAnswer` and `ReferenceDisclosure` are
// consumed externally; the rest are internal to the rendering pipeline.

import { createContext, useContext, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowUpRight,
  BookOpen,
  Brain,
  CalendarDays,
  ChevronDown,
  ExternalLink,
  FileText,
  Globe,
  Inbox,
  KanbanSquare,
  Layers3,
  Mail,
  MessageSquareText,
  Mic,
  UsersRound,
} from "lucide-react";
import type { SearchResult, SearchResultKind } from "../../lib/types";
import { safeExternalUrl } from "../../lib/urlSafety";

const kindIcon: Record<SearchResultKind, ReactNode> = {
  deliverable: <KanbanSquare size={14} className="text-sky-500" />,
  initiative: <Layers3 size={14} className="text-violet-500" />,
  stakeholder: <UsersRound size={14} className="text-emerald-500" />,
  meeting: <Mic size={14} className="text-orange-500" />,
  capture: <Inbox size={14} className="text-zinc-500" />,
  email: <Mail size={14} className="text-rose-500" />,
  email_thread: <Mail size={14} className="text-rose-500" />,
  email_message: <Mail size={14} className="text-rose-500" />,
  calendar_event: <CalendarDays size={14} className="text-orange-500" />,
  file: <FileText size={14} className="text-zinc-500" />,
  ask_turn: <BookOpen size={14} className="text-violet-500" />,
  conversation: <MessageSquareText size={14} className="text-indigo-500" />,
  memory: <Brain size={14} className="text-teal-600" />,
  web: <Globe size={14} className="text-sky-400" />,
};

interface CitationRefsContextValue {
  refs: SearchResult[];
  /**
   * When provided, intercepts navigation for internal citation links. Used by
   * the Spotlight overlay: clicking a `/initiatives/abc` citation should
   * surface the main Trace window at that route and dismiss the overlay,
   * rather than navigating react-router inside the spotlight webview.
   */
  onNavigate?: (route: string) => void;
}

const CitationRefsContext = createContext<CitationRefsContextValue>({ refs: [] });

export function MarkdownAnswer({
  content,
  refs = [],
  streaming = false,
  onNavigate,
}: {
  content: string;
  refs?: SearchResult[];
  streaming?: boolean;
  onNavigate?: (route: string) => void;
}) {
  return (
    <CitationRefsContext.Provider value={{ refs, onNavigate }}>
      <MarkdownAnswerInner content={content} streaming={streaming} />
    </CitationRefsContext.Provider>
  );
}

function MarkdownAnswerInner({ content, streaming = false }: { content: string; streaming?: boolean }) {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let index = 0;
  let key = 0;

  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      index += 1;
      continue;
    }

    const fence = trimmed.match(/^```(\w+)?\s*$/);
    if (fence) {
      const language = fence[1];
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith("```")) {
        code.push(lines[index]);
        index += 1;
      }
      if (index < lines.length) {
        index += 1;
      }
      blocks.push(
        <pre
          className="my-3 overflow-x-auto rounded-xl bg-zinc-950 px-3 py-2 text-[12px] leading-5 text-zinc-100"
          key={`code-${key++}`}
        >
          {language ? (
            <span className="mb-2 block text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
              {language}
            </span>
          ) : null}
          <code>{code.join("\n")}</code>
        </pre>,
      );
      continue;
    }

    const heading = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      const level = heading[1].length;
      const headingText = heading[2].replace(/^[\p{Emoji_Presentation}\p{Extended_Pictographic}]\s*/u, "").trim();
      if (level === 1) {
        blocks.push(
          <div className="mt-5 flex items-center gap-2 text-[18px] font-semibold leading-7 text-zinc-950 first:mt-0" key={`heading-${key++}`}>
            <span className="h-[7px] w-[7px] shrink-0 rounded-[2px] bg-orange-400" />
            {renderInlineMarkdown(headingText, `heading-${key}`)}
          </div>,
        );
      } else if (level === 2) {
        blocks.push(
          <div className="mt-6 first:mt-0" key={`heading-${key++}`}>
            <div className="flex items-center gap-2 border-b border-zinc-100 pb-1.5">
              <span className="h-[6px] w-[6px] shrink-0 rounded-[2px] bg-orange-400" />
              <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-zinc-400">
                {headingText}
              </span>
            </div>
          </div>,
        );
      } else {
        blocks.push(
          <div className="flex items-center gap-2 border-b border-zinc-100 pb-1.5 text-[14px] font-semibold leading-6 text-zinc-800" style={{ marginTop: "1.5rem" }} key={`heading-${key++}`}>
            <span className="h-[5px] w-[5px] shrink-0 rounded-full bg-orange-400" />
            {renderInlineMarkdown(headingText, `heading-${key}`)}
          </div>,
        );
      }
      index += 1;
      continue;
    }

    if (isMarkdownTableStart(lines, index)) {
      const headers = splitMarkdownTableRow(lines[index]);
      index += 2;
      const rows: string[][] = [];
      while (index < lines.length && lines[index].includes("|") && lines[index].trim()) {
        rows.push(splitMarkdownTableRow(lines[index]));
        index += 1;
      }
      blocks.push(
        <div className="my-3 overflow-x-auto rounded-xl border border-zinc-100" key={`table-${key++}`}>
          <table className="min-w-full border-collapse text-left text-[13px]">
            <thead className="bg-zinc-100 text-zinc-700">
              <tr>
                {headers.map((header, headerIndex) => (
                  <th className="border-b border-zinc-100 px-3 py-2 font-semibold" key={`th-${headerIndex}`}>
                    {renderInlineMarkdown(header, `th-${key}-${headerIndex}`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-100">
              {rows.map((row, rowIndex) => (
                <tr key={`tr-${rowIndex}`}>
                  {headers.map((_, cellIndex) => (
                    <td className="px-3 py-2 align-top text-zinc-700" key={`td-${rowIndex}-${cellIndex}`}>
                      {renderInlineMarkdown(row[cellIndex] ?? "", `td-${key}-${rowIndex}-${cellIndex}`)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }

    if (/^[-*]\s+/.test(trimmed) || /^\d+\.\s+/.test(trimmed)) {
      const ordered = /^\d+\.\s+/.test(trimmed);
      const items: ReactNode[] = [];
      while (index < lines.length) {
        const item = lines[index].trim();
        const itemMatch = ordered ? item.match(/^\d+\.\s+(.+)$/) : item.match(/^[-*]\s+(.+)$/);
        if (!itemMatch) {
          break;
        }
        items.push(
          <li className="pl-1" key={`item-${key++}`}>
            {renderInlineMarkdown(itemMatch[1], `item-${key}`)}
          </li>,
        );
        index += 1;
      }
      const ListTag = ordered ? "ol" : "ul";
      blocks.push(
        <ListTag
          className={[
            "my-2 space-y-1 pl-5 text-[14px] leading-6 text-zinc-700",
            ordered ? "list-decimal" : "list-disc",
          ].join(" ")}
          key={`list-${key++}`}
        >
          {items}
        </ListTag>,
      );
      continue;
    }

    if (trimmed.startsWith("> ")) {
      const quotes: string[] = [];
      while (index < lines.length && lines[index].trim().startsWith("> ")) {
        quotes.push(lines[index].trim().replace(/^>\s?/, ""));
        index += 1;
      }
      blocks.push(
        <blockquote
          className="my-3 border-l-2 border-zinc-100 pl-3 text-[14px] leading-6 text-zinc-600"
          key={`quote-${key++}`}
        >
          {renderInlineMarkdown(quotes.join(" "), `quote-${key}`)}
        </blockquote>,
      );
      continue;
    }

    const paragraph: string[] = [];
    while (index < lines.length) {
      const current = lines[index];
      const currentTrimmed = current.trim();
      if (
        !currentTrimmed ||
        currentTrimmed.startsWith("```") ||
        /^(#{1,4})\s+/.test(currentTrimmed) ||
        /^[-*]\s+/.test(currentTrimmed) ||
        /^\d+\.\s+/.test(currentTrimmed) ||
        currentTrimmed.startsWith("> ")
      ) {
        break;
      }
      paragraph.push(currentTrimmed);
      index += 1;
    }

    blocks.push(
      <p
        className={[
          "my-2 text-[14px] leading-7 first:mt-0 last:mb-0 transition-colors duration-300",
          streaming ? "font-semibold text-zinc-950" : "text-zinc-700",
        ].join(" ")}
        key={`p-${key++}`}
      >
        {renderInlineMarkdown(paragraph.join(" "), `p-${key}`)}
      </p>,
    );
  }

  return <div className="space-y-1">{blocks}</div>;
}

function renderInlineMarkdown(text: string, keyPrefix: string) {
  const nodes: ReactNode[] = [];
  const tokenRegex = /(`[^`]+`|\*\*[^*]+?\*\*|\[\^\d+\]|\[\d+\]|\[[^\]]+\]\([^)]+\))/g;
  let cursor = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenRegex.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(text.slice(cursor, match.index));
    }

    const token = match[0];
    const key = `${keyPrefix}-${match.index}`;
    if (token.startsWith("`")) {
      nodes.push(
        <code className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[0.92em] text-zinc-700" key={key}>
          {token.slice(1, -1)}
        </code>,
      );
    } else if (token.startsWith("**")) {
      nodes.push(
        <strong className="font-semibold text-zinc-950" key={key}>
          {renderInlineMarkdown(token.slice(2, -2), `${key}-strong`)}
        </strong>,
      );
    } else if (token.startsWith("[^")) {
      const num = Number.parseInt(token.slice(2, -1), 10);
      nodes.push(<CitationMarker index={num} key={key} />);
    } else if (/^\[\d+\]$/.test(token)) {
      const num = Number.parseInt(token.slice(1, -1), 10);
      nodes.push(<CitationMarker index={num} key={key} />);
    } else {
      const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      const href = link ? safeMarkdownHref(link[2].trim()) : null;
      nodes.push(
        href ? (
          <InlineLink href={href} key={key}>
            {link?.[1]}
          </InlineLink>
        ) : (
          token
        ),
      );
    }
    cursor = match.index + token.length;
  }

  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }

  return nodes;
}

function InlineLink({ href, children }: { href: string; children: ReactNode }) {
  const { onNavigate } = useContext(CitationRefsContext);
  const isExternal = safeExternalUrl(href) !== null;
  const isInternal = href.startsWith("/") && !href.startsWith("//");
  const className =
    "inline-flex items-center gap-0.5 text-[14px] font-medium text-sky-600 underline decoration-sky-300 underline-offset-2 hover:text-sky-800";

  if (isInternal && onNavigate) {
    return (
      <button
        className={className}
        onClick={(event) => {
          event.preventDefault();
          onNavigate(href);
        }}
        type="button"
      >
        {children}
        <ExternalLink size={10} className="shrink-0 opacity-60" />
      </button>
    );
  }

  return (
    <a
      className={className}
      href={href}
      rel={isExternal ? "noreferrer" : undefined}
      target={isExternal ? "_blank" : undefined}
    >
      {children}
      <ExternalLink size={10} className="shrink-0 opacity-60" />
    </a>
  );
}

function CitationMarker({ index }: { index: number }) {
  const { refs, onNavigate } = useContext(CitationRefsContext);
  const target = refs[index - 1];
  if (!target) {
    return (
      <sup className="ml-0.5 text-[10px] text-zinc-400">[{index}]</sup>
    );
  }
  const title = `${target.kind}: ${target.title}`;
  const route = target.route ?? "";
  const safeExternalRoute = safeExternalUrl(route);
  const isExternal = safeExternalRoute !== null;
  const isInternal = route.startsWith("/") && !route.startsWith("//");
  const className =
    "ml-0.5 inline-flex h-4 min-w-[16px] items-center justify-center rounded-full bg-sky-50 px-1 align-text-top text-[10px] font-semibold text-sky-600 hover:bg-sky-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-300";
  if (isInternal) {
    if (onNavigate) {
      return (
        <button
          className={className}
          onClick={(event) => {
            event.preventDefault();
            onNavigate(route);
          }}
          title={title}
          type="button"
        >
          {index}
        </button>
      );
    }
    return (
      <Link className={className} title={title} to={route}>
        {index}
      </Link>
    );
  }
  if (isExternal) {
    return (
      <a className={className} href={safeExternalRoute!} rel="noreferrer" target="_blank" title={title}>
        {index}
      </a>
    );
  }
  return (
    <sup className="ml-0.5 text-[10px] text-zinc-400" title={title}>
      [{index}]
    </sup>
  );
}

function isMarkdownTableStart(lines: string[], index: number) {
  const header = lines[index]?.trim() ?? "";
  const separator = lines[index + 1]?.trim() ?? "";
  return (
    header.includes("|") &&
    /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?$/.test(separator)
  );
}

function splitMarkdownTableRow(row: string) {
  return row
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function safeMarkdownHref(href: string) {
  if ((href.startsWith("/") && !href.startsWith("//")) || href.startsWith("#")) {
    return href;
  }
  return safeExternalUrl(href);
}

export function ReferenceDisclosure({
  refs,
  onNavigate,
}: {
  refs: SearchResult[];
  onNavigate?: (route: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);

  const workspaceRefs = refs.filter((r) => r.kind !== "web");
  const webRefs = refs.filter((r) => r.kind === "web");
  const preview = refs.slice(0, 3);

  return (
    <div className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <button
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left"
        onClick={() => setExpanded((value) => !value)}
        type="button"
      >
        <BookOpen size={14} className="text-sky-500" />
        <span className="text-[12px] font-semibold text-zinc-950">Sources</span>
        <span className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
          {preview.map((ref) => (
            <span
              className="inline-flex min-w-0 items-center gap-1 rounded-md bg-zinc-100 px-1.5 py-0.5 text-[11px] font-medium text-zinc-500"
              key={`source-chip-${ref.kind}-${ref.entity_id}`}
              title={ref.title}
            >
              {kindIcon[ref.kind] ?? <FileText size={12} />}
              <span className="max-w-28 truncate">{ref.title}</span>
            </span>
          ))}
          {refs.length > preview.length ? (
            <span className="shrink-0 text-[11px] font-medium text-zinc-400">
              +{refs.length - preview.length}
            </span>
          ) : null}
        </span>
        <ChevronDown
          size={15}
          className={["text-zinc-400 transition-transform", expanded ? "rotate-180" : ""].join(" ")}
        />
      </button>
      <AnimatePresence initial={false}>
        {expanded ? (
          <motion.div
            animate={{ height: "auto", opacity: 1 }}
            className="overflow-hidden border-t border-zinc-100"
            exit={{ height: 0, opacity: 0 }}
            initial={{ height: 0, opacity: 0 }}
          >
            <div className="divide-y divide-zinc-100">
              {workspaceRefs.map((ref) => (
                <ReferenceLink
                  key={`${ref.kind}-${ref.entity_id}`}
                  refItem={ref}
                  onNavigate={onNavigate}
                />
              ))}
            </div>
            {webRefs.length > 0 ? (
              <div className="border-t border-zinc-100">
                <div className="flex items-center gap-1.5 px-3 py-2">
                  <Globe size={11} className="text-sky-400" />
                  <span className="text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
                    From the web
                  </span>
                </div>
                <div className="divide-y divide-zinc-100">
                  {webRefs.map((ref) => (
                    <ReferenceLink
                      key={`web-${ref.entity_id}`}
                      refItem={ref}
                      onNavigate={onNavigate}
                    />
                  ))}
                </div>
              </div>
            ) : null}
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

export function ReferenceLink({
  refItem,
  onNavigate,
}: {
  refItem: SearchResult;
  onNavigate?: (route: string) => void;
}) {
  const isInternal = refItem.route.startsWith("/") && !refItem.route.startsWith("//");
  const safeExternalRoute = safeExternalUrl(refItem.route);
  const isExternal = safeExternalRoute !== null;
  const isNavigable = isInternal || isExternal;

  const content = (
    <>
      <span className="shrink-0">{kindIcon[refItem.kind] ?? <FileText size={14} />}</span>
      <span className="min-w-0 flex-1">
        <span className={["block truncate text-[13px] font-medium", isNavigable ? "text-zinc-900" : "text-zinc-400"].join(" ")}>
          {refItem.title}
        </span>
        <span className="block truncate text-[11px] text-zinc-400">
          {!isNavigable ? "Not created yet" : refItem.subtitle ?? ""}
        </span>
      </span>
      {isNavigable
        ? <ArrowUpRight size={13} className="shrink-0 text-zinc-300" />
        : <span className="shrink-0 rounded-full bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400">candidate</span>
      }
    </>
  );

  const baseClass = "flex items-center gap-3 px-3 py-2.5";
  if (isInternal) {
    if (onNavigate) {
      return (
        <button
          className={`${baseClass} w-full text-left transition-colors hover:bg-zinc-50`}
          onClick={() => onNavigate(refItem.route)}
          type="button"
        >
          {content}
        </button>
      );
    }
    return <Link className={`${baseClass} transition-colors hover:bg-zinc-50`} to={refItem.route}>{content}</Link>;
  }
  if (isExternal) return <a className={`${baseClass} transition-colors hover:bg-zinc-50`} href={safeExternalRoute!} rel="noreferrer" target="_blank">{content}</a>;
  return <div className={`${baseClass} cursor-default opacity-60`}>{content}</div>;
}
