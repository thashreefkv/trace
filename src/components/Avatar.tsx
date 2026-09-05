import { memo } from "react";
import { avatarColor, initials } from "../lib/avatar";

type Size = "xs" | "sm" | "md" | "lg";

interface AvatarProps {
  name: string;
  size?: Size;
  className?: string;
}

const sizeClasses: Record<Size, string> = {
  xs: "h-5 w-5 text-[9px]",
  sm: "h-9 w-9 text-[11px]",
  md: "h-10 w-10 text-[12px]",
  lg: "h-14 w-14 text-lg",
};

const shapeClasses: Record<Size, string> = {
  xs: "rounded-full",
  sm: "rounded-full",
  md: "rounded-full",
  lg: "rounded-2xl",
};

export const Avatar = memo(function Avatar({ name, size = "sm", className = "" }: AvatarProps) {
  const color = avatarColor(name);
  return (
    <div
      className={[
        "flex shrink-0 items-center justify-center font-bold",
        sizeClasses[size],
        shapeClasses[size],
        color.bg,
        color.text,
        className,
      ].join(" ")}
    >
      {initials(name)}
    </div>
  );
});
