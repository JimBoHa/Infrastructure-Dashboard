"use client";

import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";

type MermaidModule = typeof import("mermaid");

let mermaidInitPromise: Promise<MermaidModule> | null = null;

async function getMermaid(): Promise<MermaidModule> {
  if (!mermaidInitPromise) {
    mermaidInitPromise = import("mermaid").then((module) => {
      module.default.initialize({
        startOnLoad: false,
        securityLevel: "strict",
        theme: "base",
        flowchart: {
          htmlLabels: false,
        },
        themeVariables: {
          fontFamily:
            "ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, Apple Color Emoji, Segoe UI Emoji",
          fontSize: "13px",
          textColor: "#111827",
          mainBkg: "#ffffff",
          nodeBkg: "#ffffff",
          nodeBorder: "#e5e7eb",
          lineColor: "#6b7280",
          edgeLabelBackground: "#ffffff",
          clusterBkg: "#f9fafb",
          clusterBorder: "#d1d5db",
        },
      });
      return module;
    });
  }
  return mermaidInitPromise;
}

export function MermaidDiagram({
  diagram,
  className,
  ariaLabel = "Diagram",
  tooltips,
}: {
  diagram: string;
  className?: string;
  ariaLabel?: string;
  tooltips?: Record<string, string>;
}) {
  const reactId = useId();
  const renderId = useMemo(() => `m_${reactId.replaceAll(":", "_")}`, [reactId]);
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const svgHostRef = useRef<HTMLDivElement | null>(null);
  const [activeTooltip, setActiveTooltip] = useState<{
    text: string;
    x: number;
    y: number;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function render() {
      setError(null);
      setSvg(null);
      try {
        const mermaid = await getMermaid();
        const { svg: renderedSvg } = await mermaid.default.render(renderId, diagram);
        const withAria = renderedSvg.replace(
          "<svg",
          `<svg role="img" aria-label="${ariaLabel.replaceAll('"', "'")}"`,
        );
        if (!cancelled) setSvg(withAria);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    }

    void render();
    return () => {
      cancelled = true;
    };
  }, [ariaLabel, diagram, renderId]);

  useEffect(() => {
    const host = svgHostRef.current;
    if (!host) return;
    host.innerHTML = svg ?? "";

    if (svg) {
      const svgEl = host.querySelector("svg");
      if (svgEl) {
        const edgePaths = svgEl.querySelectorAll(
          "path.flowchart-link, .edgePath path, .edgePaths path",
        );
        for (const edge of edgePaths) {
          try {
            edge.setAttribute("fill", "none");
            (edge as SVGElement).style.setProperty("fill", "none", "important");
            (edge as SVGElement).style.setProperty("stroke-width", "1.6px", "important");
          } catch {
            // Ignore DOM write failures and keep rendering the rest of the diagram.
          }
        }
      }
    }
  }, [svg]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !svg) return;

    const svgEl = container.querySelector("svg");
    if (!svgEl) return;

    const tooltipMap = tooltips ?? {};

    const candidateLabels = (el: Element): string[] => {
      const findClosest = (start: Element | null, selector: string): Element | null => {
        let current: Element | null = start;
        while (current) {
          if (typeof current.matches === "function" && current.matches(selector)) return current;
          current = current.parentElement;
        }
        return null;
      };

      const clickTitle =
        findClosest(el, "a")?.getAttribute("title") ??
        "";
      const titleText = el.querySelector("title")?.textContent ?? "";
      const explicit =
        el.getAttribute("aria-label") ??
        el.getAttribute("data-id") ??
        el.getAttribute("id") ??
        "";

      const nodeLabel =
        el.querySelector("span.nodeLabel")?.textContent ??
        el.querySelector("div.nodeLabel")?.textContent ??
        el.querySelector("span.label")?.textContent ??
        el.querySelector("div.label")?.textContent ??
        el.querySelector("text")?.textContent ??
        "";

      const normalized: string[] = [];
      for (const candidate of [nodeLabel, titleText, clickTitle, explicit]) {
        const normalizedCandidate = normalizeSvgLabel(candidate);
        if (normalizedCandidate) normalized.push(normalizedCandidate);
      }
      return Array.from(new Set(normalized));
    };

    const tooltipKeyForElement = (el: Element): string | null => {
      for (const candidate of candidateLabels(el)) {
        if (tooltipMap[candidate]) return candidate;
      }
      return null;
    };

    const showForElement = (el: Element) => {
      const text = el.getAttribute("data-farm-tooltip");
      if (!text) return;
      const containerRect = container.getBoundingClientRect();
      const rect =
        (el as unknown as SVGGraphicsElement | null)?.getBoundingClientRect?.() ??
        (el as unknown as HTMLElement | null)?.getBoundingClientRect?.() ??
        null;
      if (!rect) return;
      const x = rect.left - containerRect.left + rect.width / 2;
      const y = rect.top - containerRect.top - 8;
      setActiveTooltip({ text, x, y });
    };

    const clearTooltip = () => setActiveTooltip(null);

    const targets = Array.from(svgEl.querySelectorAll("g.node, g.cluster"));
    for (const el of targets) {
      const tooltipKey = tooltipKeyForElement(el);
      if (!tooltipKey) continue;
      el.setAttribute("data-farm-tooltip", tooltipMap[tooltipKey]);
      el.setAttribute("tabindex", "0");
      el.setAttribute("focusable", "true");
    }

    const closestMermaidTarget = (start: Element | null): Element | null => {
      let current: Element | null = start;
      while (current) {
        if (typeof current.matches === "function" && current.matches("g.node, g.cluster")) {
          return current;
        }
        if (typeof current.matches === "function" && current.matches("a")) {
          const wrapped =
            current.querySelector("g.node, g.cluster");
          if (wrapped) return wrapped;
        }
        current = current.parentElement;
      }
      return null;
    };

    const handleOver = (ev: Event) => {
      const target = ev.target as Element | null;
      if (!target) return;
      const node = closestMermaidTarget(target);
      if (!node) return;
      if (!node.getAttribute("data-farm-tooltip")) return;
      showForElement(node);
    };

    const handleOut = (ev: Event) => {
      const target = ev.target as Element | null;
      const related = (ev as MouseEvent).relatedTarget as Element | null;
      const fromNode = closestMermaidTarget(target);
      if (!fromNode) return;
      const toNode = closestMermaidTarget(related);
      if (toNode === fromNode) return;
      clearTooltip();
    };

    const handleFocusIn = (ev: FocusEvent) => {
      const target = ev.target as Element | null;
      if (!target) return;
      const node = closestMermaidTarget(target);
      if (!node) return;
      if (!node.getAttribute("data-farm-tooltip")) return;
      showForElement(node);
    };

    const handleFocusOut = (ev: FocusEvent) => {
      const target = ev.target as Element | null;
      const related = ev.relatedTarget as Element | null;
      const fromNode = closestMermaidTarget(target);
      if (!fromNode) return;
      const toNode = closestMermaidTarget(related);
      if (toNode === fromNode) return;
      clearTooltip();
    };

    svgEl.addEventListener("mouseover", handleOver, true);
    svgEl.addEventListener("mouseout", handleOut, true);
    svgEl.addEventListener("focusin", handleFocusIn, true);
    svgEl.addEventListener("focusout", handleFocusOut, true);

    return () => {
      svgEl.removeEventListener("mouseover", handleOver, true);
      svgEl.removeEventListener("mouseout", handleOut, true);
      svgEl.removeEventListener("focusin", handleFocusIn, true);
      svgEl.removeEventListener("focusout", handleFocusOut, true);
    };
  }, [svg, tooltips]);

  if (error) {
    return (
      <div className={className}>
        <InlineBanner tone="warning" className="p-3">
          Mermaid diagram failed to render: {error}
        </InlineBanner>
      </div>
    );
  }

  return (
    <div className={className}>
      <style>{`
        .farm-mermaid svg a { cursor: pointer; }
        .farm-mermaid svg .node rect,
        .farm-mermaid svg .node polygon,
        .farm-mermaid svg .node circle { shape-rendering: geometricPrecision; }
        /* Mermaid v11 flowcharts render edges as <path class="flowchart-link ..."> in a .edgePaths group. */
        .farm-mermaid svg .edgePaths path.flowchart-link {
          stroke-width: 1.6px;
          fill: none !important;
        }

        /* Back-compat for older Mermaid edge grouping. */
        .farm-mermaid svg .edgePath path { stroke-width: 1.6px; fill: none !important; }

        .farm-mermaid svg .edgePaths,
        .farm-mermaid svg .edgeLabels,
        .farm-mermaid svg .edgePath,
        .farm-mermaid svg .edgeLabel { pointer-events: none; }
      `}</style>
      <Card
        ref={containerRef}
        className="farm-mermaid relative overflow-x-auto rounded-lg gap-0 bg-card-inset p-3"
      >
        <div ref={svgHostRef} className="min-w-[720px]" />
        {!svg ? (
          <Card className="min-h-[220px] rounded-lg gap-0 border-dashed" />
        ) : null}
        {activeTooltip ? (
          <div
            className="pointer-events-none absolute z-10 max-w-[280px] -translate-x-1/2 rounded-md border border-border bg-card px-3 py-2 text-xs text-card-foreground shadow-lg"
            style={{ left: activeTooltip.x, top: activeTooltip.y }}
            role="tooltip"
          >
            {activeTooltip.text}
          </div>
        ) : null}
      </Card>
    </div>
  );
}

function normalizeSvgLabel(value: string) {
  return value.replaceAll(/\s+/g, " ").trim();
}
