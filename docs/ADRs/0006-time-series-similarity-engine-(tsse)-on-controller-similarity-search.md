# 0006. Time-Series Similarity Engine (TSSE) on-controller similarity search

* **Status:** Proposed
* **Date:** 2026-01-23

## Context
The Trends UI has multiple “scan” style features (especially “Related sensors”) that currently:
- fetch metrics in a client-side loop (many `/api/metrics/query` requests),
- run heavy correlation/event computations in-browser, and
- hit hard backend caps (historically: “Requested series too large”).

This is the wrong architecture for a production controller running on a single Mac mini:
- It is slow and fragile for long ranges at high resolution.
- It creates “death by a thousand requests” patterns.
- It does not scale to ~1,000 sensors.
- It prevents explainability (“why ranked”) because the job state and intermediate phases are not durable/inspectable.

We need a first-class on-controller similarity engine that:
- never forces the user to change interval (“slow but progressing” is acceptable; “error” is not),
- ranks episodic relationships (bursty correlation) highly,
- is outlier-robust, and
- is observable, debuggable, and cancelable.

## Decision
Implement TSSE as a server-side analytics plane running on the controller (macOS Mac mini), with a fixed and explicit architecture:

1) **Durable analysis jobs** (create/progress/cancel/result) in `apps/core-server-rs`
2) **Parquet “analysis lake”** as a read-optimized projection of metrics for the hot horizon (90d) on local SSD
3) **Embedded DuckDB** for Parquet reads in analysis jobs (no DuckDB daemon)
4) **Qdrant** as the required ANN candidate stage (launchd-managed; no runtime downloads)
5) **Exact episodic scoring** in Rust that produces explainable episodes and “why ranked” fields

The controller’s Postgres/Timescale DB remains the authoritative archive (unbounded retention) and stores job metadata/results, but is not used as a “scan everyone” analytics engine.

### API contract (stable surface)
Expose analysis APIs under `/api/analysis`:
- `POST /api/analysis/jobs` (create)
- `GET /api/analysis/jobs/{id}` (status)
- `GET /api/analysis/jobs/{id}/events` (progress/events polling)
- `POST /api/analysis/jobs/{id}/cancel` (cancel)
- `GET /api/analysis/jobs/{id}/result` (typed result payload per job type)
- `POST /api/analysis/preview` (bounded chart-friendly drilldown around an episode)

Capabilities:
- `analysis.run` for creating/canceling analysis jobs
- `analysis.view` for reading job status/results/previews

### Similarity model (TSSE v1)
TSSE v1 similarity is defined as **episodic agreement** between two time series over a user-selected *maximum horizon*.

Inputs:
- Focus sensor `F` and candidate sensor `C`
- Max horizon `[start,end]` (UI range is the maximum horizon)
- A base grid interval `Δ` (seconds). `Δ` comes from the UI interval when provided; internally the engine may increase `Δ` to keep computation bounded, but must never reject a request due to size.

Preprocessing (outlier-robust):
- Align `F` and `C` onto the base grid interval `Δ`.
- Compute robust normalization per series:
  - center = median
  - scale = MAD (with standard MAD→σ factor)
  - robust z-score per bucket
- Winsorize/clamp robust z to a bounded range (default ±5) so single spikes don’t dominate.
- Optional derivative channel: compute `ΔF(t)` and `ΔC(t)` on the aligned grid and apply the same robust normalization.

Lag handling (lag-aware, bounded):
- Search lag `ℓ` within `[-ℓ_max, +ℓ_max]` (default: configurable; bounded to avoid explosion).
- Perform a coarse lag sweep (on the aligned grid) and refine lag only for top candidates/episodes.

Multi-window scan (episodic):
- Evaluate a fixed set of window sizes within the max horizon (defaults; bounded by `[start,end]`):
  - 5m, 30m, 2h, 12h, 1d, 7d (subject to `Δ` and horizon length)
- For each window size and lag, compute a window-level similarity score (robust correlation on clamped robust-z values).
- Convert windows into episodes by thresholding and merging adjacent windows.

