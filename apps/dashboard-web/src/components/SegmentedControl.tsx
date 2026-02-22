"use client";

import { cn } from "@/lib/utils";

type SegmentedOption = {
  value: string;
  label: string;
  disabled?: boolean;
};

export default function SegmentedControl({
  value,
  options,
  onChange,
  size = "sm",
  variant = "default",
  className,
}: {
  value: string;
  options: ReadonlyArray<SegmentedOption>;
  onChange: (next: string) => void;
  size?: "xs" | "sm";
  /** "default" uses bg-card (standalone); "inset" uses bg-card-inset (embedded in a form/card). */
  variant?: "default" | "inset";
  className?: string;
}) {
  const sizeClasses = size === "xs" ? "h-8 text-xs" : "h-9 text-sm";

  return (
    <div
      className={cn(
        "inline-flex overflow-hidden rounded-lg border border-border shadow-xs",
        variant === "inset" ? "bg-card-inset" : "bg-card",
        className,
      )}
      role="group"
    >
      {options.map((opt) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            disabled={opt.disabled}
            className={cn(
              "inline-flex items-center justify-center px-3 font-semibold transition-colors disabled:pointer-events-none disabled:opacity-50",
              sizeClasses,
              active
 ? "bg-indigo-600 text-white"
                : variant === "inset"
 ? "text-muted-foreground hover:bg-card"
                  : "bg-card text-card-foreground hover:bg-accent",
              "focus:outline-hidden focus:ring-2 focus:ring-indigo-500/30 focus:ring-inset",
            )}
            aria-pressed={active}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
