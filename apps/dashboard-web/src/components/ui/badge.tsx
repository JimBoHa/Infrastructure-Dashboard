import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-full",
  {
    variants: {
      tone: {
 muted: "bg-muted text-muted-foreground",
 neutral: "bg-muted text-foreground",
 success: "bg-emerald-50 text-emerald-700",
 warning: "bg-amber-50 text-amber-700",
 danger: "bg-red-50 text-red-700",
 info: "bg-sky-50 text-sky-700",
 accent: "bg-indigo-50 text-indigo-700",
      },
      size: {
        sm: "px-2 py-0.5 text-xs",
        md: "px-2 py-1 text-[11px]",
        lg: "px-3 py-1 text-xs",
      },
      weight: {
        normal: "font-normal",
        semibold: "font-semibold",
      },
    },
    defaultVariants: {
      tone: "neutral",
      size: "sm",
      weight: "semibold",
    },
  },
);

type BadgeTone = NonNullable<VariantProps<typeof badgeVariants>["tone"]>;

function Badge({
  className,
  tone,
  size,
  weight,
  caps,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & {
    caps?: boolean;
  }) {
  return (
    <span
      data-slot="badge"
      className={cn(
        badgeVariants({ tone, size, weight }),
        caps && "uppercase tracking-wide",
        className,
      )}
      {...props}
    />
  );
}

export { Badge, badgeVariants };
export type { BadgeTone };
