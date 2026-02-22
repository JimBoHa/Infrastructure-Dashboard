import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 rounded-lg font-semibold transition-colors disabled:pointer-events-none disabled:opacity-50 focus:outline-hidden",
  {
    variants: {
      variant: {
        secondary:
 "border border-border bg-white text-foreground shadow-xs hover:bg-muted focus:bg-card-inset",
        primary:
 "bg-indigo-600 text-white hover:bg-indigo-700 focus:bg-indigo-700",
        danger:
 "border border-rose-200 bg-rose-50 text-rose-700 shadow-xs hover:bg-rose-100 focus:bg-rose-100",
        dashed:
 "border border-dashed border-indigo-200 bg-white text-indigo-700 shadow-xs hover:bg-indigo-50 focus:bg-indigo-50",
        ghost:
 "hover:bg-muted focus:bg-muted",
      },
      size: {
        xs: "px-3 py-2 text-xs",
        sm: "px-3 py-2 text-sm",
        md: "px-4 py-2 text-sm",
        icon: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "md",
    },
  },
);

function Button({
  className,
  variant,
  size,
  fullWidth,
  loading,
  children,
  disabled,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    fullWidth?: boolean;
    loading?: boolean;
  }) {
  return (
    <button
      data-slot="button"
      className={cn(
        buttonVariants({ variant, size }),
        fullWidth && "w-full",
        className,
      )}
      aria-busy={loading || undefined}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? (
        <span
          aria-hidden="true"
          className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
        />
      ) : null}
      {children}
    </button>
  );
}

export { Button, buttonVariants };
