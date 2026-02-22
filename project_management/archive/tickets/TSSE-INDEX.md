# Time-Series Similarity Engine (TSSE) — Ticket Index (Mac mini)

Scope: Implement a time-series similarity search engine (episodic, outlier-robust, multi-window within a max horizon) that runs locally on a single macOS Mac mini.

Scale assumptions (current target):
- ~1,000 sensors at 1s–30s intervals
- 90-day analysis horizon (hot)
- Postgres retention unbounded (authoritative archive)
- Local Mac mini only; future optional NAS over 10GbE for cold data

Hard requirements:
- User can run scans at high time resolution over long ranges.
- No user-facing “requested series too large” errors; user is never required to increase interval.
- Episodic relationships must rank highly (correlation “bursts” separated by long weak periods).
- Outliers must not dominate ranking.
- One UI range acts as the maximum horizon; engine searches smaller windows automatically.
- Rust for production backend/services.
- Single-agent execution is REQUIRED for any pending/incomplete ticket; no multi-agent Collab Harness required.

Audit notes (verified in repo code; Jan 2026):
- Trends → “Related sensors” is currently client-driven:
  - `apps/dashboard-web/src/features/trends/components/AutoComparePanel.tsx`
  - loops `/api/metrics/query` via `apps/dashboard-web/src/features/trends/utils/metricsBatch.ts`
  - computes correlation/event ranking in-browser (`relatedSensors.ts`, `eventMatch.ts`)
- `/api/metrics/query` uses cursor-based paging in Rust (no “Requested series too large” hard-fail):
  - `apps/core-server-rs/src/routes/metrics.rs` (auto-paged responses via `cursor` + `next_cursor`)
  - dashboard-web `fetchMetricsSeries` follows `next_cursor` and merges pages
- Similar scan loops exist in other panels (e.g. Co-occurrence focus scan).

Committed architecture (unambiguous; no analysis fallback):
- All similarity scans run as server-side analysis jobs reading Parquet via embedded DuckDB, with Qdrant as the required ANN stage.
- Postgres/Timescale remains the authoritative archive (unbounded retention) + metadata store (jobs/results), but is not used as a “scan everyone” analysis engine.

---

## Ticket List

P0 = critical path.

- **TSE-0001 (P0)** ✅ Done: TSSE requirements + success metrics + design ADR
- **TSE-0002 (P0)** ✅ Done: Analysis Jobs framework (server-side, Rust)
- **TSE-0003 (P0)** ✅ Done: Analysis API surface (create/job/progress/result/preview)
- **TSE-0004 (P0)** ✅ Done: Parquet “analysis lake” spec (90d hot, sharded partitions)
- **TSE-0005 (P0)** ✅ Done: Postgres → Parquet replication (incremental + backfill + compaction)
- **TSE-0006 (P0)** ✅ Done: DuckDB embedded query layer (Rust) for Parquet reads
- **TSE-0007 (P0)** ✅ Done: Qdrant local deployment + schema (required ANN stage)
- **TSE-0008 (P0)** ✅ Done: Feature/embedding pipeline (robust, multi-scale signatures)
- **TSE-0009 (P0)** ✅ Done: Candidate generation service (Qdrant + filters + recall safeguards)
- **TSE-0010 (P0)** ✅ Done: Exact episodic similarity scoring (robust stats + multi-window + lag)
- **TSE-0011 (P0)** ✅ Done: Related Sensors scan job (end-to-end; never error)
- **TSE-0012 (P0)** ✅ Done: Preview/episode drilldown endpoints (small payloads; chart-friendly)
- **TSE-0013 (P1)** ✅ Done: Dashboard-web refactor for Related Sensors (job-based UX)
- **TSE-0014 (P1)** ✅ Done: Migrate Relationships/Correlation matrix to analysis jobs
- **TSE-0015 (P1)** ✅ Done: Migrate Events/Spikes matching to analysis jobs
- **TSE-0016 (P1)** ✅ Done: Migrate Co-occurrence to analysis jobs
- **TSE-0017 (P1)** ✅ Done: Migrate/Redesign Matrix Profile as a job (scoped + safe)
- **TSE-0018 (P1)** ✅ Done: Replace “series too large” failures in chart metrics path (paging/streaming)
- **TSE-0019 (P0)** ✅ Done: Performance + scale benchmarks on Mac mini (gates + regressions)
- **TSE-0020 (P0)** ✅ Done: Observability + “why ranked” explanations + profiling hooks
- **TSE-0021 (P2)** ✅ Done: NAS readiness (cold partition placement + config + smoke tests)
- **TSE-0022 (P1)** ✅ Done: Security hardening for analytics plane (paths, perms, authz)

---

## Dependency Sketch

1) Core platform: 0001 → 0002 → 0003
2) Data plane: 0004 → 0005 → 0006
3) ANN plane: 0007 → 0008 → 0009
4) Algorithms: 0010 → 0011 → 0012
5) UX + other analyses: 0013 → 0014/0015/0016/0017
6) Quality: 0019 + 0020 (throughout)
