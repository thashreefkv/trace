import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Check,
  ChevronDown,
  Loader2,
  RotateCcw,
  Save,
  Search,
  Send,
  Sparkles,
  UsersRound,
  X,
} from "lucide-react";
import { RichTextEditor, type RichTextEditorHandle } from "./RichTextEditor";
import { AttachmentArea } from "./AttachmentArea";
import {
  gmailAddDraftAttachment,
  gmailDeleteLocalDraft,
  gmailDraftReplyWithBrain,
  gmailGetLocalDraft,
  gmailRemoveDraftAttachment,
  gmailSaveLocalDraft,
  gmailSendEmail,
  listStakeholders,
} from "../../lib/ipc";
import type { LocalEmailDraft, Stakeholder } from "../../lib/types";
import { toast } from "../../lib/toast";

interface Props {
  open: boolean;
  threadId: string | null;
  /** Default To recipients to seed a new draft (one per address). */
  defaultTo: string[];
  defaultSubject: string;
  onClose: () => void;
  onSent: () => void;
}

const AUTOSAVE_DEBOUNCE_MS = 1500;

export function ReplyComposer({
  open,
  threadId,
  defaultTo,
  defaultSubject,
  onClose,
  onSent,
}: Props) {
  const [draft, setDraft] = useState<LocalEmailDraft | null>(null);
  const [to, setTo] = useState<string[]>([]);
  const [cc, setCc] = useState<string[]>([]);
  const [bcc, setBcc] = useState<string[]>([]);
  const [subject, setSubject] = useState("");
  const [bodyHtml, setBodyHtml] = useState("");
  const [bodyText, setBodyText] = useState("");
  const [showCc, setShowCc] = useState(false);
  const [showBcc, setShowBcc] = useState(false);
  const [loading, setLoading] = useState(false);
  const [savingState, setSavingState] = useState<"idle" | "saving" | "saved">(
    "idle",
  );
  const [lastSavedAt, setLastSavedAt] = useState<number | null>(null);
  const [aiDrafting, setAiDrafting] = useState(false);
  const [sending, setSending] = useState(false);
  const [confirmFresh, setConfirmFresh] = useState(false);
  const [stakeholders, setStakeholders] = useState<Stakeholder[]>([]);

  const editorRef = useRef<RichTextEditorHandle>(null);
  const draftRef = useRef<LocalEmailDraft | null>(null);
  const autosaveTimerRef = useRef<number | null>(null);
  const skipNextAutosaveRef = useRef(false);

  // Keep ref in sync.
  useEffect(() => {
    draftRef.current = draft;
  }, [draft]);

  // Load stakeholders once for the recipient picker.
  useEffect(() => {
    if (!open) return;
    listStakeholders()
      .then((list) => setStakeholders(list.filter((s) => s.email)))
      .catch(() => {
        // Silent — picker just won't have suggestions.
      });
  }, [open]);

  // ---- Load on open --------------------------------------------------------
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setConfirmFresh(false);
    (async () => {
      try {
        let existing: LocalEmailDraft | null = null;
        if (threadId) {
          existing = await gmailGetLocalDraft(threadId);
        }
        if (cancelled) return;
        if (existing) {
          // Skip the first autosave that would otherwise fire from setState.
          skipNextAutosaveRef.current = true;
          setDraft(existing);
          setTo(existing.to);
          setCc(existing.cc);
          setBcc(existing.bcc);
          setSubject(existing.subject);
          setBodyHtml(existing.body_html);
          setBodyText(existing.body_text);
          setShowCc(existing.cc.length > 0);
          setShowBcc(existing.bcc.length > 0);
        } else {
          // Seed an empty draft from the thread context.
          skipNextAutosaveRef.current = true;
          setDraft(null);
          setTo(defaultTo);
          setCc([]);
          setBcc([]);
          setSubject(defaultSubject);
          setBodyHtml("");
          setBodyText("");
          setShowCc(false);
          setShowBcc(false);
        }
        setSavingState("idle");
        setLastSavedAt(null);
      } catch (error) {
        toast.error(`Failed to load draft: ${error}`);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, threadId, defaultTo, defaultSubject]);

  // ---- Auto-save (debounced) ----------------------------------------------
  const persistDraft = useCallback(
    async (silent: boolean): Promise<LocalEmailDraft | null> => {
      try {
        if (!silent) setSavingState("saving");
        const saved = await gmailSaveLocalDraft({
          id: draftRef.current?.id ?? null,
          thread_id: threadId,
          to,
          cc,
          bcc,
          subject,
          body_html: bodyHtml,
          body_text: bodyText,
        });
        setDraft(saved);
        setSavingState("saved");
        setLastSavedAt(Date.now());
        return saved;
      } catch (error) {
        setSavingState("idle");
        if (!silent) toast.error(`Failed to save draft: ${error}`);
        return null;
      }
    },
    [bcc, bodyHtml, bodyText, cc, subject, threadId, to],
  );

  // Schedule autosave whenever fields change.
  useEffect(() => {
    if (!open || loading) return;
    if (skipNextAutosaveRef.current) {
      skipNextAutosaveRef.current = false;
      return;
    }
    if (autosaveTimerRef.current) window.clearTimeout(autosaveTimerRef.current);
    autosaveTimerRef.current = window.setTimeout(() => {
      void persistDraft(true);
    }, AUTOSAVE_DEBOUNCE_MS);
    return () => {
      if (autosaveTimerRef.current) {
        window.clearTimeout(autosaveTimerRef.current);
      }
    };
  }, [open, loading, persistDraft]);

  // ---- ESC to close --------------------------------------------------------
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

  // ---- AI Draft ------------------------------------------------------------
  async function handleAiDraft() {
    if (!threadId || aiDrafting) return;
    setAiDrafting(true);
    try {
      const reply = await gmailDraftReplyWithBrain(threadId);
      // Convert plain text with blank-line paragraphs into HTML.
      const html = textToHtml(reply);
      editorRef.current?.setHtml(html);
      setBodyHtml(html);
      setBodyText(reply);
      // Force-save so the AI draft is durable immediately.
      await persistDraft(true);
      toast.success("AI drafted a reply using your brain context");
    } catch (error) {
      toast.error(`AI draft failed: ${error}`);
    } finally {
      setAiDrafting(false);
    }
  }

  // ---- Start fresh ---------------------------------------------------------
  async function handleStartFresh() {
    if (!draftRef.current) {
      // Nothing persisted yet; just clear local state.
      editorRef.current?.clear();
      setBodyHtml("");
      setBodyText("");
      setSubject(defaultSubject);
      setTo(defaultTo);
      setCc([]);
      setBcc([]);
      setShowCc(false);
      setShowBcc(false);
      setConfirmFresh(false);
      return;
    }
    try {
      await gmailDeleteLocalDraft(draftRef.current.id);
      skipNextAutosaveRef.current = true;
      setDraft(null);
      editorRef.current?.clear();
      setBodyHtml("");
      setBodyText("");
      setSubject(defaultSubject);
      setTo(defaultTo);
      setCc([]);
      setBcc([]);
      setShowCc(false);
      setShowBcc(false);
      setSavingState("idle");
      setLastSavedAt(null);
      toast.success("Draft cleared");
    } catch (error) {
      toast.error(`Failed to clear draft: ${error}`);
    } finally {
      setConfirmFresh(false);
    }
  }

  // ---- Attachments ---------------------------------------------------------
  async function ensureDraftId(): Promise<string | null> {
    if (draftRef.current) return draftRef.current.id;
    const saved = await persistDraft(true);
    return saved?.id ?? null;
  }

  async function handleAddAttachment(sourcePath: string) {
    const id = await ensureDraftId();
    if (!id) throw new Error("draft not ready");
    const attachment = await gmailAddDraftAttachment(id, sourcePath);
    setDraft((prev) =>
      prev ? { ...prev, attachments: [...prev.attachments, attachment] } : prev,
    );
  }

  async function handleRemoveAttachment(attachmentId: string) {
    await gmailRemoveDraftAttachment(attachmentId);
    setDraft((prev) =>
      prev
        ? {
            ...prev,
            attachments: prev.attachments.filter((a) => a.id !== attachmentId),
          }
        : prev,
    );
  }

  // ---- Send ----------------------------------------------------------------
  async function handleSend() {
    if (sending) return;
    if (to.length === 0) {
      toast.error("Add at least one recipient");
      return;
    }
    if (!subject.trim()) {
      toast.error("Subject is required");
      return;
    }
    if (!bodyText.trim()) {
      toast.error("Body is empty");
      return;
    }
    // Make sure the latest state is persisted before send.
    const saved = await persistDraft(true);
    setSending(true);
    try {
      await gmailSendEmail({
        to,
        cc,
        bcc,
        subject,
        body: bodyText,
        body_html: bodyHtml,
        draft_id: saved?.id ?? draftRef.current?.id ?? null,
        thread_id: threadId,
      });
      // Clean up the draft on success.
      if (saved?.id) {
        await gmailDeleteLocalDraft(saved.id).catch(() => {});
      } else if (draftRef.current?.id) {
        await gmailDeleteLocalDraft(draftRef.current.id).catch(() => {});
      }
      toast.success("Reply sent");
      onSent();
      onClose();
    } catch (error) {
      toast.error(`Failed to send: ${error}`);
    } finally {
      setSending(false);
    }
  }

  // ---- Render --------------------------------------------------------------
  const headerLabel = useMemo(() => {
    if (defaultSubject) return defaultSubject;
    return "New message";
  }, [defaultSubject]);

  return (
    <AnimatePresence>
      {open ? (
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
            className="flex max-h-[85vh] w-full max-w-4xl flex-col rounded-t-2xl border border-zinc-100 bg-white shadow-[0_-12px_40px_rgba(0,0,0,0.18)]"
            exit={{ y: "100%" }}
            initial={{ y: "100%" }}
            onMouseDown={(event) => event.stopPropagation()}
            transition={{ type: "spring", stiffness: 380, damping: 36 }}
          >
            {/* Header */}
            <header className="flex shrink-0 items-center justify-between gap-3 border-b border-zinc-100 px-5 py-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-400">
                  Reply
                </span>
                <span className="truncate text-sm text-zinc-500">
                  {headerLabel}
                </span>
              </div>
              <div className="flex shrink-0 items-center gap-3">
                <SaveIndicator state={savingState} savedAt={lastSavedAt} />
                <button
                  aria-label="Close composer"
                  className="rounded-md p-1.5 text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
                  onClick={onClose}
                  type="button"
                >
                  <X size={16} />
                </button>
              </div>
            </header>

            {loading ? (
              <div className="flex flex-1 items-center justify-center py-12">
                <Loader2 className="animate-spin text-zinc-400" size={20} />
              </div>
            ) : (
              <>
                {/* Recipients + subject */}
                <div className="shrink-0 space-y-1 border-b border-zinc-100 px-5 py-2 text-sm">
                  <RecipientField
                    label="To"
                    onChange={setTo}
                    stakeholders={stakeholders}
                    values={to}
                    trailing={
                      <div className="flex items-center gap-2 text-[11px]">
                        {!showCc && (
                          <button
                            className="text-zinc-400 hover:text-zinc-900"
                            onClick={() => setShowCc(true)}
                            type="button"
                          >
                            Cc
                          </button>
                        )}
                        {!showBcc && (
                          <button
                            className="text-zinc-400 hover:text-zinc-900"
                            onClick={() => setShowBcc(true)}
                            type="button"
                          >
                            Bcc
                          </button>
                        )}
                      </div>
                    }
                  />
                  {showCc && (
                    <RecipientField
                      label="Cc"
                      onChange={setCc}
                      stakeholders={stakeholders}
                      values={cc}
                    />
                  )}
                  {showBcc && (
                    <RecipientField
                      label="Bcc"
                      onChange={setBcc}
                      stakeholders={stakeholders}
                      values={bcc}
                    />
                  )}
                  <div className="flex items-center gap-2 border-t border-zinc-50 pt-1.5">
                    <span className="w-16 shrink-0 text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
                      Subject
                    </span>
                    <input
                      aria-label="Subject"
                      className="min-w-0 flex-1 bg-transparent text-sm text-zinc-800 outline-none placeholder:text-zinc-400"
                      onChange={(e) => setSubject(e.currentTarget.value)}
                      placeholder="Subject"
                      type="text"
                      value={subject}
                    />
                  </div>
                </div>

                {/* Editor */}
                <div className="flex min-h-0 flex-1 flex-col">
                  <RichTextEditor
                    onChange={(html, text) => {
                      setBodyHtml(html);
                      setBodyText(text);
                    }}
                    placeholder="Write your reply…"
                    ref={editorRef}
                    value={bodyHtml}
                  />
                </div>

                {/* Attachments row */}
                <div className="shrink-0 border-t border-zinc-100 px-5 py-2">
                  <AttachmentArea
                    attachments={draft?.attachments ?? []}
                    disabled={sending}
                    onAdd={handleAddAttachment}
                    onRemove={handleRemoveAttachment}
                  />
                </div>

                {/* Footer toolbar */}
                <footer className="flex shrink-0 items-center justify-between gap-3 border-t border-zinc-100 bg-zinc-50/40 px-5 py-3">
                  <div className="flex items-center gap-1">
                    <FooterButton
                      busy={aiDrafting}
                      disabled={!threadId || sending}
                      icon={<Sparkles className="text-violet-500" size={14} />}
                      label="AI Draft"
                      onClick={() => void handleAiDraft()}
                      tone="violet"
                    />
                    <FooterButton
                      busy={savingState === "saving"}
                      disabled={sending}
                      icon={<Save size={14} />}
                      label="Save Draft"
                      onClick={() => void persistDraft(false)}
                    />
                    {confirmFresh ? (
                      <div className="flex items-center gap-1 rounded-md border border-rose-200 bg-rose-50 px-2 py-1 text-[11px]">
                        <span className="text-rose-700">Clear draft?</span>
                        <button
                          className="rounded px-1.5 py-0.5 font-semibold text-rose-700 hover:bg-rose-100"
                          onClick={() => void handleStartFresh()}
                          type="button"
                        >
                          Yes
                        </button>
                        <button
                          className="rounded px-1.5 py-0.5 text-zinc-600 hover:bg-zinc-100"
                          onClick={() => setConfirmFresh(false)}
                          type="button"
                        >
                          No
                        </button>
                      </div>
                    ) : (
                      <FooterButton
                        disabled={sending}
                        icon={<RotateCcw size={14} />}
                        label="Start fresh"
                        onClick={() => setConfirmFresh(true)}
                      />
                    )}
                  </div>
                  <button
                    className="flex items-center gap-2 rounded-lg bg-sky-600 px-4 py-2 text-sm font-semibold text-white shadow-sm transition-colors hover:bg-sky-700 disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={sending || aiDrafting}
                    onClick={() => void handleSend()}
                    type="button"
                  >
                    {sending ? (
                      <Loader2 className="animate-spin" size={15} />
                    ) : (
                      <Send size={15} />
                    )}
                    {sending ? "Sending…" : "Send"}
                  </button>
                </footer>
              </>
            )}
          </motion.section>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Sub-components
// ──────────────────────────────────────────────────────────────────────────

function SaveIndicator({
  state,
  savedAt,
}: {
  state: "idle" | "saving" | "saved";
  savedAt: number | null;
}) {
  const [, force] = useState(0);
  // Re-render every 20s so the "saved Xs ago" stays current.
  useEffect(() => {
    if (!savedAt) return;
    const id = window.setInterval(() => force((v) => v + 1), 20000);
    return () => window.clearInterval(id);
  }, [savedAt]);

  if (state === "saving") {
    return (
      <span className="inline-flex items-center gap-1 text-[11px] text-zinc-400">
        <Loader2 className="animate-spin" size={11} />
        Saving…
      </span>
    );
  }
  if (state === "saved" && savedAt) {
    return (
      <span className="inline-flex items-center gap-1 text-[11px] text-emerald-600">
        <Check size={11} />
        Saved {relativeAgo(savedAt)}
      </span>
    );
  }
  return null;
}

function FooterButton({
  busy,
  disabled,
  icon,
  label,
  onClick,
  tone,
}: {
  busy?: boolean;
  disabled?: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  tone?: "violet";
}) {
  const toneClass =
    tone === "violet"
      ? "text-violet-700 hover:bg-violet-50 hover:text-violet-800"
      : "text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900";
  return (
    <button
      className={`flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${toneClass}`}
      disabled={disabled || busy}
      onClick={onClick}
      title={label}
      type="button"
    >
      {busy ? <Loader2 className="animate-spin" size={14} /> : icon}
      {label}
    </button>
  );
}

function RecipientField({
  label,
  onChange,
  stakeholders,
  trailing,
  values,
}: {
  label: string;
  onChange: (next: string[]) => void;
  stakeholders: Stakeholder[];
  trailing?: React.ReactNode;
  values: string[];
}) {
  const [draftInput, setDraftInput] = useState("");
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerSearch, setPickerSearch] = useState("");

  function commit() {
    const trimmed = draftInput.trim().replace(/,$/, "");
    if (!trimmed) return;
    if (!values.includes(trimmed)) onChange([...values, trimmed]);
    setDraftInput("");
  }

  function pickStakeholder(s: Stakeholder) {
    if (!s.email) return;
    if (!values.includes(s.email)) onChange([...values, s.email]);
    setPickerOpen(false);
    setPickerSearch("");
  }

  const filteredStakeholders = stakeholders.filter((s) => {
    if (values.includes(s.email)) return false;
    if (!pickerSearch.trim()) return true;
    const q = pickerSearch.toLowerCase();
    return (
      s.name.toLowerCase().includes(q) ||
      s.email.toLowerCase().includes(q) ||
      (s.role && s.role.toLowerCase().includes(q))
    );
  });

  return (
    <div className="relative flex items-center gap-2">
      <span className="w-16 shrink-0 text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
        {label}
      </span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
        {values.map((addr) => {
          const linked = stakeholders.find(
            (s) => s.email.toLowerCase() === addr.toLowerCase(),
          );
          return (
            <span
              className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[12px] ${
                linked
                  ? "border border-sky-100 bg-sky-50 text-sky-700"
                  : "bg-zinc-100 text-zinc-700"
              }`}
              key={addr}
              title={linked ? `${linked.name} <${addr}>` : addr}
            >
              {linked ? linked.name : addr}
              <button
                aria-label={`Remove ${addr}`}
                className="rounded p-0.5 text-zinc-400 hover:bg-zinc-200 hover:text-rose-600"
                onClick={() => onChange(values.filter((v) => v !== addr))}
                type="button"
              >
                <X size={10} />
              </button>
            </span>
          );
        })}
        <input
          aria-label={`${label} recipients`}
          className="min-w-[120px] flex-1 bg-transparent text-sm text-zinc-800 outline-none placeholder:text-zinc-400"
          onBlur={commit}
          onChange={(e) => setDraftInput(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === "," || e.key === "Tab") {
              if (draftInput.trim()) {
                e.preventDefault();
                commit();
              }
            } else if (e.key === "Backspace" && draftInput === "" && values.length > 0) {
              onChange(values.slice(0, -1));
            }
          }}
          placeholder={values.length === 0 ? "name@example.com" : ""}
          type="email"
          value={draftInput}
        />
      </div>
      <button
        aria-label={`Pick stakeholder for ${label}`}
        className={`shrink-0 rounded p-1 transition-colors ${
          pickerOpen
            ? "bg-zinc-100 text-zinc-900"
            : "text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
        }`}
        onClick={() => setPickerOpen((v) => !v)}
        title="Pick from stakeholders"
        type="button"
      >
        <UsersRound size={14} />
      </button>
      {trailing ? <div className="shrink-0">{trailing}</div> : null}

      {pickerOpen ? (
        <>
          <button
            aria-label="Close picker"
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setPickerOpen(false)}
            tabIndex={-1}
            type="button"
          />
          <div className="absolute right-0 top-full z-20 mt-1 w-72 overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.12)]">
            <div className="flex items-center gap-2 border-b border-zinc-100 px-3 py-2">
              <Search className="text-zinc-400" size={13} />
              <input
                aria-label="Search stakeholders"
                autoFocus
                className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-400"
                onChange={(e) => setPickerSearch(e.currentTarget.value)}
                placeholder="Search stakeholders…"
                type="text"
                value={pickerSearch}
              />
            </div>
            <div className="max-h-72 overflow-y-auto py-1">
              {filteredStakeholders.length === 0 ? (
                <p className="px-3 py-2 text-[12px] text-zinc-400">
                  {stakeholders.length === 0
                    ? "No stakeholders yet."
                    : "No matches."}
                </p>
              ) : (
                filteredStakeholders.slice(0, 12).map((s) => (
                  <button
                    className="flex w-full items-start gap-2 px-3 py-1.5 text-left text-sm hover:bg-zinc-50"
                    key={s.id}
                    onClick={() => pickStakeholder(s)}
                    type="button"
                  >
                    <span className="mt-0.5 inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-zinc-100 text-[10px] font-semibold uppercase text-zinc-600">
                      {(s.name || s.email).slice(0, 2)}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-zinc-800">
                        {s.name || s.email}
                      </span>
                      <span className="block truncate text-[11px] text-zinc-400">
                        {s.email}
                        {s.role ? ` · ${s.role}` : ""}
                      </span>
                    </span>
                  </button>
                ))
              )}
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────

function textToHtml(text: string): string {
  // Convert blank-line-separated paragraphs into <p>…</p>; single newlines → <br/>.
  const safe = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  const paragraphs = safe.split(/\n\s*\n/).filter(Boolean);
  return paragraphs.map((p) => `<p>${p.replace(/\n/g, "<br/>")}</p>`).join("");
}

function relativeAgo(ts: number): string {
  const seconds = Math.max(1, Math.floor((Date.now() - ts) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}
// Ensure ChevronDown is referenced to silence any tree-shaking concerns in dev.
// (Used implicitly by future expandable rows.)
void ChevronDown;
