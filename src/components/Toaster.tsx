import { Toaster as SonnerToaster } from "sonner";

export function Toaster() {
  return (
    <SonnerToaster
      position="bottom-right"
      duration={4000}
      visibleToasts={4}
      closeButton
      toastOptions={{
        unstyled: false,
        classNames: {
          toast:
            "!rounded-xl !border !border-zinc-100 !bg-white !shadow-[0_2px_12px_rgba(0,0,0,0.06)] !text-zinc-900 !font-normal",
          title: "!text-sm !font-medium !text-zinc-900",
          description: "!text-xs !text-zinc-500 !mt-0.5",
          actionButton:
            "!bg-zinc-900 !text-white !text-[11px] !font-semibold !rounded-md !px-2.5 !py-1 hover:!bg-zinc-700",
          cancelButton:
            "!bg-zinc-100 !text-zinc-700 !text-[11px] !font-semibold !rounded-md !px-2.5 !py-1",
          closeButton:
            "!bg-white !border !border-zinc-200 !text-zinc-400 hover:!text-zinc-700",
          success: "!border-emerald-100",
          error: "!border-red-100",
          warning: "!border-amber-100",
          info: "!border-sky-100",
        },
      }}
    />
  );
}
