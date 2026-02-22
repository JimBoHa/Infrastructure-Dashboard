# TSE-0011: Related Sensors scan job (end-to-end; never error)

Priority: P0
Status: In Progress (tracked as TSSE-12 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Deliver an end-to-end server-side Related Sensors scan that:
- never fails with “series too large”
- uses Qdrant candidate generation + exact episodic scoring
- returns ranked results with episodes and explanations

## Current behavior being replaced (verified)
- Trends “Related sensors” is currently a client loop in:
  - `apps/dashboard-web/src/features/trends/components/AutoComparePanel.tsx`
  - `apps/dashboard-web/src/features/trends/utils/metricsBatch.ts` (loops `/api/metrics/query`)
  - `apps/dashboard-web/src/features/trends/utils/relatedSensors.ts` and `eventMatch.ts` (in-browser scoring)
- `/api/metrics/query` hard-fails at `MAX_METRICS_POINTS = 25_000` in `apps/core-server-rs/src/routes/metrics.rs`.

## Scope
- Implement job type: `related_sensors_v1`.
- Pipeline:
  1) candidate generation (TSE-0009)
  2) exact scoring (TSE-0010) over Parquet/DuckDB (TSE-0006)
  3) store results, expose via API (TSE-0003)
- Robust handling:
  - missing data
  - sensors with different native sampling
  - derived sensors
  - partial overlap

## Non-negotiable product behaviors
- The job must accept “small interval + long range” requests without rejecting them; “slow but progressing” is acceptable, “error and tell user to increase interval” is not.
- Results must surface episodes so the UI can “click to zoom to strongest episode” without the user range-fiddling.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: integrates candidate gen + scoring.
- Deliverable: result schema + storage.
- Deliverable: perf profiling on Mac mini.

## Acceptance Criteria
- For a representative dataset, job completes and returns results without errors.
- Results include episodes and “why ranked”.
- Job progress is visible and cancelable.
