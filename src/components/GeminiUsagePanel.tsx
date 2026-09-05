import { useCallback, useEffect, useMemo, useState } from "react";
import { Coins, RefreshCw } from "lucide-react";
import { getGeminiDailyTrend, getGeminiUsageSummary } from "../lib/ipc";
import type { DailyTrend, GeminiUsageSummary } from "../lib/types";

const PERIODS: { label: string; hours: number }[] = [
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
  { label: "30d", hours: 24 * 30 },
];

type Currency = "USD" | "INR";

type FxRate = {
  base: "USD";
  quote: "INR";
  rate: number;
  date: string;
  fetchedAt: number;
};

const CURRENCY_STORAGE_KEY = "trace.geminiUsage.currency";
const FX_STORAGE_KEY = "trace.fx.usdInr";

function readStoredCurrency(): Currency {
  if (typeof window === "undefined") return "USD";
  return window.localStorage.getItem(CURRENCY_STORAGE_KEY) === "INR"
    ? "INR"
    : "USD";
}

function readStoredFxRate(): FxRate | null {
  if (typeof window === "undefined") return null;
  try {
    const parsed = JSON.parse(window.localStorage.getItem(FX_STORAGE_KEY) || "null");
    if (
      parsed?.base === "USD" &&
      parsed?.quote === "INR" &&
      typeof parsed.rate === "number" &&
      parsed.rate > 0 &&
      typeof parsed.date === "string" &&
      typeof parsed.fetchedAt === "number"
    ) {
      return parsed;
    }
  } catch {
    // Ignore malformed cache.
  }
  return null;
}

async function fetchUsdInrRate(): Promise<FxRate> {
  const response = await fetch(
    "https://api.frankfurter.dev/v2/rates?base=USD&quotes=INR",
  );
  if (!response.ok) {
    throw new Error(`FX rate request failed: ${response.status}`);
  }
  const data = await response.json();
  const row = Array.isArray(data) ? data[0] : data;
  const rate = Number(row?.rate ?? row?.rates?.INR);
  const date = String(row?.date ?? "");
  if (!Number.isFinite(rate) || rate <= 0 || !date) {
    throw new Error("FX rate response was missing USD/INR data");
  }
  return {
    base: "USD",
    quote: "INR",
    rate,
    date,
    fetchedAt: Date.now(),
  };
}

function shouldRefreshFxRate(rate: FxRate | null): boolean {
  if (!rate) return true;
  const twelveHoursMs = 12 * 60 * 60 * 1000;
  return Date.now() - rate.fetchedAt > twelveHoursMs;
}

