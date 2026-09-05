import type { GmailSyncSettings, GmailSyncSettingsInput } from "../lib/types";

interface GmailSyncSettingsControlsProps {
  compact?: boolean;
  disabled?: boolean;
  settings: GmailSyncSettings;
  onChange: (input: GmailSyncSettingsInput) => void;
}

export function GmailSyncSettingsControls({
  compact = false,
  disabled = false,
  settings,
  onChange,
}: GmailSyncSettingsControlsProps) {
  return (
    <div className="space-y-4">
      <div className={compact ? "grid gap-3 sm:grid-cols-2" : "grid gap-3 sm:grid-cols-2 xl:grid-cols-4"}>
        <NumberControl
          disabled={disabled}
          label="Sync interval"
          max={168}
          min={1}
          onChange={(sync_interval_hours) => onChange({ sync_interval_hours })}
          unit="hours"
          value={settings.sync_interval_hours}
        />
        <NumberControl
          disabled={disabled}
          label="Thread cap"
          max={500}
          min={10}
          onChange={(max_threads_per_sync) => onChange({ max_threads_per_sync })}
          unit="threads / sync"
          value={settings.max_threads_per_sync}
        />
        <NumberControl
          disabled={disabled}
          label="Notification poll"
          min={1}
          onChange={(notification_poll_minutes) => onChange({ notification_poll_minutes })}
          unit="minutes"
          value={settings.notification_poll_minutes}
        />
        <NumberControl
          disabled={disabled}
          label="AI analysis cap"
          max={25}
          min={0}
          onChange={(auto_analyze_limit) => onChange({ auto_analyze_limit })}
          unit="threads / sync"
          value={settings.auto_analyze_limit}
        />
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        <ToggleControl
          checked={settings.sync_enabled}
          disabled={disabled}
          hint="Keep local mail data fresh in the background."
          label="Background sync"
          onChange={(sync_enabled) => onChange({ sync_enabled })}
        />
        <ToggleControl
          checked={settings.notify_new_mail}
          disabled={disabled}
          hint="Show desktop notifications after local sync."
          label="Desktop notifications"
          onChange={(notify_new_mail) => onChange({ notify_new_mail })}
        />
        <ToggleControl
          checked={settings.include_sent}
          disabled={disabled}
          hint="Let local analysis learn from messages you send."
          label="Analyze sent mail"
          onChange={(include_sent) => onChange({ include_sent })}
        />
        <ToggleControl
          checked={settings.include_drafts}
          disabled={disabled}
          hint="Keep draft activity visible to Work Mail."
          label="Monitor drafts"
          onChange={(include_drafts) => onChange({ include_drafts })}
        />
        <ToggleControl
          checked={settings.backfill_enabled}
          disabled={disabled}
          hint="Pull older history while the mailbox is still warming up."
          label="Backward sync"
          onChange={(backfill_enabled) => onChange({ backfill_enabled })}
        />
        <ToggleControl
          checked={settings.relevance_filter_enabled}
          disabled={disabled}
          hint="Skip low-signal threads before they enter the work view."
          label="Only relevant mail"
          onChange={(relevance_filter_enabled) => onChange({ relevance_filter_enabled })}
        />
        <ToggleControl
          checked={settings.auto_analyze_enabled}
          disabled={disabled}
          hint="Read synced threads and save AI analysis automatically."
          label="AI read and save analysis"
          onChange={(auto_analyze_enabled) => onChange({ auto_analyze_enabled })}
        />
      </div>
    </div>
  );
}

function NumberControl({
  disabled,
  label,
  max,
  min,
  onChange,
  unit,
  value,
}: {
  disabled: boolean;
  label: string;
  max?: number;
  min: number;
  onChange: (value: number) => void;
  unit: string;
  value: number;
}) {
  return (
    <label className="space-y-1.5">
      <span className="field-label">{label}</span>
      <input
        className="field-control"
        defaultValue={value}
        disabled={disabled}
        key={`${label}:${value}`}
        max={max}
        min={min}
        onBlur={(event) => onChange(Number(event.currentTarget.value))}
        type="number"
      />
      <span className="block text-[11px] text-zinc-400">{unit}</span>
    </label>
  );
}

function ToggleControl({
  checked,
  disabled,
  hint,
  label,
  onChange,
}: {
  checked: boolean;
  disabled: boolean;
  hint: string;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-start justify-between gap-3 rounded-xl border border-zinc-200 px-3 py-2.5 text-[12px] text-zinc-700">
      <span className="min-w-0">
        <span className="block font-semibold text-zinc-800">{label}</span>
        <span className="mt-0.5 block leading-4 text-zinc-400">{hint}</span>
      </span>
      <input
        checked={checked}
        className="mt-0.5 h-4 w-4 shrink-0 accent-zinc-900"
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
        type="checkbox"
      />
    </label>
  );
}
