# TSE-0012: Preview & episode drilldown endpoints (bounded payloads)

Priority: P0
Status: In Progress (tracked as TSSE-13 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Enable the UI to preview a suggested relationship without fetching massive series for all candidates.

## Scope
- Preview endpoint takes:
  - focus sensor id
  - candidate sensor id
  - selected episode (or auto-select strongest)
  - optional lag alignment
- Returns:
  - bounded time window series for focus and candidate
  - metadata (units, labels)
  - optional “aligned by lag” series
- Must not error due to size:
  - enforce maximum preview window duration or max points per response
  - allow paging if needed

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: endpoint design + OpenAPI.
- Deliverable: efficient DuckDB queries for window slices.
- Deliverable: UI compatibility review.

## Acceptance Criteria
- Preview supports episode drilldown with fast latency.
- Preview responses are bounded and never hit “series too large”.
