"use client";

import type { Options } from "highcharts";
import HighchartsReact, { type HighchartsReactProps, type HighchartsReactRefObject } from "highcharts-react-official";
import { useCallback, useEffect, useMemo, useRef, type CSSProperties, type MutableRefObject } from "react";
import { Highcharts } from "@/components/HighchartsProvider";
import { cn } from "@/lib/utils";

export type HighchartsChartRef = HighchartsReactRefObject;

type HighchartsPanelProps = {
  options: Options;
  chartRef?: MutableRefObject<HighchartsChartRef | null>;
  wrapperClassName?: string;
  containerClassName?: string;
  containerStyle?: CSSProperties;
  containerProps?: HighchartsReactProps["containerProps"];
  constructorType?: HighchartsReactProps["constructorType"];
  resetZoomOnDoubleClick?: boolean;
  onDoubleClick?: () => void;
  enableAutoReflow?: boolean;
  centeredMaxWidthPx?: number;
  testId?: string;
  highcharts?: typeof Highcharts;
} & Omit<HighchartsReactProps, "options" | "highcharts" | "containerProps" | "constructorType">;

export function HighchartsPanel({
  options,
  chartRef,
  wrapperClassName,
  containerClassName,
  containerStyle,
  containerProps,
  constructorType,
  resetZoomOnDoubleClick = false,
  onDoubleClick,
  enableAutoReflow = false,
  centeredMaxWidthPx,
  testId,
  highcharts,
  ...reactProps
}: HighchartsPanelProps) {
  const internalChartRef = useRef<HighchartsChartRef | null>(null);
  const effectiveChartRef = chartRef ?? internalChartRef;
  const wrapperRef = useRef<HTMLDivElement | null>(null);

  const mergedContainerProps = useMemo<HighchartsReactProps["containerProps"]>(() => {
    const incomingStyle = (containerProps?.style as CSSProperties | undefined) ?? {};
    const style: CSSProperties = {
      ...(containerStyle ?? {}),
      ...incomingStyle,
    };

    if (centeredMaxWidthPx != null) {
      style.maxWidth = `${centeredMaxWidthPx}px`;
      if (style.margin == null) {
        style.margin = "0 auto";
      }
    }

    return {
      ...containerProps,
      className: cn("h-full w-full", containerClassName, containerProps?.className),
      style,
    };
  }, [centeredMaxWidthPx, containerClassName, containerProps, containerStyle]);

  const handleDoubleClick = useCallback(() => {
    if (resetZoomOnDoubleClick) {
      effectiveChartRef.current?.chart?.zoomOut();
    }
    onDoubleClick?.();
  }, [effectiveChartRef, onDoubleClick, resetZoomOnDoubleClick]);

  useEffect(() => {
    if (!enableAutoReflow) return;
    if (typeof ResizeObserver === "undefined") return;

    const wrapperEl = wrapperRef.current;
    if (!wrapperEl) return;

    const observer = new ResizeObserver(() => {
      effectiveChartRef.current?.chart?.reflow();
    });

    observer.observe(wrapperEl);
    return () => observer.disconnect();
  }, [effectiveChartRef, enableAutoReflow]);

  return (
    <div ref={wrapperRef} data-testid={testId} className={wrapperClassName} onDoubleClick={handleDoubleClick}>
      <HighchartsReact
        {...reactProps}
        ref={effectiveChartRef}
        highcharts={highcharts ?? Highcharts}
        options={options}
        constructorType={constructorType}
        containerProps={mergedContainerProps}
      />
    </div>
  );
}

export type { HighchartsPanelProps };
