# TICKET-0039: Trends: Savitzky–Golay smoothing toggle (advanced chart settings)

**Status:** Open

## Description

Trends already supports multi-sensor charting and relationship analysis, but operators also need a **signal-processing mode** to reduce noise, highlight rate-of-change, and make spike/edge events easier to see.

Add an optional **Savitzky–Golay (SG)** filter to the Trends **Trend chart** that can be enabled/disabled and configured via Chart settings.

This should be implemented in a scientifically correct way:
- SG smoothing must be computed via least-squares polynomial fitting over a sliding window (not a moving average).
- It must correctly support **derivatives** (0 = smoothed signal, 1 = first derivative, etc.) with sample spacing (`Δt`) derived from the selected Trend chart **Interval**.
- It must preserve missing-data gaps (do not smooth across null/offline gaps).

Constraints:
- This is **visualization-only** for the Trend chart (no data writes; do not change stored telemetry).
- Do not change other charts/sections unless explicitly enabled for them (avoid cross-page regressions).
- Keep UI responsive by default; allow the user to opt into heavier computation via the toggle.

## Scope
* [ ] Add a Trend chart SG filter toggle (off by default).
* [ ] Add an “Advanced” collapsible section under Chart settings with SG parameters:
  - window length
  - polynomial degree
  - derivative order
  - edge handling mode (at least: `interp` recommended, plus a simpler mode like `nearest` or `mirror`)
  - derivative unit scaling (optional; clarify output units when derivative > 0)
* [ ] Validate parameters and show clear guidance/errors (e.g., window must be odd; degree < window; derivative ≤ degree).
* [ ] Implement the SG helper as a reusable utility and apply it to the Trend chart only.
* [ ] Tier A validation on an installed controller: refresh bundle, capture/view screenshots, and log evidence.

## Acceptance Criteria
* [ ] Trends → Chart settings includes a “Savitzky–Golay” toggle (off by default).
* [ ] When enabled, Trend chart renders filtered series (smoothed/derivative) using SG, without smoothing across gaps.
* [ ] Advanced SG settings exist under a collapsible “Advanced” section and are explained in plain English.
* [ ] Invalid SG settings do not crash the page; the UI shows a clear error and does not apply the filter.
* [ ] The SG implementation accounts for `Δt` based on the selected Interval when derivative order > 0.
* [ ] Other charts/pages using `TrendChart` are unchanged unless they explicitly opt in.
* [ ] `make ci-web-smoke` passes.
* [ ] Tier A: installed controller is upgraded to a bundle containing this change (no DB/settings reset), at least one screenshot is captured and **viewed** under `manual_screenshots_web/`, and a run log is recorded under `project_management/runs/`.

## Notes
Primary implementation targets:
- `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
- `apps/dashboard-web/src/components/TrendChart.tsx`

