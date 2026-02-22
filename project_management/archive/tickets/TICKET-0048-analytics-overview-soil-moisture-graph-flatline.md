# TICKET-0048: analytics-overview-soil-moisture-graph-flatline

**Status:** Closed

## Description
The **Analytics Overview → Soil moisture** chart renders as a flat line at `0%`, even though real soil moisture telemetry is ingesting and the **Trends** chart shows non-zero soil moisture values.

This is misleading and makes the Analytics Overview page unusable for soil moisture monitoring.

## Scope
* [x] Confirm root cause by inspecting `GET /api/analytics/soil` response.
* [x] Implement real soil analytics aggregation from the controller DB (avg/min/max + field summaries).
* [x] Keep the response backwards compatible and update OpenAPI schema.
* [x] Tier-A validate on the installed controller (rebuild + refresh; no DB/settings reset) and capture/view evidence screenshots.

## Acceptance Criteria
* [x] `GET /api/analytics/soil` returns a non-zero series when moisture metrics exist (no fabricated all-zero series).
* [x] Response includes fleet-level `series_avg`, `series_min`, and `series_max` (with `series` remaining present for compatibility).
* [x] Analytics Overview “Soil moisture” chart reflects real data (matches Trends directionally; no 0% flatline when sensors are reporting).
* [x] Tier-A evidence captured and viewed on installed controller:
  - version refreshed to `0.1.9.232`
  - `make e2e-installed-health-smoke` passes
  - screenshot stored under `manual_screenshots_web/` showing the non-zero soil moisture chart
* [x] Local validation passes:
  - `make ci-core-smoke`
  - `make ci-web-smoke`

## Notes
References:
- ADR: `docs/ADRs/0008-real-soil-analytics-from-metrics.md`
- Task: AN-35 (`project_management/TASKS.md`)
- Tier-A run: `project_management/runs/RUN-20260131-tier-a-an35-soil-analytics-0.1.9.232.md`

Evidence:
- `manual_screenshots_web/20260131_162607/analytics.png` (captured + viewed; soil moisture chart is non-zero)

Implementation:
- Fix implemented in `apps/core-server-rs/src/routes/analytics.rs` (`/api/analytics/soil` now queries `sensors` + `metrics`).
- Installed controller refreshed to `0.1.9.232` (commit `9b9a283`).
