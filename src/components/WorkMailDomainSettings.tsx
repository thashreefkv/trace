import { useEffect, useState } from "react";
import { AtSign } from "lucide-react";
import {
  gmailDeleteWorkMailDomain,
  gmailListWorkMailDomains,
  gmailUpsertWorkMailDomain,
} from "../lib/ipc";
import { qk } from "../lib/queries";
import { queryClient } from "../lib/queryClient";
import type { WorkMailDomain } from "../lib/types";

interface WorkMailDomainSettingsProps {
  embedded?: boolean;
}

export function WorkMailDomainSettings({ embedded = false }: WorkMailDomainSettingsProps) {
  const [domains, setDomains] = useState<WorkMailDomain[]>([]);
  const [draftDomain, setDraftDomain] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    gmailListWorkMailDomains().then(setDomains).catch(() => {});
  }, []);

  async function saveDomain(domain: string, enabled = true, note?: string | null) {
    setSaving(true);
    try {
      const saved = await gmailUpsertWorkMailDomain({
        domain,
        enabled,
        note: note ?? null,
      });
      setDomains((current) => {
        const without = current.filter((item) => item.domain !== saved.domain);
        return [...without, saved].sort((left, right) => left.domain.localeCompare(right.domain));
      });
      setDraftDomain("");
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
    } finally {
      setSaving(false);
    }
  }

  async function removeDomain(domain: string) {
    if (!confirm(`Remove @${domain} from Work Mail scope?`)) return;
    await gmailDeleteWorkMailDomain(domain);
    setDomains((current) => current.filter((item) => item.domain !== domain));
    void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
  }

  return (
    <section
      className={
        embedded
          ? "mt-5 border-t border-zinc-100 pt-4"
          : "overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
      }
      id={embedded ? undefined : "work-mail-domains"}
      tabIndex={embedded ? undefined : -1}
    >
      <div className={embedded ? "mb-2" : "flex items-start gap-3 border-b border-zinc-100 px-5 py-4"}>
        {!embedded ? (
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-sky-50 text-sky-600">
            <AtSign size={15} />
          </div>
        ) : null}
        <div>
          <p className="page-kicker">Work scope</p>
          <h4 className={embedded ? "text-sm font-semibold text-zinc-900" : "text-[13px] font-semibold text-zinc-900"}>
            Enabled sender domains
          </h4>
          {!embedded ? (
            <p className="mt-1 text-[11px] text-zinc-400">
              Decide which domains belong in Work Mail before rules and classifiers run.
            </p>
          ) : null}
        </div>
      </div>

      <div className={embedded ? "" : "px-5 py-4"}>
        {domains.length === 0 ? (
          <p className="mb-2 rounded-xl border border-zinc-100 bg-zinc-50 px-3 py-2 text-[12px] text-zinc-400">
            No sender domains are pinned to Work Mail scope yet.
          </p>
        ) : (
          <div className="space-y-1.5">
            {domains.map((domain) => (
              <div
                className="flex items-center justify-between gap-2 rounded-lg border border-zinc-100 px-3 py-2 text-xs"
                key={domain.domain}
              >
                <label className="inline-flex min-w-0 items-center gap-2">
                  <input
                    checked={domain.enabled}
                    onChange={(event) =>
                      void saveDomain(domain.domain, event.currentTarget.checked, domain.note)
                    }
                    type="checkbox"
                  />
                  <span className="truncate font-mono text-zinc-700">@{domain.domain}</span>
                </label>
                <button
                  className="text-[11px] font-medium text-zinc-400 hover:text-rose-600"
                  onClick={() => void removeDomain(domain.domain)}
                  type="button"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        )}

        <div className="mt-2 flex gap-2">
          <input
            className="field-control h-9 flex-1"
            onChange={(event) => setDraftDomain(event.currentTarget.value)}
            placeholder="example.com"
            value={draftDomain}
          />
          <button
            className="btn h-9"
            disabled={saving || !draftDomain.trim()}
            onClick={() => void saveDomain(draftDomain.trim())}
            type="button"
          >
            Add
          </button>
        </div>
      </div>
    </section>
  );
}
