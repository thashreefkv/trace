import { useCallback, useEffect, useRef, useState } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Link from "@tiptap/extension-link";
import { AlertCircle, CheckCircle2, Loader2, RefreshCw, Save } from "lucide-react";
import { driveHasEditorScope, getGoogleDoc, saveGoogleDoc } from "../../lib/files";
import { gdocsToTipTap, tipTapToRequests, type GDocsDocument } from "../../lib/docsConverter";

interface GoogleDocEditorProps {
  driveFileId: string;
  fileName: string;
}

export function GoogleDocEditor({ driveFileId }: GoogleDocEditorProps) {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveState, setSaveState] = useState<"idle" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const [needsReauth, setNeedsReauth] = useState(false);
  const bodyEndIndexRef = useRef<number>(1);

  const editor = useEditor({
    extensions: [StarterKit, Underline, Link.configure({ openOnClick: false })],
    content: "",
    editorProps: {
      attributes: {
        class: "prose prose-sm max-w-none focus:outline-none min-h-[300px] px-6 py-4",
      },
    },
  });

  const computeBodyEndIndex = (doc: GDocsDocument): number => {
    let total = 1;
    for (const el of doc.body.content as Array<{ paragraph?: { elements?: Array<{ textRun?: { content?: string } }> } }>) {
      if (el.paragraph) {
        for (const elem of el.paragraph.elements ?? []) {
          total += elem.textRun?.content?.length ?? 0;
        }
      }
    }
    return total;
  };

  const loadDoc = useCallback(async () => {
    if (!editor) return;
    try {
      setLoading(true);
      setError(null);

      const hasScope = await driveHasEditorScope();
      if (!hasScope) {
        setNeedsReauth(true);
        return;
      }

      const doc = await getGoogleDoc(driveFileId);
      bodyEndIndexRef.current = computeBodyEndIndex(doc);
      editor.commands.setContent(gdocsToTipTap(doc as GDocsDocument));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [driveFileId, editor]);

  useEffect(() => {
    void loadDoc();
  }, [loadDoc]);

  const handleSave = useCallback(async () => {
    if (!editor) return;
    setSaving(true);
    setSaveState("idle");
    try {
      const tiptapDoc = editor.getJSON() as Parameters<typeof tipTapToRequests>[0];
      const insertRequests = tipTapToRequests(tiptapDoc);

      const deleteRequest =
        bodyEndIndexRef.current > 2
          ? [{ deleteContentRange: { range: { startIndex: 1, endIndex: bodyEndIndexRef.current - 1 } } }]
          : [];

      await saveGoogleDoc(driveFileId, [...deleteRequest, ...insertRequests]);
      setSaveState("saved");

      // Re-fetch to get updated body length for the next save.
      const updated = await getGoogleDoc(driveFileId);
      bodyEndIndexRef.current = computeBodyEndIndex(updated);
      setTimeout(() => setSaveState("idle"), 3000);
    } catch (e) {
      setError(String(e));
      setSaveState("error");
    } finally {
      setSaving(false);
    }
  }, [editor, driveFileId]);

  // Cmd+S / Ctrl+S
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        void handleSave();
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [handleSave]);

  if (needsReauth) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <AlertCircle className="text-amber-500" size={28} />
        <p className="text-[13px] font-medium text-zinc-800">Editing requires additional permissions</p>
        <p className="max-w-[260px] text-[12px] text-zinc-500">
          Re-connect Google Drive to enable in-app editing of Docs and Slides.
        </p>
        <p className="text-[11px] text-zinc-400">
          Go to the Drive tab and disconnect, then reconnect your account.
        </p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <AlertCircle className="text-rose-500" size={24} />
        <p className="text-[13px] text-rose-700">{error}</p>
        <button
          className="btn"
          onClick={() => { setError(null); void loadDoc(); }}
          type="button"
        >
          <RefreshCw size={13} />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar */}
      <div className="flex shrink-0 items-center justify-between border-b border-zinc-200 bg-white px-3 py-1.5">
        <div className="flex items-center gap-0.5">
          {[
            { label: "B", mark: "bold", style: "font-bold" },
            { label: "I", mark: "italic", style: "italic" },
            { label: "U", mark: "underline", style: "underline" },
          ].map(({ label, mark, style }) => (
            <button
              key={mark}
              className={[
                "h-6 w-6 rounded text-[12px] hover:bg-zinc-100",
                style,
                editor?.isActive(mark) ? "bg-zinc-200" : "",
              ].join(" ")}
              onMouseDown={(e) => {
                e.preventDefault();
                editor?.chain().focus().toggleMark(mark as "bold" | "italic" | "underline").run();
              }}
              title={mark}
              type="button"
            >
              {label}
            </button>
          ))}
          <div className="mx-1 h-3.5 w-px bg-zinc-200" />
          {([1, 2, 3] as const).map((level) => (
            <button
              key={level}
              className={[
                "h-6 rounded px-1.5 text-[10px] font-semibold hover:bg-zinc-100",
                editor?.isActive("heading", { level }) ? "bg-zinc-200" : "",
              ].join(" ")}
              onMouseDown={(e) => {
                e.preventDefault();
                editor?.chain().focus().toggleHeading({ level }).run();
              }}
              title={`Heading ${level}`}
              type="button"
            >
              H{level}
            </button>
          ))}
          <div className="mx-1 h-3.5 w-px bg-zinc-200" />
          <button
            className={[
              "h-6 rounded px-1.5 text-[10px] hover:bg-zinc-100",
              editor?.isActive("bulletList") ? "bg-zinc-200" : "",
            ].join(" ")}
            onMouseDown={(e) => {
              e.preventDefault();
              editor?.chain().focus().toggleBulletList().run();
            }}
            title="Bullet list"
            type="button"
          >
            • List
          </button>
        </div>

        <div className="flex items-center gap-2">
          {saveState === "saved" && (
            <span className="flex items-center gap-1 text-[11px] text-emerald-600">
              <CheckCircle2 size={11} /> Saved
            </span>
          )}
          {saveState === "error" && (
            <span className="flex items-center gap-1 text-[11px] text-rose-600">
              <AlertCircle size={11} /> Failed
            </span>
          )}
          <button
            className="btn btn-primary flex items-center gap-1"
            disabled={saving || loading}
            onClick={() => void handleSave()}
            type="button"
          >
            {saving ? <Loader2 className="animate-spin" size={11} /> : <Save size={11} />}
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>

      {/* Editor body */}
      <div className="min-h-0 flex-1 overflow-y-auto bg-white">
        {loading ? (
          <div className="space-y-3 p-6">
            <div className="skeleton h-7 w-1/3" />
            <div className="skeleton h-4 w-full" />
            <div className="skeleton h-4 w-5/6" />
            <div className="skeleton h-4 w-full" />
            <div className="skeleton h-4 w-4/5" />
            <div className="mt-4 skeleton h-4 w-full" />
            <div className="skeleton h-4 w-3/4" />
          </div>
        ) : (
          <EditorContent editor={editor} />
        )}
      </div>
    </div>
  );
}
