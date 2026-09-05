import { useState } from "react";
import { CloudCog, ShieldCheck } from "lucide-react";
import { driveConnect, type DriveAccount } from "../../lib/files";

interface ConnectDriveCardProps {
  onConnected: (account: DriveAccount) => void;
}

export function ConnectDriveCard({ onConnected }: ConnectDriveCardProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    try {
      setBusy(true);
      setError(null);
      const account = await driveConnect();
      onConnected(account);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white p-6">
      <div className="mb-3 inline-flex h-9 w-9 items-center justify-center rounded-xl bg-sky-50 text-sky-600">
        <CloudCog size={18} />
      </div>
      <h2 className="text-[15px] font-semibold text-zinc-950">Connect Google Drive</h2>
      <p className="mt-1 text-[12px] leading-relaxed text-zinc-500">
        Bring Drive files (Docs, Sheets, Slides, PDFs, anything) into Trace so they can sit next
        to your local files and be linked to initiatives, deliverables, tasks, and stakeholders.
      </p>
      <div className="my-4 rounded-lg border border-zinc-100 bg-zinc-50 p-3">
        <div className="flex items-start gap-2 text-[11px] leading-relaxed text-zinc-600">
          <ShieldCheck className="mt-0.5 shrink-0 text-emerald-600" size={13} />
          <p>
            Trace will request <span className="font-mono">drive.readonly</span> so you can
            browse and import existing files. We never modify your Drive. Tokens are stored
            locally on this Mac.
          </p>
        </div>
      </div>
      {error ? <div className="notice notice-error mb-3">{error}</div> : null}
      <button
        className="btn btn-primary"
        disabled={busy}
        onClick={() => void handleConnect()}
        type="button"
      >
        <CloudCog size={14} />
        {busy ? "Opening browser…" : "Connect Drive"}
      </button>
    </div>
  );
}
