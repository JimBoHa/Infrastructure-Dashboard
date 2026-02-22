# TSE-0002: Analysis Jobs framework (server-side, Rust)

Priority: P0
Status: Done (tracked as TSSE-3 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Implement a robust, reusable analysis job framework that can power:
- Related Sensors scan
- Relationships matrix
- Event/spike matching
- Co-occurrence
- Matrix profile-like analyses

This eliminates client-side “N requests + compute” patterns and guarantees “never error” by turning heavy work into cancelable jobs with progress.

## Integration points (verified)
- Controller runtime: implement this inside `apps/core-server-rs` (production controller service).
- Route wiring: `apps/core-server-rs/src/routes/mod.rs` nests all routes under `/api` via `.merge(...)`.
- Existing “job-like” reference (do NOT reuse as-is): deployment jobs are tracked in-memory inside
  `apps/core-server-rs/src/services/deployments/*` (HashMap + Mutex). That pattern is fine for short-lived UI
  workflows, but TSSE analysis jobs must be **durable and queryable** (persisted in Postgres) and support longer runtimes.

## Scope
- Job model: create, run, progress, cancel, complete, fail (with structured error), expire.
- Storage: persist job metadata and results index in Postgres.
- Execution: bounded concurrency, fair scheduling, resource throttling for Mac mini.
- Progress reporting: structured progress events (counts, phases, ETA best-effort).
- Cancellation: cooperative cancellation propagated to workers.

## Non-goals
- Implement specific similarity algorithms (that’s later tickets).

## Collab Harness (REQUIRED during implementation)
Orchestrator must use multi-agent workflow:
- Worker A: Postgres schema + migrations + job state model.
- Worker B: job runner + cancellation + concurrency controls.
- Worker C: API shape + progress streaming approach (SSE vs polling) and security.

High-visibility deliverables per worker:
- 1) interface proposal (types + schemas)
- 2) patch
- 3) test evidence

## Acceptance Criteria
- New job tables exist (or equivalent) with indices supporting lookups by status/user/type.
- Job runner supports:
  - bounded concurrency
  - per-job progress updates
  - cancellation
  - safe retry semantics (idempotent where appropriate)
- Jobs can be deduplicated by a “job key” (params hash) when configured.
- Unit tests cover state transitions and cancellation.

## Testing / Validation
- Add a synthetic “noop analysis job” to validate scheduler, progress, cancel.
- Demonstrate multiple concurrent jobs without starving.
