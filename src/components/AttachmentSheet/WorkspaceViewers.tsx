import { useCallback, useEffect, useRef, useState } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import TipTapLink from "@tiptap/extension-link";
import { AlertCircle, ExternalLink, Loader2, RefreshCw, Settings2 } from "lucide-react";
import { Link as RouterLink } from "react-router-dom";
import {
  driveHasEditorScope,
  driveStatus,
  getGoogleDoc,
  getGoogleSheet,
  getGoogleSlides,
  getSlideThumbnail,
} from "../../lib/files";
import { gdocsToTipTap, type GDocsDocument } from "../../lib/docsConverter";

// ──────────────────────────────────────────────────────────────────────────
// Common helpers
// ──────────────────────────────────────────────────────────────────────────

function ViewerLoader({ label }: { label: string }) {
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-white">
      <Loader2 className="animate-spin text-zinc-400" size={20} />
      <p className="text-xs text-zinc-500">{label}</p>
    </div>
  );
}

function ViewerError({
  error,
  onOpenExternal,
  onRetry,
}: {
  error: string;
  onOpenExternal: () => void;
  onRetry: () => void;
}) {
  const lower = error.toLowerCase();
  const isAuth =
    lower.includes("401") ||
    lower.includes("403") ||
    lower.includes("permission") ||
    lower.includes("unauthorized") ||
    lower.includes("forbidden");
  const isNotFound = lower.includes("404") || lower.includes("notfound");
  // Default-expanded for auth errors so the user immediately sees the HTTP
  // status code — that's the fastest way to diagnose the actual failure mode.
  const [showDetails, setShowDetails] = useState(isAuth || isNotFound);
  const [connectedEmail, setConnectedEmail] = useState<string | null>(null);

  // Look up the connected Drive account email so we can show "Trace is
  // connected as X — is X the account that can see this file?" That single
  // piece of information explains 90% of auth failures.
  useEffect(() => {
    if (!isAuth) return;
    driveStatus()
      .then((s) => {
        const email = s.accounts[0]?.email ?? null;
        setConnectedEmail(email);
      })
      .catch(() => setConnectedEmail(null));
  }, [isAuth]);

  let title = "Couldn't load preview";
  let body =
    "Something went wrong while loading this file. Retry, or open it in your browser.";
  if (isAuth) {
    title = "Trace can't read this file";
    body =
      "Google rejected the request. Either Trace is connected to a different Google account than the one that has access to this file, or the stored OAuth token doesn't have the right scope yet.";
  } else if (isNotFound) {
    title = "File not found";
    body =
      "Google says this file doesn't exist or has been removed. If it works in your browser, the URL may be a redirect — try Open in browser.";
  }

  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 overflow-auto bg-white px-6 py-8 text-center">
      <AlertCircle className={isAuth ? "text-amber-500" : "text-rose-500"} size={28} />
      <div className="max-w-md">
        <p className="text-sm font-semibold text-zinc-800">{title}</p>
        <p className="mt-1 text-[12px] leading-5 text-zinc-500">{body}</p>
        {isAuth && connectedEmail ? (
          <p className="mt-3 text-[11px] text-zinc-600">
            Trace is connected as{" "}
            <span className="font-semibold text-zinc-900">{connectedEmail}</span>
            . Is this the account that can open the file in your browser?
          </p>
        ) : null}
      </div>
      <div className="flex flex-wrap items-center justify-center gap-2">
        <button
          className="flex items-center gap-1.5 rounded-md border border-zinc-200 px-3 py-1.5 text-[12px] font-medium text-zinc-700 hover:bg-zinc-50"
          onClick={onRetry}
          type="button"
        >
          <RefreshCw size={12} /> Retry
        </button>
        {isAuth ? (
          <RouterLink
            className="flex items-center gap-1.5 rounded-md border border-zinc-200 px-3 py-1.5 text-[12px] font-medium text-zinc-700 hover:bg-zinc-50"
            to="/files"
          >
            <Settings2 size={12} /> Drive settings
          </RouterLink>
        ) : null}
        <button
          className="flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-[12px] font-semibold text-white hover:bg-sky-700"
          onClick={onOpenExternal}
          type="button"
        >
          <ExternalLink size={12} /> Open in browser
        </button>
      </div>
      <button
        className="text-[11px] text-zinc-400 hover:text-zinc-700"
        onClick={() => setShowDetails((v) => !v)}
        type="button"
      >
        {showDetails ? "Hide details" : "Show technical details"}
      </button>
      {showDetails ? (
        <pre className="mx-6 max-h-40 max-w-2xl overflow-auto whitespace-pre-wrap break-all rounded-md border border-zinc-100 bg-zinc-50 px-3 py-2 text-left text-[11px] text-zinc-600">
          {error}
        </pre>
      ) : null}
    </div>
  );
}

