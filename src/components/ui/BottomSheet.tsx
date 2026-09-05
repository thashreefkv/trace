import * as RadixDialog from "@radix-ui/react-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { MOTION } from "../../lib/motion";

interface BottomSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title?: string;
  description?: string;
  /** Max height of the sheet, default "90vh" */
  height?: string;
  children: ReactNode;
}

export function BottomSheet({
  open,
  onOpenChange,
  title,
  description,
  height = "90vh",
  children,
}: BottomSheetProps) {
  return (
    <RadixDialog.Root onOpenChange={onOpenChange} open={open}>
      <RadixDialog.Portal>
        <AnimatePresence>
          {open && (
            <>
              <RadixDialog.Overlay asChild forceMount>
                <motion.div
                  animate={{ opacity: 1 }}
                  className="fixed inset-0 z-40 bg-zinc-950/20 backdrop-blur-sm"
                  exit={{ opacity: 0 }}
                  initial={{ opacity: 0 }}
                  transition={{ duration: MOTION.short }}
                />
              </RadixDialog.Overlay>
              <RadixDialog.Content asChild forceMount>
                <motion.div
                  animate={{ opacity: 1, y: 0 }}
                  className="fixed bottom-0 left-0 right-0 z-40 flex flex-col overflow-hidden rounded-t-2xl border-t border-zinc-100 bg-white shadow-2xl outline-none"
                  exit={{ opacity: 0, y: "100%" }}
                  initial={{ opacity: 0, y: "100%" }}
                  style={{ maxHeight: height }}
                  transition={{ duration: MOTION.short }}
                >
                  {/* Drag handle */}
                  <div className="flex shrink-0 justify-center pt-3">
                    <div className="h-1 w-10 rounded-full bg-zinc-200" />
                  </div>
                  {(title || description) && (
                    <div className="flex shrink-0 items-start justify-between gap-4 px-5 pb-3 pt-3">
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
