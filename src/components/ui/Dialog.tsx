import * as RadixDialog from "@radix-ui/react-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { X, AlertTriangle } from "lucide-react";
import type { ReactNode } from "react";
import { MOTION } from "../../lib/motion";

type Size = "sm" | "md" | "lg" | "xl";

const widths: Record<Size, string> = {
  sm: "min(92vw,28rem)",
  md: "min(92vw,34rem)",
  lg: "min(92vw,44rem)",
  xl: "min(92vw,56rem)",
};

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  kicker?: string;
  size?: Size;
  children: ReactNode;
}

interface DialogConfirmProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  destructive?: boolean;
  loading?: boolean;
}

// Body + Footer subcomponents for layout
function DialogBody({ children }: { children: ReactNode }) {
  return <div className="space-y-4">{children}</div>;
}

function DialogFooter({ children }: { children: ReactNode }) {
  return <div className="flex items-center justify-end gap-2 pt-2">{children}</div>;
}

function DialogCancel({ children, onClick }: { children: ReactNode; onClick?: () => void }) {
  return (
    <RadixDialog.Close asChild>
      <button className="btn" onClick={onClick} type="button">
        {children}
      </button>
    </RadixDialog.Close>
  );
}

function DialogAction({
  children,
  onClick,
  variant = "primary",
  loading = false,
  disabled = false,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "danger" | "secondary";
  loading?: boolean;
  disabled?: boolean;
}) {
  const cls =
    variant === "primary"
      ? "btn btn-primary"
      : variant === "danger"
        ? "btn btn-danger"
        : "btn btn-secondary";
  return (
    <button className={cls} disabled={loading || disabled} onClick={onClick} type="button">
      {children}
    </button>
  );
}

export function Dialog({ open, onOpenChange, title, description, kicker, size = "md", children }: DialogProps) {
  const w = widths[size];
  return (
    <RadixDialog.Root onOpenChange={onOpenChange} open={open}>
      <RadixDialog.Portal>
        <AnimatePresence>
          {open && (
            <>
              <RadixDialog.Overlay asChild forceMount>
                <motion.div
                  animate={{ opacity: 1 }}
                  className="fixed inset-0 z-50 bg-zinc-950/20 backdrop-blur-sm"
                  exit={{ opacity: 0 }}
                  initial={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
                />
              </RadixDialog.Overlay>
              <RadixDialog.Content asChild forceMount>
                <motion.div
                  animate={{ opacity: 1, y: 0 }}
                  className="fixed left-1/2 top-12 z-50 max-h-[calc(100vh-6rem)] overflow-y-auto rounded-2xl border border-zinc-100 bg-white p-5 shadow-2xl outline-none"
                  exit={{ opacity: 0, y: -4 }}
                  initial={{ opacity: 0, y: 8 }}
                  style={{ width: w, translateX: "-50%" }}
                  transition={{ duration: MOTION.short }}
                >
                  <div className="mb-4 flex items-start justify-between gap-4">
                    <div>
                      {kicker && <p className="page-kicker">{kicker}</p>}
                      <RadixDialog.Title className="text-lg font-semibold text-zinc-950">
                        {title}
                      </RadixDialog.Title>
                      {description && (
                        <RadixDialog.Description className="mt-1 text-sm leading-6 text-zinc-500">
                          {description}
                        </RadixDialog.Description>
                      )}
                    </div>
                    <RadixDialog.Close className="icon-btn shrink-0">
                      <X size={14} />
                    </RadixDialog.Close>
                  </div>
                  {children}
                </motion.div>
              </RadixDialog.Content>
            </>
          )}
        </AnimatePresence>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}

// Shortcut for destructive confirmations
export function DialogConfirm({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  onConfirm,
  destructive = false,
  loading = false,
}: DialogConfirmProps) {
  return (
    <Dialog onOpenChange={onOpenChange} open={open} size="sm" title="">
      <div className="mb-4 flex items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-red-50 text-red-500">
          <AlertTriangle size={16} />
        </div>
        <div>
          <p className="text-base font-semibold text-zinc-950">{title}</p>
          {description && (
            <p className="mt-1 text-sm leading-6 text-zinc-500">{description}</p>
          )}
        </div>
      </div>
      <DialogFooter>
        <DialogCancel>{cancelLabel}</DialogCancel>
        <DialogAction
          disabled={loading}
          loading={loading}
          onClick={onConfirm}
          variant={destructive ? "danger" : "primary"}
        >
          {confirmLabel}
        </DialogAction>
      </DialogFooter>
    </Dialog>
  );
}

Dialog.Body = DialogBody;
Dialog.Footer = DialogFooter;
Dialog.Cancel = DialogCancel;
Dialog.Action = DialogAction;