function formatCost(usd: number, currency: Currency, fxRate: FxRate | null): string {
  if (currency === "INR") {
    if (!fxRate) return "₹--";
    const inr = usd * fxRate.rate;
    if (inr < 1) return `₹${inr.toFixed(3)}`;
    if (inr < 100) return `₹${inr.toFixed(2)}`;
    return `₹${inr.toFixed(0)}`;
  }
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

function humanizeFeature(name: string): string {
  return name.replace(/_/g, " ");
}

const TREND_PERIODS: { label: string; days: number }[] = [
  { label: "7d", days: 7 },
  { label: "30d", days: 30 },
  { label: "90d", days: 90 },
];

export function GeminiUsagePanel() {
  const [hours, setHours] = useState<number>(24);
  const [trendDays, setTrendDays] = useState<number>(30);
  const [currency, setCurrencyState] = useState<Currency>(() => readStoredCurrency());
  const [fxRate, setFxRate] = useState<FxRate | null>(() => readStoredFxRate());
  const [fxError, setFxError] = useState(false);
  const [summary, setSummary] = useState<GeminiUsageSummary | null>(null);
  const [trend, setTrend] = useState<DailyTrend | null>(null);
  const [loading, setLoading] = useState(false);

  const setCurrency = (next: Currency) => {
    setCurrencyState(next);
    window.localStorage.setItem(CURRENCY_STORAGE_KEY, next);
  };

  const loadFxRate = useCallback(
    async (force = false) => {
      if (currency !== "INR") return;
      const cached = readStoredFxRate();
      if (!force && cached && !shouldRefreshFxRate(cached)) {
        setFxRate(cached);
        setFxError(false);
        return;
      }
      try {
        const next = await fetchUsdInrRate();
        window.localStorage.setItem(FX_STORAGE_KEY, JSON.stringify(next));
        setFxRate(next);
        setFxError(false);
      } catch {
        setFxRate(cached);
        setFxError(true);
      }
    },
    [currency],
  );

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [data, daily] = await Promise.all([
        getGeminiUsageSummary(hours),
        getGeminiDailyTrend(trendDays),
      ]);
      setSummary(data);
      setTrend(daily);
      await loadFxRate(true);
    } catch {
      // ipc wrapper toasts
    } finally {
      setLoading(false);
    }
  }, [hours, trendDays, loadFxRate]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    void loadFxRate();
  }, [loadFxRate]);

  const cacheRate = useMemo(() => {
    if (!summary || summary.total_prompt_tokens === 0) return 0;
    return Math.round(
      (summary.total_cached_tokens / summary.total_prompt_tokens) * 100,
    );
  }, [summary]);

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-zinc-100 text-zinc-600">
            <Coins size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">
              Gemini usage
            </h2>
            <p className="text-[11px] text-zinc-400">
              Token consumption and approximate cost.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex rounded-lg border border-zinc-200 bg-zinc-50 p-0.5 text-[11px]">
            {(["USD", "INR"] as const).map((c) => (
              <button
                className={`rounded-md px-2 py-1 font-medium transition-colors ${
                  currency === c
                    ? "bg-white text-zinc-900 shadow-sm"
                    : "text-zinc-500 hover:text-zinc-700"
                }`}
                key={c}
                onClick={() => setCurrency(c)}
                type="button"
              >
                {c === "USD" ? "$" : "₹"}
              </button>
            ))}
          </div>
          <div className="flex rounded-lg border border-zinc-200 bg-zinc-50 p-0.5 text-[11px]">
            {PERIODS.map((p) => (
              <button
                className={`rounded-md px-2 py-1 font-medium transition-colors ${
                  hours === p.hours
                    ? "bg-white text-zinc-900 shadow-sm"
                    : "text-zinc-500 hover:text-zinc-700"
                }`}
                key={p.label}
                onClick={() => setHours(p.hours)}
                type="button"
              >
                {p.label}
              </button>
            ))}
          </div>
          <button
            aria-label="Refresh usage"
            className="btn h-8 w-8 px-0"
            disabled={loading}
            onClick={() => void load()}
            type="button"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      {!summary ? (
        <div className="space-y-2 p-5">
          {Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="h-12 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : summary.total_calls === 0 ? (
        <div className="px-5 py-10 text-center">
          <Coins className="mx-auto mb-2 text-zinc-200" size={24} />
          <p className="text-sm text-zinc-400">No Gemini calls yet.</p>
          <p className="mt-1 text-xs text-zinc-300">
            Usage will appear here as the AI works.
          </p>
        </div>
      ) : (
        <div className="px-5 py-4 space-y-4">
          <div className="grid grid-cols-4 gap-2">
            <Stat
              label="Cost"
              value={formatCost(summary.total_cost_usd, currency, fxRate)}
              hint="estimate"
            />
            <Stat label="Calls" value={`${summary.total_calls}`} hint={summary.error_calls > 0 ? `${summary.error_calls} err` : undefined} />
            <Stat label="Tokens" value={formatTokens(summary.total_tokens)} />
            <Stat label="Cache" value={`${cacheRate}%`} hint={`${formatTokens(summary.total_cached_tokens)} cached`} />
          </div>

          <TrendChart
            trend={trend}
            trendDays={trendDays}
            setTrendDays={setTrendDays}
            currency={currency}
            fxRate={fxRate}
          />

          {summary.by_feature.length > 0 && (
            <div>
              <p className="page-kicker mb-2">By feature</p>
              <div className="space-y-1">
                {summary.by_feature.map((f) => (
                  <UsageRow
                    key={f.feature}
                    label={humanizeFeature(f.feature)}
                    secondary={`${f.calls} call${f.calls === 1 ? "" : "s"} · ${formatTokens(f.total_tokens)} tok`}
                    cost={f.cost_usd}
                    currency={currency}
                    fxRate={fxRate}
                    weightShare={
                      summary.total_cost_usd > 0
                        ? f.cost_usd / summary.total_cost_usd
                        : 0
                    }
                  />
                ))}
              </div>
            </div>
          )}

          {summary.by_model.length > 0 && (
            <div>
              <p className="page-kicker mb-2">By model</p>
              <div className="space-y-1">
                {summary.by_model.map((m) => (
                  <UsageRow
                    key={m.model}
                    label={m.model}
                    secondary={`${m.calls} call${m.calls === 1 ? "" : "s"} · ${formatTokens(m.total_tokens)} tok${m.cached_tokens > 0 ? ` · ${formatTokens(m.cached_tokens)} cached` : ""}`}
                    cost={m.cost_usd}
                    currency={currency}
                    fxRate={fxRate}
                    weightShare={
                      summary.total_cost_usd > 0
                        ? m.cost_usd / summary.total_cost_usd
                        : 0
                    }
                    mono
                  />
                ))}
              </div>
            </div>
          )}

          <p className="pt-2 text-[10px] text-zinc-300">
            {currency === "INR" && fxRate
              ? `Costs are stored in USD. INR uses the cached USD→INR rate ${fxRate.rate.toFixed(2)} from ${fxRate.date}${fxError ? " (offline fallback)" : ""}.`
              : "Cost is an approximation based on published Gemini Flash/Pro rates. Treat as a guide, not a billing source of truth."}
          </p>
        </div>
      )}
    </section>
  );
}

