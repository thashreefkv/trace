import { useEffect, useMemo, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Copy,
  ExternalLink,
  FileText,
  Loader2,
  X,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  GoogleDocViewer,
  GoogleSheetViewer,
  GoogleSlidesViewer,
} from "./WorkspaceViewers";
import { toast } from "../../lib/toast";
import { hostnameMatches, safeExternalUrl } from "../../lib/urlSafety";

interface Props {
  open: boolean;
  url: string | null;
  onClose: () => void;
}

/**
 * Bottom-sheet preview of an attached / linked file. Tries to embed the
 * source via its native preview/embed URL; falls back to a clean "open in
 * browser" affordance if the host blocks framing.
 */
export function AttachmentSheet({ onClose, open, url }: Props) {
  const meta = useMemo(() => (url ? getEmbedMeta(url) : null), [url]);

  const [loadState, setLoadState] = useState<"loading" | "loaded" | "blocked">(
    "loading",
  );

  // Reset load state every time the URL changes / sheet re-opens.
  useEffect(() => {
    if (!open || !url) return;
    setLoadState("loading");
    // If the iframe hasn't reported "loaded" within 6s, assume the host
    // blocked embedding (X-Frame-Options) and surface the fallback.
    const timer = window.setTimeout(() => {
      setLoadState((s) => (s === "loading" ? "blocked" : s));
    }, 6000);
    return () => window.clearTimeout(timer);
  }, [open, url]);

  // ESC closes.
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  async function copyUrl() {
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
    } catch {
      // ignore
    }
  }

  function openExternally() {
    if (!url) return;
    const safeUrl = safeExternalUrl(url);
    if (!safeUrl) {
      toast.error("Only valid HTTPS links can be opened.");
      return;
    }
    openUrl(safeUrl).catch((error) => {
      toast.error(`Couldn't open browser: ${error}`);
    });
  }

  return (
    <AnimatePresence>
      {open && url ? (
        <motion.div
          animate={{ opacity: 1 }}
          className="fixed inset-0 z-40 flex items-end justify-center bg-black/30 backdrop-blur-sm"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          onMouseDown={onClose}
          transition={{ duration: 0.18, ease: "easeOut" }}
        >
          <motion.section
            animate={{ y: 0 }}
            className="flex h-[90vh] w-full max-w-6xl flex-col rounded-t-2xl border border-zinc-100 bg-white shadow-[0_-12px_40px_rgba(0,0,0,0.18)]"
            exit={{ y: "100%" }}
            initial={{ y: "100%" }}
            onMouseDown={(event) => event.stopPropagation()}
            transition={{ type: "spring", stiffness: 380, damping: 36 }}
          >
            {/* Header */}
            <header className="flex shrink-0 items-center justify-between gap-3 border-b border-zinc-100 px-5 py-3">
              <div className="flex min-w-0 items-center gap-3">
                <span
                  className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-md ${meta?.iconBg ?? "bg-zinc-100"} ${meta?.iconText ?? "text-zinc-600"}`}
                >
                  <FileText size={14} />
                </span>
                <div className="min-w-0">
                  <p className={`text-[11px] font-bold uppercase tracking-[0.2em] ${meta?.iconText ?? "text-zinc-500"}`}>
                    {meta?.label ?? "Linked file"}
                  </p>
                  <p className="truncate text-sm text-zinc-700" title={url}>
                    {meta?.displayTitle ?? url}
                  </p>
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <button
                  aria-label="Copy link"
                  className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12px] font-medium text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
                  onClick={() => void copyUrl()}
                  title="Copy URL"
                  type="button"
                >
                  <Copy size={13} />
                  Copy link
                </button>
                <button
                  className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12px] font-medium text-sky-700 hover:bg-sky-50"
                  onClick={openExternally}
                  title="Open in browser"
                  type="button"
                >
                  <ExternalLink size={13} />
                  Open in browser
                </button>
                <button
                  aria-label="Close"
                  className="rounded-md p-1.5 text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
                  onClick={onClose}
                  type="button"
                >
                  <X size={16} />
                </button>
              </div>
            </header>

            {/* Body */}
            <div className="relative flex-1 overflow-hidden bg-zinc-50">
              {meta?.workspace ? (
                meta.workspace.kind === "doc" ? (
                  <GoogleDocViewer
                    fileId={meta.workspace.fileId}
                    onOpenExternal={openExternally}
                  />
                ) : meta.workspace.kind === "slides" ? (
                  <GoogleSlidesViewer
                    fileId={meta.workspace.fileId}
                    onOpenExternal={openExternally}
                  />
                ) : (
                  <GoogleSheetViewer
                    fileId={meta.workspace.fileId}
                    onOpenExternal={openExternally}
                  />
                )
              ) : meta?.embedUrl ? (
                <>
                  {loadState === "loading" ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center gap-2">
                      <Loader2 className="animate-spin text-zinc-400" size={20} />
                      <p className="text-xs text-zinc-500">Loading preview…</p>
                    </div>
                  ) : null}
                  <iframe
                    allow="autoplay; encrypted-media; picture-in-picture"
                    className={`h-full w-full border-0 bg-white transition-opacity ${
                      loadState === "loaded" ? "opacity-100" : "opacity-0"
                    }`}
                    onLoad={() => setLoadState("loaded")}
                    referrerPolicy="no-referrer"
                    sandbox="allow-scripts allow-same-origin allow-popups"
                    src={meta.embedUrl}
                    title={meta.label}
                  />
                  {loadState === "blocked" ? (
                    <BlockedFallback
                      meta={meta}
                      onOpenExternal={openExternally}
                      url={url}
                    />
                  ) : null}
                </>
              ) : (
                <BlockedFallback meta={meta} onOpenExternal={openExternally} url={url} />
              )}
            </div>
          </motion.section>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

function BlockedFallback({
  meta,
  onOpenExternal,
  url,
}: {
  meta: EmbedMeta | null;
  onOpenExternal: () => void;
  url: string;
}) {
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-white px-6 text-center">
      <span
        className={`flex h-14 w-14 items-center justify-center rounded-2xl ${meta?.iconBg ?? "bg-zinc-100"} ${meta?.iconText ?? "text-zinc-600"}`}
      >
        <FileText size={24} />
      </span>
      <div>
        <p className="text-sm font-semibold text-zinc-800">
          {meta?.label ?? "Linked file"}
        </p>
        <p className="mt-1 max-w-md break-words text-[12px] text-zinc-500">
          {url}
        </p>
      </div>
      <p className="max-w-md text-[12px] leading-5 text-zinc-500">
        {meta?.fallbackReason ??
          "This source doesn't allow embedding. Open it in your browser to view."}
      </p>
      <button
        className="flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-sm font-semibold text-white hover:bg-sky-700"
        onClick={onOpenExternal}
        type="button"
      >
        <ExternalLink size={14} />
        Open in browser
      </button>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// URL → embed transformer
// ──────────────────────────────────────────────────────────────────────────

interface EmbedMeta {
  label: string;
  displayTitle: string;
  /** null → no embeddable variant; surface "open externally" fallback. */
  embedUrl: string | null;
  iconBg: string;
  iconText: string;
  /** Optional reason shown in the fallback panel when embedUrl is null. */
  fallbackReason?: string;
  /**
   * When set, the sheet renders a native API-backed viewer using Trace's
   * stored OAuth instead of iframe-embedding. This is how Google Workspace
   * files get a real preview without the webview-cookie auth wall.
   */
  workspace?:
    | { kind: "doc" | "slides" | "sheet"; fileId: string };
}

const GOOGLE_DOC_ID = /\/document\/d\/([a-zA-Z0-9_-]+)/;
const GOOGLE_SHEET_ID = /\/spreadsheets\/d\/([a-zA-Z0-9_-]+)/;
const GOOGLE_SLIDES_ID = /\/presentation\/d\/([a-zA-Z0-9_-]+)/;

export function getEmbedMeta(url: string): EmbedMeta {
  const safeUrl = safeExternalUrl(url);
  if (!safeUrl) {
    return {
      label: "Invalid link",
      displayTitle: url,
      embedUrl: null,
      iconBg: "bg-rose-100",
      iconText: "text-rose-700",
      fallbackReason: "Only valid HTTPS links can be opened.",
    };
  }
  const parsed = new URL(safeUrl);
  const host = parsed.hostname.replace(/^www\./, "");
  const path = parsed.pathname;

  // Google Workspace — Docs, Sheets, Slides. Rendered natively via Trace's
  // existing Drive/Docs/Sheets/Slides OAuth, bypassing the webview-cookie
  // auth wall entirely.
  const docMatch = path.match(GOOGLE_DOC_ID);
  if (host === "docs.google.com" && path.startsWith("/document/") && docMatch?.[1]) {
    return {
      label: "Google Doc",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-sky-100",
      iconText: "text-sky-700",
      workspace: { kind: "doc", fileId: docMatch[1] },
    };
  }
  const slidesMatch = path.match(GOOGLE_SLIDES_ID);
  if (host === "docs.google.com" && path.startsWith("/presentation/") && slidesMatch?.[1]) {
    return {
      label: "Google Slides",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-amber-100",
      iconText: "text-amber-700",
      workspace: { kind: "slides", fileId: slidesMatch[1] },
    };
  }
  const sheetMatch = path.match(GOOGLE_SHEET_ID);
  if (host === "docs.google.com" && path.startsWith("/spreadsheets/") && sheetMatch?.[1]) {
    return {
      label: "Google Sheet",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-emerald-100",
      iconText: "text-emerald-700",
      workspace: { kind: "sheet", fileId: sheetMatch[1] },
    };
  }
  // Drive files (non-Docs/Sheets/Slides) — no API renderer yet, fall back.
  if (host === "drive.google.com" && path.startsWith("/file/")) {
    return {
      label: "Google Drive",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-zinc-100",
      iconText: "text-zinc-700",
      fallbackReason:
        "Trace doesn't preview generic Drive files inline yet. Open in browser to view.",
    };
  }

  // Figma — official embed endpoint
  if (hostnameMatches(host, "figma.com") && /^\/(file|design|proto)\//.test(path)) {
    return {
      label: "Figma file",
      displayTitle: `${host}${path}`,
      embedUrl: `https://www.figma.com/embed?embed_host=share&url=${encodeURIComponent(safeUrl)}`,
      iconBg: "bg-violet-100",
      iconText: "text-violet-700",
    };
  }

  // Loom — share → embed
  if (hostnameMatches(host, "loom.com") && path.startsWith("/share/")) {
    return {
      label: "Loom video",
      displayTitle: `${host}${path}`,
      embedUrl: safeUrl.replace("/share/", "/embed/"),
      iconBg: "bg-violet-100",
      iconText: "text-violet-700",
    };
  }

  // YouTube — common video case (just in case it slips in via shared links)
  if (hostnameMatches(host, "youtube.com") && path === "/watch") {
    const videoId = parsed.searchParams.get("v");
    if (videoId && /^[a-zA-Z0-9_-]{6,20}$/.test(videoId)) {
      return {
        label: "YouTube video",
        displayTitle: `${host}${path}`,
        embedUrl: `https://www.youtube-nocookie.com/embed/${videoId}`,
        iconBg: "bg-rose-100",
        iconText: "text-rose-700",
      };
    }
  }
  if (host === "youtu.be") {
    const idMatch = path.match(/\/([^/?]+)/);
    if (idMatch?.[1] && /^[a-zA-Z0-9_-]{6,20}$/.test(idMatch[1])) {
      return {
        label: "YouTube video",
        displayTitle: `${host}${path}`,
        embedUrl: `https://www.youtube-nocookie.com/embed/${idMatch[1]}`,
        iconBg: "bg-rose-100",
        iconText: "text-rose-700",
      };
    }
  }

  // Arbitrary PDFs are not embedded because an untrusted origin would execute
  // inside the application webview. They can still be opened in the browser.
  if (path.toLowerCase().endsWith(".pdf")) {
    return {
      label: "PDF",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-rose-100",
      iconText: "text-rose-700",
    };
  }

  // Notion blocks framing. GitHub blocks framing. Surface fallback.
  if (hostnameMatches(host, "notion.so") || hostnameMatches(host, "notion.site")) {
    return {
      label: "Notion page",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-zinc-900",
      iconText: "text-white",
    };
  }
  if (hostnameMatches(host, "github.com")) {
    return {
      label: "GitHub",
      displayTitle: `${host}${path}`,
      embedUrl: null,
      iconBg: "bg-zinc-900",
      iconText: "text-white",
    };
  }

  // Generic sites are opened in the user's browser, never inside the app.
  return {
    label: "Linked file",
    displayTitle: host ? `${host}${path}` : url,
    embedUrl: null,
    iconBg: "bg-zinc-100",
    iconText: "text-zinc-600",
    fallbackReason: "For your security, Trace only embeds trusted preview providers.",
  };
}
