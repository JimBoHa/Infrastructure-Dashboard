# TICKET-0046: Trends chart analysis toolbar

**Status:** Done (Tier A validated 0.1.9.229; Tier B deferred to DW-98)

## Description
The Trends chart needs a polished, comprehensive analysis tooling surface (thinkorswim-style UX) without using the default Highcharts Stock Tools left-side GUI (which is not acceptable UX). Operators should get a consistent, discoverable tool palette (lines, measure tools, annotations, navigation toggles) that feels native to the dashboard design system.

This ticket defines the implementation plan for a custom toolbar UI that triggers the underlying Highcharts Stock Tools bindings and annotations API.

## References
- Recommendation: `project_management/chart-analysis-toolbar-recommendation.md`
- Existing chart: `apps/dashboard-web/src/components/TrendChart.tsx`
- Best-fit prototype work (to be integrated into toolbar): `project_management/TASKS.md` (`DW-197`)

## Scope
* [x] Implement a custom `ChartAnalysisToolbar` UI (shadcn/Radix look-and-feel) that replaces the Stock Tools left-side GUI while continuing to use Highcharts’ data-space tools.
* [x] Provide a grouped tool palette with a clear “active tool” state and good interaction affordances (cursor, hover, cancel, undo/delete where supported).
* [x] Wire toolbar buttons to:
  - navigation bindings (Highcharts stocktools class bindings), and/or
  - `chart.addAnnotation()` for annotation types, and/or
  - custom handlers (best-fit regression).
* [x] Integrate best-fit regression as a first-class toolbar tool:
  - user sets start/stop by interacting directly with the chart using the mouse
  - regression line overlays only the chosen window
  - summary stats are shown (at minimum `n`, `R²`, and a human-friendly slope/rate).
* [x] Ensure the tool palette does not interfere with normal chart navigation when no tool is active (zoom/pan/range selection must remain smooth and predictable).
* [x] Use the existing annotations persistence layer so toolbar-created annotations save/load (where supported by the backend endpoints).

## Acceptance Criteria
* [x] Trends renders a custom analysis toolbar that matches the dashboard design system and does not show the default Stock Tools left-side GUI.
* [x] Toolbar tool groups exist (minimum viable set):
  - Lines: trendline, horizontal line, best-fit line
  - Measure: Fibonacci retracement, measure XY, distance
  - Annotate: label, arrow, rectangle highlight
  - Navigate: zoom in/out controls, pan toggle
  - Eraser / Clear all
* [x] Active tool UX:
  - One active tool at a time with clear “armed” state.
  - Escape cancels the active tool without leaving the chart in a broken state.
  - Selecting/dragging/placing tools works reliably on real data and doesn’t get “eaten” by chart trackers.
* [x] Best-fit regression UX:
  - Start/stop points are set by mouse interaction on the chart (no manual timestamp typing required).
  - The regression overlay uses the correct y-axis when independent axes are enabled.
* [x] Persistence:
  - Saved annotations load on refresh (where the backend supports the annotation type).
* [x] Validation:
  - `make ci-web-smoke` passes.
  - `cd apps/dashboard-web && npm run build` passes.
  - Tier A (installed controller; no DB/settings reset) upgrade/refresh validates the Trends toolbar with at least one **viewed** screenshot captured under `manual_screenshots_web/`.

## Tier A Evidence

- Run: `project_management/runs/RUN-20260131-tier-a-dw198-trends-chart-analysis-toolbar-0.1.9.229.md`
- Screenshots: `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529`

## Notes
Implementation caveats:
- Tool activation + click/drag capture is the hardest part; design the state machine first (active tool, cancel, reset).
- Best-fit regression is custom math; it must still feel native by sharing the same activation model/cursor/selection affordances as other tools.
