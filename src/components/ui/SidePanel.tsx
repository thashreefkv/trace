import * as RadixDialog from "@radix-ui/react-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { MOTION } from "../../lib/motion";

type Width = "narrow" | "default" | "wide";

const widths: Record<Width, string> = {
  narrow: "24rem",
  default: "32rem",
  wide: "48rem",
};

interface SidePanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title?: string;
  description?: string;
  width?: Width;
  children: ReactNode;
}

export function SidePanel({
  open,
  onOpenChange,
  title,
  description,
  width = "default",
  children,
}: SidePanelProps) {
  const w = widths[width];
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
                  transition={{ duration: MOTION.short }}
                />
              </RadixDialog.Overlay>
              <RadixDialog.Content asChild forceMount>
                <motion.div
                  animate={{ opacity: 1, x: 0 }}
                  className="fixed bottom-0 right-0 top-0 z-50 flex flex-col overflow-hidden border-l border-zinc-100 bg-white shadow-2xl outline-none"
                  exit={{ opacity: 0, x: 40 }}
                  initial={{ opacity: 0, x: 40 }}
                  style={{ width: w }}
                  transition={{ duration: MOTION.short }}
                >
                  {(title || description) && (
                    <div className="flex shrink-0 items-start justify-between gap-4 border-b border-zinc-100 px-5 py-4">
                      <div>
                        {title && (
                          <RadixDialog.Title className="text-sm font-semibold text-zinc-950">
                            {title}
                          </RadixDialog.Title>
                        )}
                        {description && (
                          <RadixDialog.Description className="mt-0.5 text-xs text-zinc-400">
                            {description}
                          </RadixDialog.Description>
                        )}
                      </div>
                      <RadixDialog.Close className="icon-btn shrink-0">
                        <X size={14} />
                      </RadixDialog.Close>
                    </div>
                  )}
                  <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
                </motion.div>
              </RadixDialog.Content>
            </>
          )}
        </AnimatePresence>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
