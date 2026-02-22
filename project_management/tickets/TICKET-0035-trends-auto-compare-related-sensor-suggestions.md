# TICKET-0035: Trends: auto-compare related sensor suggestions

**Status:** Open

## Description
Operators can already compare sensors manually in the Trends tab, but the system should be able to proactively suggest “related” sensors based on observed patterns in the data (correlation, inverse correlation, and lead/lag relationships).

This ticket adds a polished “Related sensors” experience to Trends:
- Pick a **focus sensor** (one of the selected series) and automatically scan other sensors for strong relationships over the currently viewed window.
- Surface **transparent metrics** (method, overlap points, correlation strength, and best lag) so operators can trust what they’re seeing.
- Provide an **interactive preview** (overlay + lag view) and a one-click **Add to chart** action.

Constraints:
- Use only the controller’s stored telemetry (`/api/metrics/query`). No external/public API data sources are involved in this feature.
- Do not obscure data: if normalization/shape comparison is used in previews, label it explicitly and keep raw previews available.
- Keep the UI fast and stable: batch candidate metric fetches; avoid scanning unbounded sensor sets without user control.

## Scope
* [ ] Add a new Trends panel that discovers and ranks related sensors for a chosen focus series.
* [ ] Implement client-side scoring using correlation + optional best-lag search (Pearson/Spearman).
* [ ] Add interactive preview visualizations and “Add to chart” UX.
* [ ] Add deterministic Playwright coverage (stubbed API) for suggestions + add-to-chart.
* [ ] Tier A validation on an installed controller: refresh bundle, capture/view screenshots, and log evidence.

## Acceptance Criteria
* [ ] With ≥1 selected sensor, Trends shows a “Related sensors” panel with a focus sensor selector.
* [ ] Suggestions are computed from controller telemetry only and are clearly labeled with:
  - correlation method (Pearson/Spearman)
  - overlap points `n`
  - best lag (±N buckets) and lead/lag direction (when enabled)
* [ ] Each suggestion supports a preview and “Add to chart”; adding updates the chart selection and legend.
* [ ] Normalized/shape previews (if present) are explicitly labeled; raw previews remain available.
* [ ] Candidate metric queries are batched (no single request attempts to pull an unbounded sensor set).
* [ ] `make ci-web-smoke` passes.
* [ ] Tier A: installed controller is upgraded to a bundle containing this change (no DB/settings reset), at least one screenshot is captured and **viewed** under `manual_screenshots_web/`, and a run log is recorded under `project_management/runs/`.

## Notes
Primary implementation area: `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`.
