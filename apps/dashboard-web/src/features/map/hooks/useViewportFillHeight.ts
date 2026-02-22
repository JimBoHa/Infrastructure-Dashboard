"use client";

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

const DEFAULT_BOTTOM_PADDING_PX = 24;
const DEFAULT_MIN_HEIGHT_PX = 420;
const DEFAULT_DESKTOP_QUERY = "(min-width: 1024px)";

type UseViewportFillHeightOptions = {
  dependencies?: unknown[];
  bottomPaddingPx?: number;
  minHeightPx?: number;
  desktopMediaQuery?: string;
};

export function useViewportFillHeight(options: UseViewportFillHeightOptions = {}) {
  const {
    dependencies = [],
    bottomPaddingPx = DEFAULT_BOTTOM_PADDING_PX,
    minHeightPx = DEFAULT_MIN_HEIGHT_PX,
    desktopMediaQuery = DEFAULT_DESKTOP_QUERY,
  } = options;
  const viewportFillRef = useRef<HTMLDivElement | null>(null);
  const [viewportFillHeightPx, setViewportFillHeightPx] = useState<number | null>(null);

  const recomputeViewportFillHeight = useCallback(() => {
    if (typeof window === "undefined") return;

    const isDesktop = window.matchMedia(desktopMediaQuery).matches;
    if (!isDesktop) {
      setViewportFillHeightPx(null);
      return;
    }

    const container = viewportFillRef.current;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const nextHeightPx = Math.max(minHeightPx, Math.floor(window.innerHeight - rect.top - bottomPaddingPx));
    setViewportFillHeightPx(nextHeightPx);
  }, [bottomPaddingPx, desktopMediaQuery, minHeightPx]);

  useLayoutEffect(() => {
    if (typeof window === "undefined") return;
    const handle = window.requestAnimationFrame(() => recomputeViewportFillHeight());
    return () => window.cancelAnimationFrame(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- spread dependencies are caller-provided invalidation triggers
  }, [recomputeViewportFillHeight, ...dependencies]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const handler = () => recomputeViewportFillHeight();
    window.addEventListener("resize", handler);
    return () => window.removeEventListener("resize", handler);
  }, [recomputeViewportFillHeight]);

  return { viewportFillRef, viewportFillHeightPx, recomputeViewportFillHeight };
}
