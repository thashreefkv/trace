import { toast as sonnerToast } from "sonner";

export interface ToastOptions {
  description?: string;
  duration?: number;
  /** Stable identifier — repeated toasts with the same id replace each other instead of stacking. */
  id?: string;
  action?: { label: string; onClick: () => void };
}

export const toast = {
  info(message: string, opts?: ToastOptions) {
    return sonnerToast(message, opts);
  },
  success(message: string, opts?: ToastOptions) {
    return sonnerToast.success(message, opts);
  },
  warning(message: string, opts?: ToastOptions) {
    return sonnerToast.warning(message, opts);
  },
  error(message: string, opts?: ToastOptions) {
    return sonnerToast.error(message, opts);
  },
  dismiss(id?: string | number) {
    return sonnerToast.dismiss(id);
  },
};

export function formatIpcError(command: string, raw: unknown): string {
  let message: string;
  if (typeof raw === "string") {
    message = raw;
  } else if (raw && typeof raw === "object" && "message" in raw) {
    message = String((raw as { message: unknown }).message);
  } else {
    message = String(raw);
  }
  return `${humanizeCommand(command)} failed: ${truncate(message, 240)}`;
}

function humanizeCommand(cmd: string): string {
  return cmd
    .replace(/_/g, " ")
    .replace(/\bgmail\b/g, "Gmail")
    .replace(/\bgcal\b/g, "Calendar")
    .replace(/\bmcp\b/g, "MCP");
}

function truncate(s: string, max: number): string {
  return s.length > max ? `${s.slice(0, max).trimEnd()}…` : s;
}
