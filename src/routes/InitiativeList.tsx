import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { AlertTriangle, ArrowRight, Plus, RefreshCw } from "lucide-react";
import { motion } from "framer-motion";
import { createInitiative, listInitiatives } from "../lib/ipc";
import type { CreateInitiativeInput, Initiative } from "../lib/types";
import { formatDateTime } from "../lib/format";
import { InitiativeForm } from "../components/InitiativeForm";
import { StatePill } from "../components/StatePill";
import { InitiativeIcon } from "../components/InitiativeIcon";
import { Dialog } from "../components/ui/Dialog";
import { EmptyState } from "../components/EmptyState";

const MountainIllustration = () => (
  <svg viewBox="0 0 200 140" fill="none" className="mx-auto mb-5 h-28 w-auto text-zinc-200" xmlns="http://www.w3.org/2000/svg">
    <line x1="20" y1="115" x2="180" y2="115" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <path d="M20 115 Q45 90 68 100 Q82 106 95 115" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <path d="M105 115 Q118 106 132 100 Q155 90 180 115" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <path d="M58 115 L100 28 L142 115" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <path d="M82 80 L100 52 L118 80" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" strokeDasharray="3 4" />
    <line x1="100" y1="28" x2="100" y2="14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <polygon points="100,14 114,19 100,24" stroke="currentColor" strokeWidth="1.25" fill="currentColor" />
    <circle cx="152" cy="44" r="2" fill="currentColor" />
    <circle cx="166" cy="33" r="1.5" fill="currentColor" />
    <circle cx="142" cy="30" r="1" fill="currentColor" />
  </svg>
);

export function InitiativeList() {
  const navigate = useNavigate();
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);

  useEffect(() => {
    void loadInitiatives();
  }, []);

  const liveCount = useMemo(
    () => initiatives.filter((i) => i.status === "live").length,
    [initiatives],
  );

  async function loadInitiatives() {
    try {
      setError(null);
      setIsLoading(true);
      setInitiatives(await listInitiatives());
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleCreate(input: CreateInitiativeInput) {
    try {
      setError(null);
      setIsSaving(true);
      const created = await createInitiative(input);
      setInitiatives((current) => [created, ...current]);
      setIsCreateModalOpen(false);
      navigate(`/initiatives/${created.id}`);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="mx-auto min-h-screen max-w-4xl px-5 py-6">
      <section className="min-w-0">
        <div className="mb-6 flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="page-kicker">Strategic work</p>
            <h1 className="text-2xl font-semibold tracking-normal text-zinc-950">Initiatives</h1>
            {!isLoading && (
              <p className="mt-1 text-sm text-zinc-400">
                <span className="font-semibold text-zinc-700">{liveCount}</span> live
                <span className="mx-2 text-zinc-200">·</span>
                <span className="font-semibold text-zinc-700">{initiatives.length}</span> total
              </p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button className="btn h-8 w-8 px-0" onClick={() => void loadInitiatives()} title="Refresh" type="button">
              <RefreshCw size={14} />
            </button>
            <button 
              className="btn btn-primary h-8" 
              onClick={() => setIsCreateModalOpen(true)}
              type="button"
            >
              <Plus size={14} className="-ml-0.5" />
              New Initiative
            </button>
          </div>
        </div>

        {liveCount > 10 && (
          <div className="mb-5 flex gap-3 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
            <AlertTriangle className="mt-0.5 shrink-0 text-amber-500" size={15} />
            <p>{liveCount} live initiatives. Park or pause at least one before trusting the active set.</p>
          </div>
        )}

        {error && <div className="mb-4 notice notice-error text-sm">{error}</div>}

        {isLoading ? (
          <div className="space-y-3">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="h-[92px] animate-pulse rounded-2xl bg-zinc-100" />
            ))}
          </div>
        ) : initiatives.length === 0 ? (
          <EmptyState
            variant="page"
            illustration={<MountainIllustration />}
            title="No initiatives yet"
            description="Group deliverables by the goal they ladder up to. Each initiative becomes a shared target for your team."
            cta={{ label: "Create your first initiative", onClick: () => setIsCreateModalOpen(true), primary: true }}
          />
        ) : (
          <div className="space-y-3">
            {initiatives.map((initiative, i) => (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                initial={{ opacity: 0, y: 8 }}
                key={initiative.id}
                transition={{ duration: 0.15, delay: Math.min(i * 0.04, 0.2), ease: "easeOut" }}
              >
                <InitiativeCard initiative={initiative} />
              </motion.div>
            ))}
          </div>
        )}
      </section>

      {/* ── Create modal ── */}
      <Dialog
        kicker="Create"
        onOpenChange={setIsCreateModalOpen}
        open={isCreateModalOpen}
        size="lg"
        title="New initiative"
      >
        <Dialog.Body>
          <div>
            <p className="mb-1 text-xs text-zinc-400">{liveCount} of 10 live slots used</p>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-zinc-100">
              <motion.div
                animate={{ width: `${Math.min((liveCount / 10) * 100, 100)}%` }}
                className={`h-full rounded-full ${
                  liveCount >= 10 ? "bg-red-400" : liveCount >= 7 ? "bg-amber-400" : "bg-emerald-400"
                }`}
                initial={false}
                transition={{ duration: 0.5, ease: "easeOut" }}
              />
            </div>
          </div>
          <InitiativeForm
            isSubmitting={isSaving}
            onSubmit={handleCreate}
            submitLabel="Create initiative"
            onCancel={() => setIsCreateModalOpen(false)}
          />
        </Dialog.Body>
      </Dialog>
    </div>
  );
}

function InitiativeCard({ initiative }: { initiative: Initiative }) {
  return (
    <Link
      className="group flex overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] transition-all duration-150 hover:-translate-y-0.5 hover:shadow-[0_4px_20px_rgba(0,0,0,0.09)]"
      to={`/initiatives/${initiative.id}`}
    >
      {/* Body */}
      <div className="min-w-0 flex-1 px-4 py-4">
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <StatePill kind="initiative" status={initiative.status} />
          <span className="text-xs text-zinc-400">{formatDateTime(initiative.updated_at)}</span>
        </div>
        <h2 className="flex items-center gap-2 text-[15px] font-semibold text-zinc-950 transition-colors group-hover:text-zinc-700">
          <InitiativeIcon name={initiative.icon} color={initiative.icon_color} size={16} />
          {initiative.title}
        </h2>
        {initiative.framing && (
          <p className="mt-1 line-clamp-2 text-sm leading-6 text-zinc-500">{initiative.framing}</p>
        )}
      </div>

      {/* Arrow */}
      <div className="flex items-center px-4">
        <ArrowRight
          className="text-zinc-200 transition-all duration-150 group-hover:translate-x-0.5 group-hover:text-zinc-400"
          size={17}
        />
      </div>
    </Link>
  );
}
