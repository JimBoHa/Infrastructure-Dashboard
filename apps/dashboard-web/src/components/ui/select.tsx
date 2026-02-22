import * as React from "react";

import { cn } from "@/lib/utils";

function Select({ className, ...props }: React.ComponentProps<"select">) {
  return (
    <select
      data-slot="select"
      className={cn(
 "block w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30 disabled:opacity-60",
        className,
      )}
      {...props}
    />
  );
}

export { Select };
