import { useCallback, useEffect, useState } from "react";
import {
  AlertCircle,
  Clock,
  Inbox,
  RefreshCw,
  ScrollText,
} from "lucide-react";
import {
  gmailInboxDashboard,
  gmailRetryFailedAnalyses,
} from "../lib/ipc";
import { toast } from "../lib/toast";
import type { GmailInboxDashboard } from "../lib/types";

function intentLabel(value: string): string {
  return value
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function InboxDashboard() {
  const [data, setData] = useState<GmailInboxDashboard | null>(null);
  const [loading, setLoading] = useState(false);
  const [retrying, setRetrying] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setData(await gmailInboxDashboard(24));
    } catch {
      // toasted
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const id = window.setInterval(() => void load(), 60_000);
    return () => window.clearInterval(id);
  }, [load]);

  const handleRetry = async () => {
    setRetrying(true);
    try {
      const count = await gmailRetryFailedAnalyses(20);
      toast.success(`Re-analyzed ${count} failed thread${count === 1 ? "" : "s"}`);
      await load();
    } catch {
      // toasted
    } finally {
      setRetrying(false);
    }
  };

  if (!data || data.classified_count === 0) {
    return null;
  }

  return (
    <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] p-4">
      <div className="flex items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-2">
          <Inbox className="text-zinc-400" size={14} />
          <span className="page-kicker">Last 24 hours</span>
        </div>
        <button
          aria-label="Refresh dashboard"
          className="btn h-7 w-7 px-0"
          disabled={loading}
          onClick={() => void load()}
          type="button"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
        </button>
      </div>
      <div className="grid grid-cols-4 gap-2">
        <Tile
          icon={<ScrollText size={14} />}
          label="Classified"
          value={data.classified_count}
        />
        <Tile
          icon={<AlertCircle size={14} />}
          label="Need action"
          value={data.action_required_count}
          tone={data.action_required_count > 0 ? "warning" : undefined}
        />
        <Tile
          icon={<Clock size={14} />}
          label="Waiting on you"
          value={data.waiting_on_you_count}
          tone={data.waiting_on_you_count > 0 ? "info" : undefined}
        />
        <Tile
          icon={<AlertCircle size={14} />}
          label="High priority"
          value={data.high_priority_count}
          tone={data.high_priority_count > 0 ? "error" : undefined}
        />
      </div>

      {data.top_intents.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Intents:
          </span>
          {data.top_intents.map(([intent, count]) => (
            <span
              className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-600"
              key={intent}
            >
              {intentLabel(intent)} · {count}
            </span>
          ))}
        </div>
      )}

      {data.failed_classifications > 0 && (
        <div className="mt-3 flex items-center justify-between rounded-lg border border-red-100 bg-red-50 px-3 py-2 text-[11px] text-red-700">
          <span>
            {data.failed_classifications} thread
            {data.failed_classifications === 1 ? "" : "s"} failed to classify
          </span>
          <button
            className="text-[11px] font-semibold underline-offset-2 hover:underline"
            disabled={retrying}
            onClick={() => void handleRetry()}
            type="button"
          >
            {retrying ? "Retrying…" : "Retry now"}
          </button>
        </div>
      )}
    </section>
  );
}

function Tile({
  icon,
  label,
  value,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  tone?: "info" | "warning" | "error";
}) {
  const cls =
    tone === "error"
      ? "border-red-100 bg-red-50 text-red-700"
      : tone === "warning"
        ? "border-amber-100 bg-amber-50 text-amber-700"
        : tone === "info"
          ? "border-sky-100 bg-sky-50 text-sky-700"
          : "border-zinc-100 bg-zinc-50 text-zinc-700";
  return (
    <div className={`rounded-xl border p-3 ${cls}`}>
      <div className="flex items-center gap-1.5 text-zinc-400">
        {icon}
        <span className="text-[10px] font-semibold uppercase tracking-wider">
          {label}
        </span>
      </div>
      <p
        className={`mt-1 text-lg font-bold ${
          tone ? "" : "text-zinc-950"
        }`}
      >
        {value}
      </p>
    </div>
  );
}