function NeedsReauth({ onOpenExternal }: { onOpenExternal: () => void }) {
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-white px-6 text-center">
      <AlertCircle className="text-amber-500" size={28} />
      <div className="max-w-md">
        <p className="text-sm font-semibold text-zinc-800">
          Drive permission needs refreshing
        </p>
        <p className="mt-1 text-[12px] leading-5 text-zinc-500">
          Trace's stored Drive token doesn't include the scope needed to read
          this file. Reconnect Google Drive in Settings → Files, or open this
          one in your browser.
        </p>
      </div>
      <button
        className="flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-[12px] font-semibold text-white hover:bg-sky-700"
        onClick={onOpenExternal}
        type="button"
      >
        <ExternalLink size={12} /> Open in browser
      </button>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Google Doc viewer (read-only TipTap)
// ──────────────────────────────────────────────────────────────────────────

export function GoogleDocViewer({
  fileId,
  onOpenExternal,
}: {
  fileId: string;
  onOpenExternal: () => void;
}) {
  const [status, setStatus] = useState<"loading" | "ready" | "error" | "noscope">(
    "loading",
  );
  const [error, setError] = useState<string>("");
  const [attempt, setAttempt] = useState(0);

  const editor = useEditor(
    {
      editable: false,
      extensions: [StarterKit, Underline, TipTapLink.configure({ openOnClick: true })],
      content: "",
      editorProps: {
        attributes: {
          class:
            "prose prose-sm max-w-3xl mx-auto focus:outline-none px-8 py-8 text-zinc-800 leading-7",
        },
      },
    },
    [],
  );

  const load = useCallback(async () => {
    if (!editor) return;
    setStatus("loading");
    setError("");
    try {
      const hasScope = await driveHasEditorScope();
      if (!hasScope) {
        setStatus("noscope");
        return;
      }
      const doc = await getGoogleDoc(fileId);
      editor.commands.setContent(gdocsToTipTap(doc as GDocsDocument), {
        emitUpdate: false,
      });
      setStatus("ready");
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  }, [editor, fileId]);

  useEffect(() => {
    void load();
  }, [load, attempt]);

  return (
    <div className="relative h-full w-full overflow-y-auto bg-white">
      <EditorContent editor={editor} />
      {status === "loading" ? <ViewerLoader label="Loading Doc…" /> : null}
      {status === "noscope" ? (
        <NeedsReauth onOpenExternal={onOpenExternal} />
      ) : null}
      {status === "error" ? (
        <ViewerError
          error={error}
          onOpenExternal={onOpenExternal}
          onRetry={() => setAttempt((a) => a + 1)}
        />
      ) : null}
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Google Slides viewer (thumbnail strip)
// ──────────────────────────────────────────────────────────────────────────

interface SlidesShape {
  title?: string;
  slides?: { objectId?: string }[];
}

export function GoogleSlidesViewer({
  fileId,
  onOpenExternal,
}: {
  fileId: string;
  onOpenExternal: () => void;
}) {
  const [status, setStatus] = useState<"loading" | "ready" | "error" | "noscope">(
    "loading",
  );
  const [error, setError] = useState<string>("");
  const [title, setTitle] = useState<string>("");
  const [slideIds, setSlideIds] = useState<string[]>([]);
  const [thumbnails, setThumbnails] = useState<Record<string, string>>({});
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setStatus("loading");
      setError("");
      setThumbnails({});
      try {
        const hasScope = await driveHasEditorScope();
        if (!hasScope) {
          if (!cancelled) setStatus("noscope");
          return;
        }
        const data = (await getGoogleSlides(fileId)) as SlidesShape;
        if (cancelled) return;
        const ids = (data.slides ?? [])
          .map((s) => s.objectId)
          .filter((id): id is string => !!id);
        setTitle(data.title ?? "");
        setSlideIds(ids);
        setStatus("ready");
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setStatus("error");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [fileId, attempt]);

  // Fetch thumbnails lazily — kick off all in parallel once we have slide ids.
  useEffect(() => {
    if (status !== "ready" || slideIds.length === 0) return;
    let cancelled = false;
    (async () => {
      const results = await Promise.allSettled(
        slideIds.map((id) => getSlideThumbnail(fileId, id)),
      );
      if (cancelled) return;
      const map: Record<string, string> = {};
      results.forEach((r, i) => {
        if (r.status === "fulfilled") map[slideIds[i]] = r.value;
      });
      setThumbnails(map);
    })();
    return () => {
      cancelled = true;
    };
  }, [fileId, slideIds, status]);

  return (
    <div className="relative h-full w-full overflow-y-auto bg-zinc-50">
      {status === "ready" ? (
        <div className="mx-auto max-w-5xl px-6 py-8">
          {title ? (
            <p className="mb-4 text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-400">
              {title} · {slideIds.length} slide{slideIds.length === 1 ? "" : "s"}
            </p>
          ) : null}
          <div className="space-y-4">
            {slideIds.map((id, idx) => (
              <figure
                className="overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
                key={id}
              >
                <div className="relative aspect-[16/9] bg-zinc-100">
                  {thumbnails[id] ? (
                    <img
                      alt={`Slide ${idx + 1}`}
                      className="h-full w-full object-contain"
                      src={thumbnails[id]}
                    />
                  ) : (
                    <div className="absolute inset-0 flex items-center justify-center">
                      <Loader2 className="animate-spin text-zinc-300" size={18} />
                    </div>
                  )}
                </div>
                <figcaption className="border-t border-zinc-100 px-4 py-2 text-[11px] text-zinc-400">
                  Slide {idx + 1}
                </figcaption>
              </figure>
            ))}
          </div>
        </div>
      ) : null}
      {status === "loading" ? <ViewerLoader label="Loading Slides…" /> : null}
      {status === "noscope" ? (
        <NeedsReauth onOpenExternal={onOpenExternal} />
      ) : null}
      {status === "error" ? (
        <ViewerError
          error={error}
          onOpenExternal={onOpenExternal}
          onRetry={() => setAttempt((a) => a + 1)}
        />
      ) : null}
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Google Sheets viewer (basic grid)
// ──────────────────────────────────────────────────────────────────────────

interface SheetsShape {
  properties?: { title?: string };
  sheets?: Array<{
    properties?: { title?: string };
    data?: Array<{
      rowData?: Array<{
        values?: Array<{
          formattedValue?: string;
          effectiveFormat?: {
            textFormat?: { bold?: boolean; italic?: boolean };
            backgroundColor?: { red?: number; green?: number; blue?: number };
          };
        }>;
      }>;
    }>;
  }>;
}

function colLetter(idx: number): string {
  // 0-indexed → A, B, …, Z, AA, AB, …
  let n = idx;
  let s = "";
  while (true) {
    s = String.fromCharCode(65 + (n % 26)) + s;
    n = Math.floor(n / 26) - 1;
    if (n < 0) break;
  }
  return s;
}

export function GoogleSheetViewer({
  fileId,
  onOpenExternal,
}: {
  fileId: string;
  onOpenExternal: () => void;
}) {
  const [status, setStatus] = useState<"loading" | "ready" | "error" | "noscope">(
    "loading",
  );
  const [error, setError] = useState<string>("");
  const [title, setTitle] = useState<string>("");
  const [sheetTitle, setSheetTitle] = useState<string>("");
  const [rows, setRows] = useState<string[][]>([]);
  const [boldMask, setBoldMask] = useState<boolean[][]>([]);
  const [attempt, setAttempt] = useState(0);
  const scrollerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setStatus("loading");
      setError("");
      try {
        const hasScope = await driveHasEditorScope();
        if (!hasScope) {
          if (!cancelled) setStatus("noscope");
          return;
        }
        const data = (await getGoogleSheet(fileId)) as SheetsShape;
        if (cancelled) return;
        const sheet = data.sheets?.[0];
        const rowData = sheet?.data?.[0]?.rowData ?? [];
        const grid: string[][] = [];
        const bold: boolean[][] = [];
        let maxCols = 0;
        for (const r of rowData) {
          const cells = r.values ?? [];
          const rowVals: string[] = [];
          const rowBold: boolean[] = [];
          for (const c of cells) {
            rowVals.push(c.formattedValue ?? "");
            rowBold.push(c.effectiveFormat?.textFormat?.bold ?? false);
          }
          grid.push(rowVals);
          bold.push(rowBold);
          if (rowVals.length > maxCols) maxCols = rowVals.length;
        }
        // Normalize row widths.
        for (const r of grid) while (r.length < maxCols) r.push("");
        for (const r of bold) while (r.length < maxCols) r.push(false);
        setTitle(data.properties?.title ?? "");
        setSheetTitle(sheet?.properties?.title ?? "");
        setRows(grid);
        setBoldMask(bold);
        setStatus("ready");
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setStatus("error");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [fileId, attempt]);

  const cols = rows[0]?.length ?? 0;

  return (
    <div className="relative h-full w-full overflow-hidden bg-white">
      {status === "ready" ? (
        <div className="flex h-full flex-col">
          <div className="flex items-baseline justify-between border-b border-zinc-100 px-5 py-2">
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-emerald-700">
              {title || "Spreadsheet"}
            </p>
            {sheetTitle ? (
              <p className="text-[11px] text-zinc-400">{sheetTitle}</p>
            ) : null}
          </div>
          <div
            className="min-h-0 flex-1 overflow-auto bg-zinc-50"
            ref={scrollerRef}
          >
            <table className="w-max border-collapse text-[12px]">
              <thead className="sticky top-0 z-10 bg-zinc-100">
                <tr>
                  <th className="sticky left-0 z-20 w-10 border-b border-r border-zinc-200 bg-zinc-100 px-2 py-1 text-[10px] font-semibold text-zinc-400" />
                  {Array.from({ length: cols }).map((_, c) => (
                    <th
                      className="min-w-[110px] border-b border-r border-zinc-200 px-2 py-1 text-left text-[10px] font-semibold uppercase tracking-wider text-zinc-400"
                      key={c}
                    >
                      {colLetter(c)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row, r) => (
                  <tr className="odd:bg-white even:bg-zinc-50/60" key={r}>
                    <th className="sticky left-0 z-10 w-10 border-b border-r border-zinc-100 bg-zinc-100 px-2 py-1 text-right text-[10px] font-semibold text-zinc-400">
                      {r + 1}
                    </th>
                    {row.map((cell, c) => (
                      <td
                        className={`max-w-[260px] truncate border-b border-r border-zinc-100 px-2 py-1 text-zinc-700 ${
                          boldMask[r]?.[c] ? "font-semibold text-zinc-900" : ""
                        }`}
                        key={c}
                        title={cell}
                      >
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
            {rows.length === 0 ? (
              <p className="p-8 text-center text-sm text-zinc-400">
                Sheet is empty (or first sheet has no data in A1:Z200).
              </p>
            ) : null}
            {rows.length >= 200 ? (
              <p className="px-5 py-3 text-[11px] text-zinc-400">
                Showing first 200 rows × 26 columns. Open in browser for the
                full sheet.
              </p>
            ) : null}
          </div>
        </div>
      ) : null}
      {status === "loading" ? <ViewerLoader label="Loading Sheet…" /> : null}
      {status === "noscope" ? (
        <NeedsReauth onOpenExternal={onOpenExternal} />
      ) : null}
      {status === "error" ? (
        <ViewerError
          error={error}
          onOpenExternal={onOpenExternal}
          onRetry={() => setAttempt((a) => a + 1)}
        />
      ) : null}
    </div>
  );
}
