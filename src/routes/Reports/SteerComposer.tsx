import { useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowUp, X } from "lucide-react";

import { steerReport } from "../../lib/ipc";
import { toast } from "../../lib/toast";

interface Props {
  reportRunId: string;
  disabled?: boolean;
}

export function SteerComposer({ reportRunId, disabled }: Props) {
  const [text, setText] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  async function submit() {
    const instruction = text.trim();
    if (!instruction || disabled) return;
    setText("");
    textareaRef.current?.focus();
    try {
      await steerReport(reportRunId, instruction);
      setHistory((prev) => [instruction, ...prev].slice(0, 8));
      toast.success("Steering instruction sent — watch the timeline for progress.");
    } catch (err) {
      toast.error(String(err));
    }
  }

  function handleKey(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  }

  return (
    <div className="border-t border-zinc-100 bg-white px-4 py-3">
      {history.length > 0 ? (
        <AnimatePresence>
          <div className="mb-2 flex flex-wrap gap-1.5">
            {history.map((item, i) => (
              <motion.button
                animate={{ opacity: 1, scale: 1 }}
                className="inline-flex items-center gap-1 rounded-full border border-zinc-100 bg-zinc-50 px-2.5 py-1 text-[11px] text-zinc-500 hover:bg-white"
                initial={{ opacity: 0, scale: 0.9 }}
                key={`${item}-${i}`}
                onClick={() => setText(item)}
                type="button"
              >
                {item.length > 40 ? `${item.slice(0, 40)}…` : item}
              </motion.button>
            ))}
          </div>
        </AnimatePresence>
      ) : null}

      <div className="flex items-end gap-2 rounded-2xl border border-zinc-200 bg-zinc-50 px-4 py-3 focus-within:border-violet-300 focus-within:bg-white transition-colors">
        <textarea
          className="min-h-[36px] flex-1 resize-none bg-transparent text-sm text-zinc-800 placeholder:text-zinc-400 outline-none"
          disabled={disabled}
          onChange={(e) => setText(e.currentTarget.value)}
          onKeyDown={handleKey}
          placeholder="Steer the report — tighten section 3, add a risks section, run critique…"
          ref={textareaRef}
          rows={1}
          style={{ height: text.includes("\n") ? "auto" : undefined }}
          value={text}
        />
        {text ? (
          <div className="flex shrink-0 items-center gap-1.5 pb-0.5">
            <button
              aria-label="Clear"
              className="text-zinc-400 hover:text-zinc-600"
              onClick={() => setText("")}
              type="button"
            >
              <X size={14} />
            </button>
            <button
              aria-label="Send steering instruction"
              className="flex h-7 w-7 items-center justify-center rounded-full bg-zinc-900 text-white hover:bg-violet-700 transition-colors disabled:opacity-40"
              disabled={disabled || !text.trim()}
              onClick={() => void submit()}
              type="button"
            >
              <ArrowUp size={14} />
            </button>
          </div>
        ) : null}
      </div>

      <p className="mt-1.5 text-[10px] text-zinc-400">
        Enter to send · Shift+Enter for newline · Routes to the right pipeline step automatically
      </p>
    </div>
  );
}
