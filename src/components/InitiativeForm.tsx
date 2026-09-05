import { FormEvent, useEffect, useState, useMemo } from "react";
import { Check, X, Target, Pencil, Search } from "lucide-react";
import type { CreateInitiativeInput, InitiativeStatus } from "../lib/types";
import { initiativeStatusLabels, initiativeStatusOptions } from "../lib/types";
import { AVAILABLE_ICONS, DEFAULT_MEDICAL_ICONS, type IconName } from "./InitiativeIcon";

interface InitiativeFormProps {
  initialValue?: CreateInitiativeInput;
  submitLabel: string;
  isSubmitting: boolean;
  onSubmit: (input: CreateInitiativeInput) => Promise<void>;
  onCancel?: () => void;
}

const AVAILABLE_COLORS = [
  "#ef4444", // red
  "#f97316", // orange
  "#eab308", // yellow
  "#22c55e", // green
  "#06b6d4", // cyan
  "#3b82f6", // blue
  "#6366f1", // indigo
  "#a855f7", // purple
  "#ec4899", // pink
  "#71717a", // zinc
];

const defaultValue: CreateInitiativeInput = {
  title: "",
  framing: "",
  status: "live",
  icon: "Target",
  icon_color: "#6366f1",
};

export function InitiativeForm({
  initialValue = defaultValue,
  submitLabel,
  isSubmitting,
  onSubmit,
  onCancel,
}: InitiativeFormProps) {
  const [title, setTitle] = useState(initialValue.title);
  const [framing, setFraming] = useState(initialValue.framing);
  const [status, setStatus] = useState<InitiativeStatus>(initialValue.status);
  const [icon, setIcon] = useState<IconName>((initialValue.icon as IconName) || "Target");
  const [iconColor, setIconColor] = useState(initialValue.icon_color || "#6366f1");
  const [isPickerOpen, setIsPickerOpen] = useState(false);
  const [iconSearch, setIconSearch] = useState("");

  useEffect(() => {
    setTitle(initialValue.title);
    setFraming(initialValue.framing);
    setStatus(initialValue.status);
    if (initialValue.icon) setIcon(initialValue.icon as IconName);
    if (initialValue.icon_color) setIconColor(initialValue.icon_color);
  }, [initialValue]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await onSubmit({ title, framing, status, icon, icon_color: iconColor });
  }

  const SelectedIcon = AVAILABLE_ICONS[icon] || Target;

  const filteredIcons = useMemo(() => {
    if (!iconSearch) {
      return DEFAULT_MEDICAL_ICONS.map((name) => [name, AVAILABLE_ICONS[name]] as [string, React.FC<any>]);
    }
    const lower = iconSearch.toLowerCase();
    return Object.entries(AVAILABLE_ICONS)
      .filter(([name]) => name.toLowerCase().includes(lower))
      .slice(0, 150);
  }, [iconSearch]);

  return (
    <form className="space-y-4" onSubmit={handleSubmit}>
      <div className="flex gap-4">
        {/* Icon Preview with Edit button */}
        <div className="relative shrink-0 pt-6">
          <div
            className="flex h-14 w-14 items-center justify-center rounded-2xl shadow-sm"
            style={{ backgroundColor: `${iconColor}15`, color: iconColor }}
          >
            <SelectedIcon size={24} />
          </div>
          <button
            type="button"
            onClick={() => setIsPickerOpen(true)}
            className="absolute -right-2 -top-2 mt-6 flex h-6 w-6 items-center justify-center rounded-full border border-zinc-200 bg-white text-zinc-500 shadow-sm transition-colors hover:bg-zinc-50 hover:text-zinc-700"
            title="Edit icon and color"
          >
            <Pencil size={12} />
          </button>
        </div>

        <div className="flex-1 space-y-4">
          <label className="block space-y-1.5">
            <span className="field-label">Title</span>
            <input
              className="field-control"
              maxLength={120}
              onChange={(event) => setTitle(event.currentTarget.value)}
              placeholder="Content Quality Pipeline"
              required
              value={title}
            />
          </label>
        </div>
      </div>

      {isPickerOpen && (
        <div 
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/20 p-4 backdrop-blur-[2px]"
          onClick={() => setIsPickerOpen(false)}
        >
          <div 
            className="w-full max-w-sm rounded-2xl border border-zinc-100 bg-white p-5 shadow-2xl space-y-5"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold text-zinc-900">Customize Icon</h3>
              <button type="button" onClick={() => setIsPickerOpen(false)} className="rounded-full p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600 transition-colors">
                <X size={18} />
              </button>
            </div>

            <div className="space-y-3">
              <label className="block text-sm font-medium text-zinc-700">Icon Color</label>
              <div className="flex flex-wrap gap-2">
                {AVAILABLE_COLORS.map((color) => (
                  <button
                    className={`flex h-8 w-8 items-center justify-center rounded-full border-2 transition-transform ${
                      iconColor === color ? "scale-110 border-zinc-400 shadow-sm" : "border-transparent hover:scale-110 hover:shadow-sm"
                    }`}
                    key={color}
                    onClick={() => setIconColor(color)}
                    style={{ backgroundColor: color }}
                    title={color}
                    type="button"
                  />
                ))}
              </div>
            </div>

            <div className="space-y-3">
              <label className="block text-sm font-medium text-zinc-700">Select Icon</label>
              <div className="relative">
                <Search className="absolute left-3 top-2.5 text-zinc-400" size={16} />
                <input
                  type="text"
                  className="field-control pl-9"
                  placeholder="Search over 1,400 icons..."
                  value={iconSearch}
                  onChange={(e) => setIconSearch(e.target.value)}
                />
              </div>
              <div className="flex h-56 flex-wrap content-start gap-2 overflow-y-auto rounded-xl border border-zinc-100 bg-zinc-50 p-2 shadow-inner scrollbar-thin scrollbar-thumb-zinc-200">
                {filteredIcons.map(([name, IconComponent]) => {
                  if (!IconComponent) return null;
                  return (
                    <button
                      className={`flex h-10 w-10 items-center justify-center rounded-lg border transition-all ${
                        icon === name
                          ? "border-indigo-200 bg-indigo-100 text-indigo-700 shadow-sm scale-105"
                          : "border-transparent bg-white text-zinc-500 shadow-sm hover:border-zinc-200 hover:text-zinc-800"
                      }`}
                      key={name}
                      onClick={() => setIcon(name as IconName)}
                      title={name}
                      type="button"
                    >
                      <IconComponent size={20} />
                    </button>
                  );
                })}
                {filteredIcons.length === 0 && (
                  <div className="w-full py-8 text-center text-sm text-zinc-500">No icons found.</div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      <label className="block space-y-1.5 pt-2">
        <span className="field-label">Framing</span>
        <textarea
          className="field-control min-h-32"
          onChange={(event) => setFraming(event.currentTarget.value)}
          placeholder="What sustained theme of work does this initiative hold?"
          value={framing}
        />
      </label>

      <label className="block space-y-1.5">
        <span className="field-label">Status</span>
        <select
          className="field-control"
          onChange={(event) => setStatus(event.currentTarget.value as InitiativeStatus)}
          value={status}
        >
          {initiativeStatusOptions.map((option) => (
            <option key={option} value={option}>
              {initiativeStatusLabels[option]}
            </option>
          ))}
        </select>
      </label>

      <div className="flex flex-wrap items-center gap-2 pt-2">
        <button className="btn btn-primary" disabled={isSubmitting} type="submit">
          <Check aria-hidden="true" size={16} />
          {submitLabel}
        </button>
        {onCancel ? (
          <button className="btn" disabled={isSubmitting} onClick={onCancel} type="button">
            <X aria-hidden="true" size={16} />
            Cancel
          </button>
        ) : null}
      </div>
    </form>
  );
}
