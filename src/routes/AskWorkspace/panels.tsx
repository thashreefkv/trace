// Shared panel primitives used by the side drawer and the ingest panel.
// Extracted from AskWorkspace.tsx so submodules can import without depending
// on the route file directly.

import type { ReactNode } from "react";

export function PanelSurface({ children }: { children: ReactNode }) {
  return (
    <div className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] divide-y divide-zinc-100">
      {children}
    </div>
  );
}

export function PanelRow({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={["px-4 py-4", className].filter(Boolean).join(" ")}>{children}</div>;
}

export function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-xl border border-zinc-100 bg-zinc-50 px-3 py-2.5">
      <p className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">{label}</p>
      <p className="mt-0.5 text-[15px] font-semibold text-zinc-950">{value}</p>
    </div>
  );
}
