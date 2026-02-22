# TICKET-0033 — Overview: Telemetry tapestry layout regressions (hover jank + overflow)

## Problem / Why

On the Dashboard Web **Overview** tab, the “Telemetry tapestry” (local sensor heatmap) has two UX/data-quality issues:

1) Hovering heatmap cells causes **vertical layout shift** (rows jump) when the hover “details” box appears/disappears.
2) The tapestry card can show **horizontal overflow/scrollbars** inside the card at normal desktop widths (2-column layout), which is visually noisy and hides bugs.

These regressions reduce operator trust and make the telemetry overview hard to scan.

## Root cause hypotheses (to confirm during implementation)

- `apps/dashboard-web/src/features/overview/components/LocalSensorVisualizations.tsx`:
  - Hover details panel is conditionally rendered inside a wrapping flex header (`flex-wrap`), changing header height/row-wrapping on hover.
  - Heatmap container uses `overflow-x-auto` + `min-w-[680px]`, forcing internal horizontal scrolling at common widths.

## Requirements / Acceptance

### UX / Layout (hard requirements)
- Hovering across heatmap cells **never** changes the vertical position of the heatmap rows (no jank).
- The Telemetry tapestry card has **no internal horizontal scrollbar** at typical desktop widths (e.g. 1280×800) and should remain usable at tablet widths.
- The hover details area remains visible and readable, but **does not** push other content around.

### Regression prevention
- Add Playwright coverage to catch this class of regressions:
  - Assert **no horizontal overflow** (tapestry container + page-level).
  - Assert **no vertical shift** of the heatmap rows during hover/unhover.
  - Test must be deterministic via stubbed API responses.

### Validation
- `make ci-web-smoke` passes.
- Tier A (installed controller refresh; no DB/settings reset) includes a screenshot captured + viewed and a run log under `project_management/runs/`.

## Implementation notes / Suggested approach

- Make the hover details box **structurally stable**:
  - Always render a fixed-size/details container and swap content (placeholder vs hovered).
  - Avoid `flex-wrap` header reflow: prefer a grid header (stack on small screens; stable right column on desktop).
- Remove forced minimum widths and ensure flex/grid children can shrink (`min-w-0` where needed).
- Prefer responsive stacking over `overflow-x-auto` for the heatmap rows on narrow widths.

## File targets

- UI:
  - `apps/dashboard-web/src/features/overview/components/LocalSensorVisualizations.tsx`
- Playwright regression:
  - `apps/dashboard-web/playwright/overview-tapestry-layout.spec.ts` (new)
  - `apps/dashboard-web/playwright/helpers/layout.ts` (new helper)
- Project tracking:
  - `project_management/TASKS.md`
  - `project_management/BOARD.md`
  - `project_management/EPICS.md`

