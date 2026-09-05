// Per-turn view inside an Ask chat — the question bubble, tool activity,
// streamed answer, citations, retry/feedback affordances. Plus the small
// helpers it needs: ThinkingTicker, ClarificationCard, VariantNavigator,
// UserMessageEditor, TurnAttachmentList, ComposerAttachmentChip.
//
// Extracted from AskWorkspace.tsx (E3).

import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { PenLine, RefreshCw, ThumbsDown, ThumbsUp, X } from "lucide-react";

import { recordBrainFeedback } from "../../lib/ipc";
import type { AskUserQuestion } from "../../lib/types";
import { recordBrainSignal } from "../../lib/brainSignals";
import { WhyThisAnswerPanel } from "../../components/WhyThisAnswerPanel";
import { MarkdownAnswer, ReferenceDisclosure } from "./Citations";
import { ReasoningPanel, ToolSummary } from "./ToolPanel";
import { ThinkingTicker } from "./ThinkingTicker";
import { Avatar } from "./icons";
import type { AskAttachment, AskTurn } from "./state";
import { ClarifyPrompt } from "../../components/ClarifyPrompt";

export function TurnView({
  turn,
  onAnswerClarification,
  onEdit,
  onCancelEdit,
  onSubmitEdit,
  onRetry,
  onSwitchVariant,
  siblingCount,
  variantIndex,
  isEditing,
  isSubmitting,
  autoConfirmTools,
  onToggleAutoConfirm,
}: {
  turn: AskTurn;
  onAnswerClarification: (question: AskUserQuestion, answer: string) => void;
  onEdit: () => void;
  onCancelEdit: () => void;
  onSubmitEdit: (value: string) => void;
  onRetry: () => void;
  onSwitchVariant: (direction: -1 | 1) => void;
  siblingCount: number;
  variantIndex: number;
  isEditing: boolean;
  isSubmitting: boolean;
  autoConfirmTools: string[];
  onToggleAutoConfirm: (tool: string, enabled: boolean) => void;
}) {
  const inFlight = turn.status === "running" || turn.status === "streaming";
  const showVariantNav = siblingCount > 1;
  const [feedbackSent, setFeedbackSent] = useState<"useful" | "wrong" | null>(null);
  const sendFeedback = useCallback(async (value: "useful" | "wrong") => {
    if (feedbackSent) return;
    setFeedbackSent(value);
    await Promise.all([
      recordBrainFeedback({
        question: turn.question,
        template: "ask_answer",
        feedback: value,
        corrected: { item_id: turn.id, item_kind: "ask_turn" },
      }).catch(() => {}),
      recordBrainSignal({
        template: "ask_answer",
        itemId: turn.id,
        itemKind: "ask_turn",
        eventType: value,
        context: { question: turn.question },
      }),
    ]);
  }, [feedbackSent, turn.id, turn.question]);
  return (
    <article className="space-y-3">
      <div className="group flex items-start gap-3">
        <Avatar tone="user" />
        <div className="min-w-0 flex-1">
          {isEditing ? (
            <UserMessageEditor
              initial={turn.question}
              onCancel={onCancelEdit}
              onSubmit={onSubmitEdit}
            />
          ) : (
            <div className="rounded-2xl bg-zinc-100 px-4 py-3 text-[14px] leading-6 text-zinc-950">
              {turn.question}
              {turn.reasoningDepth === "deep" ? (
                <span className="ml-2 inline-flex rounded-full bg-violet-100 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-violet-700">
                  Deep
                </span>
              ) : null}
              {turn.attachments && turn.attachments.length > 0 ? (
                <TurnAttachmentList attachments={turn.attachments} />
              ) : null}
            </div>
          )}
          {!isEditing ? (
            <div className="mt-1.5 flex items-center gap-1 text-[11px] text-zinc-400 opacity-0 transition-opacity group-hover:opacity-100">
              {showVariantNav ? (
                <VariantNavigator
                  index={variantIndex}
                  onPrev={() => onSwitchVariant(-1)}
                  onNext={() => onSwitchVariant(1)}
                  total={siblingCount}
                />
              ) : null}
              <button
                className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 hover:bg-zinc-100 hover:text-zinc-950"
                disabled={isSubmitting}
                onClick={onEdit}
                type="button"
              >
                <PenLine size={11} />
                Edit
              </button>
            </div>
          ) : null}
        </div>
      </div>

      <div className="group flex items-start gap-3">
        <Avatar tone="trace" />
        <div className="min-w-0 flex-1 space-y-3">
          <ToolSummary
            mode={turn.mode ?? "research"}
            steps={turn.steps}
            status={turn.status}
            autoConfirmTools={autoConfirmTools}
            onToggleAutoConfirm={onToggleAutoConfirm}
          />
          {turn.reasoning ? <ReasoningPanel reasoning={turn.reasoning} /> : null}
          <AnimatePresence mode="popLayout">
            {turn.status === "running" && !turn.answer ? (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="px-1 py-1"
                exit={{ opacity: 0, y: -4 }}
                initial={{ opacity: 0, y: 4 }}
              >
                <ThinkingTicker />
              </motion.div>
            ) : turn.status === "error" ? (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="notice notice-error"
                exit={{ opacity: 0, y: -4 }}
                initial={{ opacity: 0, y: 4 }}
              >
                {turn.error}
              </motion.div>
            ) : turn.status === "cancelled" && !turn.answer ? (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="rounded-2xl border border-zinc-100 bg-zinc-50 px-4 py-3 text-[13px] text-zinc-400"
                exit={{ opacity: 0, y: -4 }}
                initial={{ opacity: 0, y: 4 }}
              >
                Stopped.
              </motion.div>
            ) : (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="rounded-2xl border border-zinc-100 bg-white px-4 py-3 text-[14px] leading-6 text-zinc-950 shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
                exit={{ opacity: 0, y: -4 }}
                initial={{ opacity: 0, y: 4 }}
              >
                <MarkdownAnswer content={turn.answer} refs={turn.refs} streaming={turn.status === "streaming"} />
                {turn.status === "streaming" ? (
                  <span className="ml-0.5 inline-block h-4 w-[2px] animate-pulse bg-zinc-950 align-middle" />
                ) : null}
                {turn.status === "cancelled" ? (
                  <p className="mt-2 text-[12px] text-zinc-400">Stopped before completion.</p>
                ) : null}
              </motion.div>
            )}
          </AnimatePresence>

          {turn.refs.length > 0 ? <ReferenceDisclosure refs={turn.refs} /> : null}
          {turn.scoredNodes && turn.scoredNodes.length > 0 ? (
            <WhyThisAnswerPanel
              scored={turn.scoredNodes}
              refs={turn.refs}
              query={turn.retrievalQuery}
            />
          ) : null}
          {turn.questions.length > 0 ? (
            <div className="space-y-3">
              {turn.questions.map((question) => (
                <ClarifyPrompt
                  freeTextAllowed
                  key={`${turn.id}-${question.question}`}
                  onAnswer={(selectedLabels, freeText) => {
                    const answer = freeText?.trim()
                      ? freeText.trim()
                      : selectedLabels.join(", ");
                    onAnswerClarification(question, answer);
                  }}
                  options={question.options.map((opt) => ({
                    label: opt.label,
                    description: opt.description,
                  }))}
                  question={question.question}
                />
              ))}
            </div>
          ) : null}
          {!inFlight ? (
            <div className="flex items-center gap-1 text-[11px] text-zinc-400 opacity-0 transition-opacity group-hover:opacity-100">
              <button
                className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 hover:bg-zinc-100 hover:text-zinc-950"
                disabled={isSubmitting}
                onClick={onRetry}
                type="button"
              >
                <RefreshCw size={11} />
                {turn.status === "error" || turn.status === "cancelled" ? "Try again" : "Retry"}
              </button>
              {turn.status === "done" ? (
                <>
                  <span className="mx-0.5 text-zinc-200">|</span>
                  <button
                    aria-label="Helpful"
                    className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 hover:bg-zinc-100 hover:text-zinc-950 ${feedbackSent === "useful" ? "text-emerald-600" : ""}`}
                    disabled={feedbackSent !== null}
                    onClick={() => sendFeedback("useful")}
                    type="button"
                  >
                    <ThumbsUp size={11} />
                  </button>
                  <button
                    aria-label="Not helpful"
                    className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 hover:bg-zinc-100 hover:text-zinc-950 ${feedbackSent === "wrong" ? "text-rose-500" : ""}`}
                    disabled={feedbackSent !== null}
                    onClick={() => sendFeedback("wrong")}
                    type="button"
                  >
                    <ThumbsDown size={11} />
                  </button>
                </>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>
    </article>
  );
}

export function ComposerAttachmentChip({
  attachment,
  onRemove,
}: {
  attachment: AskAttachment;
  onRemove: () => void;
}) {
  const dataUrl = `data:${attachment.mimeType};base64,${attachment.data}`;
  return (
    <div className="group relative h-14 w-14 overflow-hidden rounded-lg border border-zinc-100 bg-zinc-100">
      <img
        alt={attachment.filename ?? "attachment"}
        className="h-full w-full object-cover"
        src={dataUrl}
      />
      <button
        aria-label={`Remove attachment ${attachment.filename ?? ""}`}
        className="absolute right-1 top-1 inline-flex h-4 w-4 items-center justify-center rounded-full bg-zinc-950 text-white opacity-0 transition-opacity group-hover:opacity-100"
        onClick={onRemove}
        type="button"
      >
        <X size={9} />
      </button>
    </div>
  );
}

function TurnAttachmentList({ attachments }: { attachments: AskAttachment[] }) {
  if (attachments.length === 0) return null;
  return (
    <div className="mt-2 flex flex-wrap gap-2">
      {attachments.map((attachment) => {
        const dataUrl = `data:${attachment.mimeType};base64,${attachment.data}`;
        return (
          <a
            className="block h-24 w-24 overflow-hidden rounded-lg border border-zinc-100 bg-zinc-100"
            href={dataUrl}
            key={attachment.id}
            rel="noreferrer"
            target="_blank"
            title={attachment.filename ?? attachment.mimeType}
          >
            <img
              alt={attachment.filename ?? "attachment"}
              className="h-full w-full object-cover"
              src={dataUrl}
            />
          </a>
        );
      })}
    </div>
  );
}

function VariantNavigator({
  index,
  total,
  onPrev,
  onNext,
}: {
  index: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
}) {
  return (
    <div className="inline-flex items-center gap-0.5 rounded-md border border-zinc-100 bg-white px-1 text-zinc-600">
      <button
        aria-label="Previous variant"
        className="inline-flex h-4 w-4 items-center justify-center rounded hover:bg-zinc-100"
        onClick={onPrev}
        type="button"
      >
        ‹
      </button>
      <span className="px-1 tabular-nums">
        {index}/{total}
      </span>
      <button
        aria-label="Next variant"
        className="inline-flex h-4 w-4 items-center justify-center rounded hover:bg-zinc-100"
        onClick={onNext}
        type="button"
      >
        ›
      </button>
    </div>
  );
}

function UserMessageEditor({
  initial,
  onCancel,
  onSubmit,
}: {
  initial: string;
  onCancel: () => void;
  onSubmit: (value: string) => void;
}) {
  const [value, setValue] = useState(initial);
  const ref = useRef<HTMLTextAreaElement | null>(null);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.setSelectionRange(value.length, value.length);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <form
      className="rounded-2xl border border-zinc-100 bg-white px-3 py-2"
      onSubmit={(event) => {
        event.preventDefault();
        const trimmed = value.trim();
        if (trimmed) onSubmit(trimmed);
      }}
    >
      <textarea
        className="block w-full resize-y bg-transparent text-[14px] leading-6 text-zinc-950 outline-none"
        onChange={(event) => setValue(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onCancel();
          } else if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
            event.preventDefault();
            const trimmed = value.trim();
            if (trimmed) onSubmit(trimmed);
          }
        }}
        ref={ref}
        rows={Math.min(8, Math.max(2, value.split("\n").length))}
        value={value}
      />
      <div className="mt-1 flex items-center justify-end gap-1.5 text-[11px]">
        <button
          className="rounded-md px-2 py-1 text-zinc-400 hover:bg-zinc-100"
          onClick={onCancel}
          type="button"
        >
          Cancel
        </button>
        <button
          className="rounded-md bg-zinc-950 px-2 py-1 font-medium text-white hover:bg-zinc-800 disabled:bg-zinc-200 disabled:text-zinc-400"
          disabled={!value.trim()}
          type="submit"
        >
          Send branch
        </button>
      </div>
    </form>
  );
}

