# TSE-0001: TSSE requirements + success metrics + design ADR (Mac mini)

Priority: P0
Status: Done (tracked as TSSE-2 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Establish a single, explicit, testable contract for the Time-Series Similarity Engine (TSSE) that covers:
- product requirements (episodic similarity, outlier robustness, multi-window search)
- scale targets (1k sensors @ 1s–30s, 90d analysis horizon)
- performance budgets on a single Mac mini
- API surface expectations
- “never error” guarantees (no “requested series too large”)

Deliverable: an ADR-style design doc and a measurable success rubric that subsequent tickets implement.

ADR output (implemented):
- `docs/ADRs/0006-time-series-similarity-engine-(tsse)-on-controller-similarity-search.md`

## Context
The current “Related sensors” scan pattern triggers “Requested series too large” errors and forces a slow client-side request/compute loop. The product requirement has expanded: this is a similarity search engine (episodic, robust, multi-window), not a single Pearson correlation over one range.

## Current implementation (verified in repo code)
Controller runtime:
- Production controller API is Rust: `apps/core-server-rs` (Axum).

Related sensors scan path today:
- UI: `apps/dashboard-web/src/features/trends/components/AutoComparePanel.tsx`
- Fetch loop: `apps/dashboard-web/src/features/trends/utils/metricsBatch.ts` → `fetchMetricsSeries()` → `/api/metrics/query`
- Scoring: browser-side correlation/event logic (`apps/dashboard-web/src/features/trends/utils/relatedSensors.ts` and `eventMatch.ts`)

Hard failure source:
- `/api/metrics/query` enforces `MAX_METRICS_POINTS = 25_000` in `apps/core-server-rs/src/routes/metrics.rs`.
- The client responds by recursively splitting batches on that error, which increases request count and latency.

## Scope
- Define “similarity” objective and ranking behavior.
- Define episode representation and how it’s surfaced to the user.
- Define multi-window strategy (max horizon + smaller windows) and reasonable limits.
- Define how outliers are handled.
- Define correctness criteria and invariants.
- Define performance budgets for Mac mini.
- Decide architectural boundary: analysis jobs + Parquet + DuckDB + Qdrant.

## Non-goals
- Implement code.

## Collab Harness (REQUIRED during implementation)
Orchestrator must:
- Spawn at least 3 worker agents and run a short design review loop before finalizing the ADR.
- Maintain a shared “Open Questions” list and drive it to closure.
- Explicitly reference the “current implementation” file paths above when documenting what is being replaced.

Suggested worker agents:
- Worker A (Data/Storage): Parquet layout, ingestion, NAS future.
- Worker B (Algorithms/Stats): episodic scoring, robust correlation, lag handling.
- Worker C (API/UX): job API, progress, preview, result shape.

Visibility requirements:
- Orchestrator posts a 1-page summary: “Decisions made / Rejected options / Risks / Next tickets”.

## Acceptance Criteria
- A written ADR exists that answers:
  - exact definition of similarity score and ranking
  - episode extraction + episode scoring
  - outlier mitigation method(s)
  - multi-window plan (window sizes + how many + limits)
  - lag estimation/refinement strategy
  - data freshness semantics (90d hot Parquet, unbounded Postgres archive)
  - how “never error” is enforced
  - performance targets (p50/p95) for key workloads on Mac mini
- Includes a “test plan” mapping requirements → benchmarks/tests.

## Implementation Notes (for later tickets)
- Production backend/services must be Rust.
- Charts may still require visualization decimation, but analysis must not force users to change interval.
