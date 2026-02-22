# TSE-0003: Analysis API surface (create/job/progress/result/preview)

Priority: P0
Status: Done (tracked as TSSE-4 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Provide a stable API contract for analysis jobs so the UI can:
- start a scan
- observe progress
- fetch results and episodes
- request focused previews around detected episodes

## Integration points (verified)
- API service: `apps/core-server-rs` (Axum).
- Route wiring: add a new router (e.g. `apps/core-server-rs/src/routes/analysis.rs`) and merge it under `/api` via
  `apps/core-server-rs/src/routes/mod.rs`.
- Contract: update OpenAPI served by the controller (see `apps/core-server-rs/src/openapi.rs` and `apps/core-server-rs/src/routes/*` usage of `utoipa`).

## Scope
- Endpoints (names illustrative):
  - `POST /api/analysis/jobs` (create)
  - `GET /api/analysis/jobs/{id}` (status)
  - `GET /api/analysis/jobs/{id}/events` (SSE progress stream) OR polling contract
  - `GET /api/analysis/jobs/{id}/result` (ranked suggestions + episodes)
  - `POST /api/analysis/preview` (fetch series windows for focus+candidate around an episode)
- AuthZ: require appropriate capabilities for running scans.
- Response envelopes must be versioned.

## Non-goals
- Implement the algorithm internals.

## Collab Harness (REQUIRED)
- Worker A (API): OpenAPI updates + versioning strategy.
- Worker B (Security): authz rules + token handling + abuse limits.
- Worker C (UI contract): propose result JSON schema optimized for dashboard rendering.

Visibility:
- Orchestrator publishes a sample response JSON for each endpoint and a sequence diagram.

## Acceptance Criteria
- OpenAPI updated and includes the new endpoints.
- Endpoints support large jobs without timeouts (job-based).
- Result schema includes:
  - ranking score + components
  - episodes list with `{start,end,window_size,lag,strength,n_points,sign}`
  - “why ranked” explanation fields
  - data freshness watermark (“computed_through_ts”)
- Preview endpoint returns only what the chart needs (bounded response).