Episode extraction:
- A candidate is “episodically related” if it has one or more episodes whose window-level score exceeds a threshold and whose overlap bucket count exceeds a minimum.
- Each episode emits:
  - `start_ts`, `end_ts`
  - `window_sec`
  - `lag_sec` (+ optional lag dispersion)
  - `score_mean`, `score_peak`
  - `coverage` (episode duration / horizon duration)
  - `num_points` (overlap count)

Ranking:
- Final rank score is an aggregate of episode strength and coverage across windows, with explicit penalties:
  - penalty for low overlap, tiny single-window spikes, and extremely short episodes
  - optional penalty for unstable lag estimates
- Each result includes a stable `why_ranked` object:
  - episode count
  - best window/lag
  - coverage %
  - score components (named)
  - penalties/bonuses (string list)

Determinism:
- For fixed inputs (same lake contents, same params), output ordering is deterministic.

### Candidate generation (ANN stage)
Candidate generation is performed by Qdrant over a versioned multi-vector schema:
- `value` vector (128 dims): multi-scale value signature
- `delta` vector (128 dims): derivative signature
- `event` vector (64 dims): spike/event signature

Candidate strategy:
- Run multiple Qdrant searches (one per vector) and union the results.
- Apply high-recall safeguards:
  - enforce minimum candidate pool size (`min_pool`)
  - adaptive widening (increase `k`, relax filters) when results are insufficient
  - capture reasons per candidate (which vectors matched + scores/ranks)
- Filters (configurable):
  - same node
  - same unit/type
  - exclude selected sensors

### Data freshness semantics
- The Parquet lake stores the hot horizon (default 90 days).
- Each job result includes `computed_through_ts` watermark indicating the newest timestamp included in the lake at compute time.
- Postgres remains the authoritative archive but is not used for “scan everyone” computations.

### “Never error” enforcement
- `/api/metrics/query` must not hard-fail on point caps; it must page/stream instead.
- Analysis jobs must accept long-range/high-res requests and progress (bounded by job-level safety limits and cancelability).
- Preview responses are bounded by `max_points` and/or maximum duration and must not trigger size errors.
- Unsafe/abusive parameter sets are rejected early with clear 400s (e.g., invalid timestamps), but not due to “too many points”.

### Performance targets (Mac mini)
These are explicit budget targets that will be enforced by benchmarks (TSE-0019):
- Candidate generation p50 <= 250ms, p95 <= 750ms (local Qdrant, warmed)
- Preview endpoint p50 <= 250ms, p95 <= 750ms for typical episode windows
- Related sensors scan: bounded exact stage; overall latency depends on candidate pool + horizon but must remain cancelable and observable. The engine must provide phase timings so regressions are detectable.

### Test plan mapping (requirements → gates)
- Outlier robustness: unit tests on robust z + clamping; scoring tests with injected spikes.
- Episodic ranking: curated pair fixtures where bursty windows must rank above weak global correlations.
- Multi-window: tests that strongest episode is found when it occurs in a smaller window than the max horizon.
- Lag: tests that known lagged relationships report lag and align in preview.
- “Never error”: API tests verifying no “series too large” errors; job + preview bounded responses.
- Performance: benchmark harness on Mac mini with p50/p95 thresholds stored under `reports/`.

## Consequences
Benefits:
- Removes fragile client-side scan loops and makes long-range/high-res scans feasible.
- Makes similarity explainable via episodes + “why ranked” fields and structured progress events.
- Centralizes performance and safety controls on the controller (bounded concurrency, cancel, abuse limits).
- Provides an unambiguous production architecture (no hidden DB-fallback scans).

Costs / risks:
- Adds storage/ops complexity (Parquet lake + Qdrant service).
- Requires careful security hardening (paths/perms/authz/abuse limits).
- Requires ongoing performance hygiene (benchmarks and profiling hooks) to avoid regressions.

Mitigations:
- Keep every surface versioned (job types, embeddings, result schemas).
- Use explicit runbooks and Tier‑A validation evidence for controller upgrades.
- Enforce benchmark gates and add phase-level timing logs for jobs.
