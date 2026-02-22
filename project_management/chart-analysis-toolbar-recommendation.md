# Chart Analysis Toolbar — Implementation Notes (DW-198)

**Date:** 2026-01-31  
**Scope:** Trends chart only (`apps/dashboard-web/src/components/TrendChart.tsx`)  
**Goal:** Replace default Highcharts Stock Tools GUI with a polished, dashboard-native analysis toolbar

## Decision (implemented)

Build a custom toolbar using existing dashboard-web UI primitives (`Card`, `Button`, `Select`, `InlineBanner`) and wire it to:
- Highcharts Stock Tools **navigation bindings** (for draw/measure/annotation tools), and
- the existing `/api/chart-annotations` persistence API (save/load annotations),

while keeping the default Highcharts Stock Tools left-side GUI disabled (`stockTools.gui.enabled=false`).

## Why this approach

1. **Single chart engine** — Highcharts already draws tools in data space (zoom/pan stays correct).
2. **No new UI dependencies** — stays within the existing shadcn/Radix + Tailwind patterns.
3. **Offline-friendly** — no external services or CDN assets required.
4. **Extends existing pattern** — best-fit regression remains a custom chart interaction, but is presented as a first-class toolbar tool.

## Architecture

- `TrendChart.tsx` owns the analysis tool state machine:
  - Exactly one active tool at a time.
  - `Esc` cancels the current tool (binding tools, pan, eraser, best-fit).
  - “Pan” is a chart-mode toggle; “Eraser” routes Highcharts annotation clicks to deletion.
- Tool activation uses Highcharts navigation bindings:
  - `TrendChart.tsx` injects `Highcharts.getOptions().navigation.bindings` into `chartOptions.navigation.bindings` so Highcharts instantiates `chart.navigationBindings` even when Stock Tools GUI is disabled.
  - Toolbar buttons arm tools by calling `chart.navigationBindings.bindingsButtonClick(...)` with a hidden/virtual button element (matching the binding’s expected class name).
- Best-fit regression is implemented as a custom interaction:
  - User clicks start/end points on the chart to define the regression window.
  - Overlay series uses the correct y-axis when `Independent axes` is enabled.
  - Summary includes at minimum `n`, `R²`, and slope/rate messaging.

## Persistence

- Highcharts-created annotations are persisted via `/api/chart-annotations`:
  - On tool completion (`navigation.events.deselectButton`), new annotations are serialized and POSTed via `onCreatePersistentAnnotation`.
  - “Clear all” and eraser-delete call `onDeletePersistentAnnotation`.
- `TrendsPageClient.tsx` renders persisted annotations by mapping stored `highcharts_annotation` rows back into `annotations[]` with the DB row `id`.

## Constraints

- Works fully offline (no CDN, no remote dependency downloads).
- No second rendering/canvas layer over the chart.
- Must not re-enable the default Stock Tools left-side GUI.
