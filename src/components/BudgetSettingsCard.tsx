import { useEffect, useState } from "react";
import { Check, Loader2, ShieldAlert, Wallet } from "lucide-react";
import {
  getAppConfig,
  getBudgetStatus,
  setAppConfig as saveAppConfig,
} from "../lib/ipc";
import type { AppConfig, BudgetStatus } from "../lib/types";

const DEFAULT_CONFIG: AppConfig = {
  budget_daily_usd: 0,
  budget_monthly_usd: 0,
  budget_alert_threshold_pct: 80,
  budget_block_when_exceeded: false,
};

function formatUsd(n: number): string {
  if (!Number.isFinite(n)) return "$0.00";
  return `$${n.toFixed(2)}`;
}

function percentBarColor(pct: number, exceeded: boolean): string {
  if (exceeded) return "bg-red-500";
  if (pct >= 80) return "bg-amber-500";
  return "bg-sky-400";
}

function ProgressBar({ pct, exceeded }: { pct: number; exceeded: boolean }) {
  const clamped = Math.max(0, Math.min(100, pct));
  return (
    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-zinc-100">
      <div
        className={["h-full transition-all", percentBarColor(pct, exceeded)].join(" ")}
        style={{ width: `${Math.max(clamped, pct > 0 ? 2 : 0)}%` }}
      />
    </div>
  );
}

export function BudgetSettingsCard() {
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [status, setStatus] = useState<BudgetStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  async function refresh() {
    try {
      const [cfg, st] = await Promise.all([getAppConfig(), getBudgetStatus()]);
      if (cfg) setConfig(cfg);
      if (st) setStatus(st);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
    const id = setInterval(() => void refresh(), 30_000);
    return () => clearInterval(id);
  }, []);

  async function handleSave() {
    setSaving(true);
    setMessage(null);
    try {
      await saveAppConfig(config);
      setMessage({ ok: true, text: "Budget settings saved." });
      void refresh();
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setSaving(false);
    }
  }

  const limitsConfigured =
    config.budget_daily_usd > 0 || config.budget_monthly_usd > 0;

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between px-5 py-4 border-b border-zinc-100">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-50 text-emerald-600">
            <Wallet size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">AI budget</h2>
            <p className="text-[11px] text-zinc-400">
              Warn or pause AI calls when spend crosses a threshold
            </p>
          </div>
        </div>
        {status?.block_active ? (
          <span className="flex items-center gap-1.5 rounded-full bg-red-50 px-2.5 py-1 text-[11px] font-medium text-red-700">
            <ShieldAlert size={11} />
            Blocked
          </span>
        ) : status?.alert_state === "exceeded" ? (
          <span className="flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-1 text-[11px] font-medium text-amber-700">
            Over limit
          </span>
        ) : status?.alert_state === "warning" ? (
          <span className="flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-1 text-[11px] font-medium text-amber-700">
            Near limit
          </span>
        ) : null}
      </div>

      <div className="px-5 py-4 space-y-4">
        {message && (
          <div
            className={[
              "flex items-center gap-2 rounded-lg px-3 py-2 text-[12px]",
              message.ok
                ? "bg-emerald-50 text-emerald-800"
                : "bg-red-50 text-red-700",
            ].join(" ")}
          >
            <Check size={13} />
            {message.text}
          </div>
        )}

        {/* Current spend */}
        {loading ? (
          <div className="space-y-2">
            <div className="h-12 animate-pulse rounded-xl bg-zinc-100" />
            <div className="h-12 animate-pulse rounded-xl bg-zinc-100" />
          </div>
        ) : status ? (
          <div className="grid grid-cols-2 gap-3">
            <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3 space-y-1.5">
              <div className="text-[11px] uppercase tracking-wide text-zinc-400">
                Today
              </div>
              <div className="text-[13px] font-semibold text-zinc-900">
                {formatUsd(status.daily_spent_usd)}
                {status.daily_limit_usd > 0 && (
                  <span className="ml-1.5 text-[11px] font-normal text-zinc-400">
                    of {formatUsd(status.daily_limit_usd)}
                  </span>
                )}
              </div>
              {status.daily_limit_usd > 0 && (
                <ProgressBar
                  pct={status.daily_pct}
                  exceeded={status.daily_spent_usd >= status.daily_limit_usd}
                />
              )}
            </div>
            <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3 space-y-1.5">
              <div className="text-[11px] uppercase tracking-wide text-zinc-400">
                This month
              </div>
              <div className="text-[13px] font-semibold text-zinc-900">
                {formatUsd(status.monthly_spent_usd)}
                {status.monthly_limit_usd > 0 && (
                  <span className="ml-1.5 text-[11px] font-normal text-zinc-400">
                    of {formatUsd(status.monthly_limit_usd)}
                  </span>
                )}
              </div>
              {status.monthly_limit_usd > 0 && (
                <ProgressBar
                  pct={status.monthly_pct}
                  exceeded={status.monthly_spent_usd >= status.monthly_limit_usd}
                />
              )}
            </div>
          </div>
        ) : null}

        {/* Limit inputs */}
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1.5">
            <label className="field-label">Daily limit (USD)</label>
            <input
              type="number"
              min="0"
              step="0.01"
              className="field-control w-full"
              value={config.budget_daily_usd}
              onChange={(e) =>
                setConfig({ ...config, budget_daily_usd: Number(e.target.value) || 0 })
              }
            />
          </div>
          <div className="space-y-1.5">
            <label className="field-label">Monthly limit (USD)</label>
            <input
              type="number"
              min="0"
              step="1"
              className="field-control w-full"
              value={config.budget_monthly_usd}
              onChange={(e) =>
                setConfig({ ...config, budget_monthly_usd: Number(e.target.value) || 0 })
              }
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <label className="field-label">
            Alert at {Math.round(config.budget_alert_threshold_pct)}% of limit
          </label>
          <input
            type="range"
            min="10"
            max="100"
            step="5"
            className="w-full accent-emerald-500"
            value={config.budget_alert_threshold_pct}
            onChange={(e) =>
              setConfig({
                ...config,
                budget_alert_threshold_pct: Number(e.target.value),
              })
            }
          />
        </div>

        <label className="flex items-center gap-2 text-[12px] text-zinc-700">
          <input
            type="checkbox"
            className="h-3.5 w-3.5 accent-emerald-500"
            checked={config.budget_block_when_exceeded}
            onChange={(e) =>
              setConfig({ ...config, budget_block_when_exceeded: e.target.checked })
            }
          />
          Block AI calls when limit exceeded
          <span className="ml-1 text-[11px] text-zinc-400">
            (off by default — warn only)
          </span>
        </label>

        <div className="flex justify-end pt-1">
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving}
            onClick={() => void handleSave()}
          >
            {saving ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <Check size={14} />
            )}
            Save
          </button>
        </div>

        {!limitsConfigured && (
          <p className="text-[11px] text-zinc-400">
            Set a daily or monthly limit above to enable budget alerts. A limit
            of 0 means no limit.
          </p>
        )}
      </div>
    </section>
  );
}
