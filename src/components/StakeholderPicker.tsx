import { useEffect, useState } from "react";
import { Check, UserPlus, Users, X } from "lucide-react";
import { listStakeholders, createStakeholder } from "../lib/ipc";
import type { Stakeholder } from "../lib/types";

interface StakeholderPickerProps {
  selectedIds: string[];
  onChange: (ids: string[]) => void;
  placeholder?: string;
  trigger?: React.ReactNode;
}

export function StakeholderPicker({ selectedIds, onChange, placeholder = "Select stakeholders...", trigger }: StakeholderPickerProps) {
  const [stakeholders, setStakeholders] = useState<Stakeholder[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    listStakeholders().then(setStakeholders).catch(console.error);
  }, []);

  const selectedStakeholders = stakeholders.filter(s => selectedIds.includes(s.id));
  const filteredStakeholders = stakeholders.filter(s => 
    s.name.toLowerCase().includes(search.toLowerCase()) ||
    (s.role && s.role.toLowerCase().includes(search.toLowerCase()))
  );

  function toggle(id: string) {
    if (selectedIds.includes(id)) {
      onChange(selectedIds.filter(i => i !== id));
    } else {
      onChange([...selectedIds, id]);
    }
  }

  async function handleCreate() {
    if (!search.trim() || creating) return;
    setCreating(true);
    try {
      const created = await createStakeholder({ name: search.trim() });
      setStakeholders(prev => [...prev, created]);
      onChange([...selectedIds, created.id]);
      setSearch("");
    } catch (e) {
      console.error(e);
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="relative">
      {trigger ? (
        <div onClick={() => setIsOpen(true)}>{trigger}</div>
      ) : (
        <div 
          className="flex min-h-[38px] w-full flex-wrap gap-1.5 rounded-lg border border-zinc-200 bg-white px-2 py-1.5 text-sm shadow-sm transition-all focus-within:border-sky-500 focus-within:ring-1 focus-within:ring-sky-500 cursor-pointer"
          onClick={() => setIsOpen(true)}
        >
          {selectedStakeholders.length === 0 && (
            <span className="text-zinc-400 py-0.5 px-1">{placeholder}</span>
          )}
          {selectedStakeholders.map(s => (
            <span key={s.id} className="inline-flex items-center gap-1 rounded-md bg-sky-50 px-2 py-0.5 text-[12px] font-medium text-sky-700 border border-sky-100">
              {s.name}
            </span>
          ))}
        </div>
      )}

      {isOpen && (
        <>
          <div className="fixed inset-0 z-[100] bg-zinc-950/20 backdrop-blur-[2px]" onClick={() => setIsOpen(false)} />
          <div className="fixed left-1/2 top-[15vh] z-[101] w-full max-w-md -translate-x-1/2 overflow-hidden rounded-2xl border border-zinc-200 bg-white shadow-2xl shadow-zinc-950/20 motion-panel">
            <div className="border-b border-zinc-100 bg-zinc-50/50 px-4 py-3 flex items-center justify-between">
              <span className="text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-500">Assign Stakeholders</span>
              <button onClick={() => setIsOpen(false)} className="text-zinc-400 hover:text-zinc-600 transition-colors">
                <X size={16} />
              </button>
            </div>
            
            <div className="p-4">
              <div className="relative mb-4">
                <input
                  autoFocus
                  type="text"
                  className="w-full rounded-xl border border-zinc-200 bg-zinc-50 px-4 py-2.5 text-[14px] placeholder:text-zinc-400 outline-none focus:border-sky-500 focus:ring-1 focus:ring-sky-500 transition-all"
                  placeholder="Search or create stakeholder..."
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && search && filteredStakeholders.length === 0) {
                      e.preventDefault();
                      handleCreate();
                    }
                  }}
                />
                {search && filteredStakeholders.length === 0 && (
                  <button
                    type="button"
                    onClick={handleCreate}
                    disabled={creating}
                    className="absolute right-2 top-1.5 flex items-center gap-1.5 rounded-lg bg-sky-600 px-2.5 py-1.5 text-[11px] font-bold text-white hover:bg-sky-500 transition-all shadow-sm"
                  >
                    <UserPlus size={12} />
                    Create
                  </button>
                )}
              </div>

              <div className="max-h-64 overflow-auto rounded-xl border border-zinc-100 divide-y divide-zinc-50">
                {filteredStakeholders.map(s => {
                  const isSelected = selectedIds.includes(s.id);
                  return (
                    <button
                      key={s.id}
                      type="button"
                      onClick={() => toggle(s.id)}
                      className="flex w-full items-center justify-between px-4 py-3 text-left text-[13px] hover:bg-sky-50 group transition-colors"
                    >
                      <div className="min-w-0 flex-1">
                        <span className={`block font-semibold ${isSelected ? "text-sky-700" : "text-zinc-900"}`}>{s.name}</span>
                        {s.role && <span className="block truncate text-[11px] text-zinc-500">{s.role}</span>}
                      </div>
                      <div className={`flex h-5 w-5 items-center justify-center rounded-full border transition-all ${isSelected ? "bg-sky-600 border-sky-600 text-white" : "border-zinc-200 bg-white group-hover:border-sky-300"}`}>
                        {isSelected && <Check size={12} strokeWidth={3} />}
                      </div>
                    </button>
                  );
                })}
                {filteredStakeholders.length === 0 && !search && (
                  <div className="px-3 py-10 text-center">
                    <Users size={24} className="mx-auto mb-2 text-zinc-200" />
                    <p className="text-[12px] text-zinc-400 font-medium">No stakeholders registered yet</p>
                  </div>
                )}
              </div>
            </div>
            
            <div className="bg-zinc-50/50 px-4 py-3 text-center border-t border-zinc-100">
               <button 
                onClick={() => setIsOpen(false)}
                className="btn btn-primary w-full h-10 rounded-xl"
               >
                 Done
               </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
