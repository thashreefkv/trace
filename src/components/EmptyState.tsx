import type { FC, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

type Variant = "inline" | "page" | "hero";

interface Cta {
  label: string;
  onClick: () => void;
  primary?: boolean;
}

interface EmptyStateProps {
  variant?: Variant;
  icon?: LucideIcon;
  illustration?: ReactNode;
  title: string;
  description?: string;
  cta?: Cta;
}

export const EmptyState: FC<EmptyStateProps> = ({
  variant = "inline",
  icon: Icon,
  illustration,
  title,
  description,
  cta,
}) => {
  if (variant === "inline") {
    return (
      <div className="px-5 py-10 text-center">
        {Icon && <Icon className="mx-auto mb-2 text-zinc-200" size={24} />}
        <p className="text-sm text-zinc-400">{title}</p>
        {description && <p className="mt-1 text-xs text-zinc-300">{description}</p>}
        {cta && (
          <button
            className={`mt-3 ${cta.primary ? "btn btn-primary btn-sm" : "btn btn-sm"}`}
            onClick={cta.onClick}
            type="button"
          >
            {cta.label}
          </button>
        )}
      </div>
    );
  }

  if (variant === "page") {
    return (
      <div className="empty-state">
        {illustration ?? (Icon && <Icon className="mb-3 text-zinc-200" size={36} />)}
        <p className="text-sm font-semibold text-zinc-700">{title}</p>
        {description && (
          <p className="mt-1 max-w-xs text-xs leading-5 text-zinc-400">{description}</p>
        )}
        {cta && (
          <button
            className={`mt-4 ${cta.primary ? "btn btn-primary" : "btn"}`}
            onClick={cta.onClick}
            type="button"
          >
            {cta.label}
          </button>
        )}
      </div>
    );
  }

  // hero — full-width dashed container with illustration or large icon
  return (
    <div className="flex min-h-[28rem] flex-col items-center justify-center rounded-2xl border border-dashed border-zinc-200 bg-white px-8 py-16 text-center">
      {illustration ?? (Icon && <Icon className="mb-4 text-zinc-200" size={48} />)}
      <p className="mt-1 text-base font-semibold text-zinc-700">{title}</p>
      {description && (
        <p className="mt-2 max-w-sm text-sm leading-6 text-zinc-400">{description}</p>
      )}
      {cta && (
        <button
          className={`mt-5 ${cta.primary ? "btn btn-primary btn-lg" : "btn btn-lg"}`}
          onClick={cta.onClick}
          type="button"
        >
          {cta.label}
        </button>
      )}
    </div>
  );
};
