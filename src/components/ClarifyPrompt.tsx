import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Check, X } from "lucide-react";

export interface ClarifyOption {
  label: string;
  description?: string;
}

interface Props {
  question: string;
  options: ClarifyOption[];
  freeTextAllowed?: boolean;
  multiSelect?: boolean;
  onAnswer: (selectedLabels: string[], freeText?: string) => void;
  onDismiss?: () => void;
}

export function ClarifyPrompt({
  question,
  options,
  freeTextAllowed = true,
  multiSelect = false,
  onAnswer,
  onDismiss,
}: Props) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [freeText, setFreeText] = useState("");

  function toggle(label: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (multiSelect) {
        if (next.has(label)) next.delete(label);
        else next.add(label);
      } else {
        next.clear();
        if (!prev.has(label)) next.add(label);
      }
      return next;
    });
  }

  const canSubmit = selected.size > 0 || freeText.trim().length > 0;

  function submit() {
    if (!canSubmit) return;
    onAnswer(Array.from(selected), freeText.trim() || undefined);
  }

  return (
    <AnimatePresence>
      <motion.div
        animate={{ opacity: 1, y: 0 }}
        className="rounded-2xl border border-zinc-100 bg-white p-5 shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
        exit={{ opacity: 0, y: -6 }}
        initial={{ opacity: 0, y: 6 }}
        transition={{ duration: 0.18 }}
      >
        <div className="flex items-start justify-between gap-3">
          <p className="text-sm font-medium leading-snug text-zinc-800">{question}</p>
          {onDismiss ? (
            <button
              className="shrink-0 text-zinc-400 hover:text-zinc-600"
              onClick={onDismiss}
              type="button"
            >
              <X size={15} />
            </button>
          ) : null}
        </div>

        <div className="mt-4 grid gap-2">
          {options.map((opt) => {
            const active = selected.has(opt.label);
            return (
              <button
                className={[
                  "flex items-start gap-3 rounded-xl border px-4 py-3 text-left transition-colors",
                  active
                    ? "border-violet-300 bg-violet-50"
                    : "border-zinc-100 bg-zinc-50 hover:border-zinc-200 hover:bg-white",
                ].join(" ")}
                key={opt.label}
                onClick={() => toggle(opt.label)}
                type="button"
              >
                <span
                  className={[
                    "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors",
                    active ? "border-violet-500 bg-violet-500" : "border-zinc-300",
                  ].join(" ")}
                >
                  {active ? <Check className="text-white" size={10} /> : null}
                </span>
                <span>
                  <span className="text-sm font-medium text-zinc-900">{opt.label}</span>
                  {opt.description ? (
                    <span className="mt-0.5 block text-xs text-zinc-500">{opt.description}</span>
                  ) : null}
                </span>
              </button>
            );
          })}
        </div>

        {freeTextAllowed ? (
          <textarea
            className="mt-3 w-full resize-none rounded-xl border border-zinc-200 bg-white px-3 py-2.5 text-sm text-zinc-800 placeholder:text-zinc-400 focus:border-violet-400 focus:outline-none"
            onChange={(e) => setFreeText(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
            }}
            placeholder="Other — describe in your own words…"
            rows={2}
            value={freeText}
          />
        ) : null}

        <div className="mt-4 flex justify-end gap-2">
          {onDismiss ? (
            <button className="btn h-8 text-xs" onClick={onDismiss} type="button">
              Dismiss
            </button>
          ) : null}
          <button
            className="btn btn-primary h-8 text-xs"
            disabled={!canSubmit}
            onClick={submit}
            type="button"
          >
            Submit
          </button>
        </div>
      </motion.div>
    </AnimatePresence>
  );
}
