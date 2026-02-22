"use client";

import { useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@/components/ui/collapsible";

type Density = "sm" | "md";

const densityHeaderClass: Record<Density, string> = {
  sm: "px-3 py-2.5",
  md: "px-4 py-3",
};

const densityBodyClass: Record<Density, string> = {
  sm: "px-3 py-3",
  md: "px-4 py-4",
};

const densityTitleClass: Record<Density, string> = {
  sm: "text-sm font-semibold text-card-foreground",
  md: "text-lg font-semibold text-card-foreground",
};

const densityDescriptionClass: Record<Density, string> = {
  sm: "text-xs text-muted-foreground",
  md: "text-sm text-muted-foreground",
};

export default function CollapsibleCard({
  title,
  description,
  actions,
  children,
  defaultOpen = true,
  open,
  onOpenChange,
  density = "md",
  className,
  bodyClassName,
  id,
  "data-testid": dataTestId,
}: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (next: boolean) => void;
  density?: Density;
  className?: string;
  bodyClassName?: string;
  id?: string;
  "data-testid"?: string;
}) {
  const isControlled = typeof open === "boolean";
  const [internalOpen, setInternalOpen] = useState(defaultOpen);

  const effectiveOpen = isControlled ? (open as boolean) : internalOpen;
  const setOpen = (next: boolean) => {
    if (!isControlled) setInternalOpen(next);
    onOpenChange?.(next);
  };

  const chevron = (
    <svg
      viewBox="0 0 20 20"
      fill="currentColor"
      aria-hidden="true"
      className={cn(
        "h-5 w-5 text-muted-foreground transition-transform",
        effectiveOpen ? "rotate-180" : "rotate-0",
      )}
    >
      <path
        fillRule="evenodd"
        d="M5.23 7.21a.75.75 0 0 1 1.06.02L10 11.19l3.71-3.96a.75.75 0 1 1 1.08 1.04l-4.25 4.54a.75.75 0 0 1-1.08 0L5.21 8.27a.75.75 0 0 1 .02-1.06Z"
        clipRule="evenodd"
      />
    </svg>
  );

  return (
    <Collapsible open={effectiveOpen} onOpenChange={setOpen}>
      <div
        id={id}
        data-testid={dataTestId}
        className={cn(
          "bg-card text-card-foreground min-w-0 rounded-xl border shadow-sm",
          className,
        )}
      >
        <div className={cn("flex items-start justify-between gap-4", densityHeaderClass[density])}>
          <CollapsibleTrigger asChild>
            <button
              type="button"
              className="flex min-w-0 flex-1 cursor-pointer select-none items-start gap-4 text-left"
            >
              <div className="min-w-0 flex-1">
                <h3 className={densityTitleClass[density]}>{title}</h3>
                {description ? (
                  <div className={cn("mt-1", densityDescriptionClass[density])}>{description}</div>
                ) : null}
              </div>
              {!actions ? chevron : null}
            </button>
          </CollapsibleTrigger>

          {actions ? (
            <div className="flex shrink-0 items-center gap-2">
              {actions}
              <CollapsibleTrigger asChild>
                <button type="button" className="cursor-pointer">
                  {chevron}
                </button>
              </CollapsibleTrigger>
            </div>
          ) : null}
        </div>

        <CollapsibleContent forceMount asChild>
          <div
            className={cn(
              "grid grid-cols-1 overflow-hidden transition-[grid-template-rows] duration-300 ease-in-out data-[state=open]:overflow-visible",
              "data-[state=open]:grid-rows-[1fr] data-[state=closed]:grid-rows-[0fr]",
            )}
          >
            <div className="min-h-0 min-w-0">
              <div
                className={cn(
                  "border-t border-border",
                  densityBodyClass[density],
                  bodyClassName,
                )}
              >
                {children}
              </div>
            </div>
          </div>
        </CollapsibleContent>
      </div>
    </Collapsible>
  );
}
