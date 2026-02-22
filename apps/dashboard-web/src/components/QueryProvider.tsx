"use client";

import { QueryClientProvider, focusManager } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { createQueryClient } from "@/lib/queryClient";

export default function QueryProvider({ children }: { children: React.ReactNode }) {
  const [client] = useState(() => createQueryClient());
  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") {
      return;
    }
    focusManager.setEventListener((handleFocus) => {
      const onVisibilityChange = () => {
        handleFocus(document.visibilityState === "visible");
      };
      window.addEventListener("visibilitychange", onVisibilityChange, false);
      return () => {
        window.removeEventListener("visibilitychange", onVisibilityChange);
      };
    });
    focusManager.setFocused(document.visibilityState === "visible");
  }, []);

  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
