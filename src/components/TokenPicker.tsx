import { useMemo, useRef, useState } from "react";
import { Check, Plus, Search, X } from "lucide-react";

interface TokenPickerProps<T> {
  items: T[];
  selectedIds: string[];
  getId: (item: T) => string;
  getLabel: (item: T) => string;
  getSubLabel?: (item: T) => string | null | undefined;
  placeholder?: string;
  onCreate?: (name: string) => Promise<void>;
  onToggle: (id: string) => void;
  singleSelect?: boolean;
}

export function TokenPicker<T>({
  items,
  selectedIds,
  getId,
  getLabel,
  getSubLabel,
  placeholder = "Search…",
  onCreate,
  onToggle,
  singleSelect = false,
}: TokenPickerProps<T>) {
  const [query, setQuery] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const selectedItems = useMemo(
    () => items.filter((item) => selectedIds.includes(getId(item))),
    [items, selectedIds, getId],
  );

  const filtered = useMemo(() => {
    if (!query.trim()) return items;
    const lower = query.toLowerCase();
    return items.filter((item) => getLabel(item).toLowerCase().includes(lower));
  }, [items, query, getLabel]);

  const hasQuery = query.trim().length > 0;

  const showCreate =
    !!onCreate &&
    hasQuery &&
    !filtered.some((item) => getLabel(item).toLowerCase() === query.trim().toLowerCase());

  async function handleCreate() {
    if (!onCreate || !query.trim() || isCreating) return;
    setIsCreating(true);
    try {
      await onCreate(query.trim());
      setQuery("");
      if (!singleSelect) inputRef.current?.focus();
    } finally {
      setIsCreating(false);
    }
  }

  function handleToggle(id: string) {
    if (singleSelect) {
      // Select the new item (deselect others)
      if (!selectedIds.includes(id)) {
        selectedIds.forEach((prev) => onToggle(prev));
        onToggle(id);
      }
      setQuery("");
      setIsOpen(false);
    } else {
      onToggle(id);
      setQuery("");
      inputRef.current?.focus();
    }
  }

  return (
    <div className="space-y-2">
      {/* Selected chips */}
      {selectedItems.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedItems.map((item) => (
            <span
              className="flex items-center gap-1 rounded-md bg-zinc-100 px-2 py-1 text-xs font-medium text-zinc-700"
              key={getId(item)}
            >
              {getLabel(item)}
              <button
                className="ml-0.5 rounded text-zinc-400 hover:text-zinc-700"
                onClick={() => onToggle(getId(item))}
                type="button"
              >
                <X size={10} />
              </button>
            </span>
          ))}
        </div>
      )}

      {/* Search input */}
      <div className="relative">
        <Search
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          size={13}
        />
        <input
          ref={inputRef}
          className="field-control h-9 pl-9 text-sm"
          onBlur={() => setTimeout(() => setIsOpen(false), 160)}
          onChange={(e) => {
            setQuery(e.currentTarget.value);
            setIsOpen(true);
          }}
          onFocus={() => setIsOpen(true)}
          placeholder={placeholder}
          value={query}
        />
      </div>

      {/* Dropdown */}
      {isOpen && (filtered.length > 0 || showCreate) && (
        <div className="relative z-10">
          <div className="absolute top-0 w-full overflow-hidden rounded-xl border border-zinc-200 bg-white shadow-[0_4px_20px_rgba(0,0,0,0.09)]">
            <div className="max-h-52 overflow-y-auto">
              {filtered.length === 0 && !showCreate ? (
                <p className="px-3 py-3 text-xs text-zinc-400">No matches.</p>
              ) : null}

              {filtered.map((item) => {
                const isSelected = selectedIds.includes(getId(item));
                return (
                  <button
                    className="flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm transition-colors hover:bg-zinc-50"
                    key={getId(item)}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      handleToggle(getId(item));
                    }}
                    type="button"
                  >
                    <span
                      className={[
                        "flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors",
                        singleSelect ? "rounded-full" : "rounded",
                        isSelected
                          ? "border-zinc-900 bg-zinc-900"
                          : "border-zinc-300 bg-white",
                      ].join(" ")}
                    >
                      {isSelected && <Check className="text-white" size={10} />}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate font-medium text-zinc-900">
                        {getLabel(item)}
                      </span>
                      {getSubLabel && getSubLabel(item) ? (
                        <span className="block truncate text-xs text-zinc-400">
                          {getSubLabel(item)}
                        </span>
                      ) : null}
                    </span>
                  </button>
                );
              })}
            </div>

            {showCreate && (
              <button
                className="flex w-full items-center gap-2 border-t border-zinc-100 px-3 py-2.5 text-sm text-sky-600 transition-colors hover:bg-sky-50"
                disabled={isCreating}
                onMouseDown={(e) => {
                  e.preventDefault();
                  void handleCreate();
                }}
                type="button"
              >
                <Plus size={13} />
                {isCreating ? "Creating…" : `Create "${query.trim()}"`}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
