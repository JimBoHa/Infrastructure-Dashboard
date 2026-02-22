# TSE-0018: Replace “series too large” failures in chart metrics path (paging/streaming)

Priority: P1
Status: Done (tracked as TSSE-19 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Ensure users are never forced to increase interval due to backend caps, including for charting paths.

## Current behavior (as of 2026-01-24)
- Backend: `/api/metrics/query` returns paged responses (cursor + `next_cursor`) when the request would exceed `MAX_METRICS_POINTS`, instead of hard-failing with “Requested series too large”.
- Dashboard-web: `fetchMetricsSeries` follows `next_cursor` and merges pages; `metricsBatch` only batches `sensor_ids`.
- Tests:
  - `apps/core-server-rs/src/routes/metrics.rs` paging unit tests
  - `apps/dashboard-web/tests/fetchMetricsSeriesPaging.test.ts`

## Scope
- Replace hard error behavior with:
  - pagination/cursor OR streaming responses
  - server-side chunking
- Update dashboard-web chart fetcher to handle paged/streamed responses.
- Add safeguards for rendering (visual decimation may be required for UI performance, but must be faithful and explicit).

Design constraint (aligned with Option B / no-analysis-fallback):
- Large chart/preview reads should come from the Parquet+DuckDB data plane where possible (same lake used by TSSE),
  rather than introducing another “large Timescale scan” path with different performance cliffs.

## Collab Harness (REQUIRED)
- Worker A: API design and backward compatibility.
- Worker B: backend implementation.
- Worker C: UI rendering strategy and performance.

## Acceptance Criteria
- No “Requested series too large” surfaced to users.
- Very large queries still complete (may take time) and can be canceled.