function Stat({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3">
      <p className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
        {label}
      </p>
      <p className="mt-1 text-lg font-bold text-zinc-950">{value}</p>
      {hint && <p className="text-[10px] text-zinc-400">{hint}</p>}
    </div>
  );
}

function TrendChart({
  trend,
  trendDays,
  setTrendDays,
  currency,
  fxRate,
}: {
  trend: DailyTrend | null;
  trendDays: number;
  setTrendDays: (n: number) => void;
  currency: Currency;
  fxRate: FxRate | null;
}) {
  // Fill missing days with zero so the x-axis is continuous.
  const filledPoints = useMemo(() => {
    if (!trend || trend.buckets.length === 0) {
      return [] as { date: string; cost_usd: number }[];
    }
    const byDate = new Map<string, number>(
      trend.buckets.map((b) => [b.date, b.cost_usd]),
    );
    const today = new Date();
    today.setUTCHours(0, 0, 0, 0);
    const out: { date: string; cost_usd: number }[] = [];
    for (let i = trendDays - 1; i >= 0; i--) {
      const d = new Date(today);
      d.setUTCDate(d.getUTCDate() - i);
      const key = d.toISOString().slice(0, 10);
      out.push({ date: key, cost_usd: byDate.get(key) ?? 0 });
    }
    return out;
  }, [trend, trendDays]);

  const maxCost = useMemo(
    () => Math.max(...filledPoints.map((p) => p.cost_usd), 0),
    [filledPoints],
  );

  const W = 320;
  const H = 80;
  const PAD_X = 4;
  const PAD_Y = 6;

  const path = useMemo(() => {
    if (filledPoints.length === 0) return "";
    if (filledPoints.length === 1) {
      const x = W / 2;
      const y = H - PAD_Y;
      return `M ${x} ${y} L ${x} ${y}`;
    }
    const step = (W - 2 * PAD_X) / (filledPoints.length - 1);
    return filledPoints
      .map((p, i) => {
        const x = PAD_X + i * step;
        const y =
          maxCost > 0
            ? H - PAD_Y - ((H - 2 * PAD_Y) * p.cost_usd) / maxCost
            : H - PAD_Y;
        return `${i === 0 ? "M" : "L"} ${x.toFixed(1)} ${y.toFixed(1)}`;
      })
      .join(" ");
  }, [filledPoints, maxCost]);

  const fillPath = useMemo(() => {
    if (filledPoints.length < 2 || !path) return "";
    const step = (W - 2 * PAD_X) / (filledPoints.length - 1);
    const lastX = PAD_X + (filledPoints.length - 1) * step;
    return `${path} L ${lastX.toFixed(1)} ${H - PAD_Y} L ${PAD_X} ${H - PAD_Y} Z`;
  }, [filledPoints, path]);

  return (
    <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3">
      <div className="mb-2 flex items-center justify-between">
        <div>
          <p className="page-kicker">Cost over time</p>
          <p className="text-[11px] text-zinc-400">
            {trend
              ? `${formatCost(trend.total_cost_usd, currency, fxRate)} across last ${trendDays} day${trendDays === 1 ? "" : "s"}`
              : "Loading…"}
          </p>
        </div>
        <div className="flex rounded-lg border border-zinc-200 bg-white p-0.5 text-[11px]">
          {TREND_PERIODS.map((p) => (
            <button
              key={p.label}
              className={`rounded-md px-2 py-1 font-medium transition-colors ${
                trendDays === p.days
                  ? "bg-zinc-100 text-zinc-900"
                  : "text-zinc-500 hover:text-zinc-700"
              }`}
              onClick={() => setTrendDays(p.days)}
              type="button"
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>
      {filledPoints.length === 0 || maxCost === 0 ? (
        <div className="flex h-[80px] items-center justify-center text-[11px] text-zinc-300">
          No spend in the last {trendDays} day{trendDays === 1 ? "" : "s"}.
        </div>
      ) : (
        <svg
          viewBox={`0 0 ${W} ${H}`}
          preserveAspectRatio="none"
          className="h-[80px] w-full"
        >
          {fillPath && (
            <path d={fillPath} className="fill-sky-100/60 stroke-none" />
          )}
          <path
            d={path}
            className="fill-none stroke-sky-500"
            strokeWidth="1.5"
            strokeLinejoin="round"
            strokeLinecap="round"
          />
          {filledPoints.map((p, i) => {
            const step =
              filledPoints.length > 1
                ? (W - 2 * PAD_X) / (filledPoints.length - 1)
                : 0;
            const x = PAD_X + i * step;
            const y =
              maxCost > 0
                ? H - PAD_Y - ((H - 2 * PAD_Y) * p.cost_usd) / maxCost
                : H - PAD_Y;
            if (p.cost_usd === 0) return null;
            return (
              <circle
                key={p.date}
                cx={x}
                cy={y}
                r="2"
                className="fill-sky-500"
              >
                <title>
                  {p.date}: {formatCost(p.cost_usd, currency, fxRate)}
                </title>
              </circle>
            );
          })}
        </svg>
      )}
    </div>
  );
}

function UsageRow({
  label,
  secondary,
  cost,
  currency,
  fxRate,
  weightShare,
  mono,
}: {
  label: string;
  secondary: string;
  cost: number;
  currency: Currency;
  fxRate: FxRate | null;
  weightShare: number;
  mono?: boolean;
}) {
  return (
    <div className="rounded-lg border border-zinc-100 bg-white px-3 py-2">
      <div className="flex items-baseline justify-between gap-2">
        <span
          className={`text-[12px] font-medium text-zinc-900 ${mono ? "font-mono" : ""}`}
        >
          {label}
        </span>
        <span className="text-[12px] tabular-nums text-zinc-700">
          {formatCost(cost, currency, fxRate)}
        </span>
      </div>
      <div className="mt-1 flex items-center gap-2">
        <div className="h-1 flex-1 overflow-hidden rounded-full bg-zinc-100">
          <div
            className="h-full bg-sky-400"
            style={{ width: `${Math.max(2, Math.round(weightShare * 100))}%` }}
          />
        </div>
        <span className="shrink-0 text-[10px] text-zinc-400">{secondary}</span>
      </div>
    </div>
  );
}
