# TSE-0020: Observability + profiling + “why ranked” explanations

Priority: P0
Status: In Progress (tracked as TSSE-21 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Make the TSSE diagnosable and trustworthy:
- deep visibility into job phases and resource usage
- easy profiling on-device
- explainable results (“why ranked”) suitable for UI display

## Scope
- Structured logs for:
  - job lifecycle
  - candidate selection
  - scoring phases
  - episode extraction
- Metrics:
  - job duration by phase
  - DB/Parquet scan time
  - Qdrant latency
  - cache hit rates
- Profiling hooks:
  - on-demand CPU profiles / flamegraphs (dev mode)
- Explainability fields:
  - episode counts, coverage, strength summaries
  - penalties applied

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: logging/metrics schema.
- Deliverable: profiling plan.
- Deliverable: UI explanation fields.

## Acceptance Criteria
- A single run can be traced end-to-end with clear phase timings.
- “why ranked” fields available for each suggestion.

## Audit (repo state as of 2026-01-24)

### Durable job surfaces (DB)
- `analysis_jobs`: status + `progress` JSON (phase/completed/total/message) + `error` JSON (code/message/details).
- `analysis_job_events`: kind + payload. Currently used kinds:
  - `created` (payload includes `job_type`, `job_key`)
  - `started` / `completed` / `failed` (payload currently `{}`)
  - `progress` (payload includes phase/completed/total/message)
  - `cancel_requested` / `canceled` (payload includes `before_start` only for canceled)
- `analysis_job_results`: JSON result only for completed jobs.

### TSSE job results (API)
- Results usually include `timings_ms` (but keys/units vary by job).
- `related_sensors_v1` includes rich `why_ranked` + `episodes` per candidate and also emits `tracing::info!` logs with phase timings.
- Other jobs generally do **not** emit structured phase logs; they update `progress` and (sometimes) return `timings_ms`.

## Missing Observability (minimal, high-value)

### Consistent correlation fields (logs + events)
- `job_id`, `job_type`, `job_key`, `created_by` should be present on *every* job lifecycle log line.
- A single top-level `analysis_job` tracing span should wrap execution so downstream logs inherit context.

### Phase timings that survive failures
- `timings_ms` only exists for completed jobs. Failed/canceled jobs lose timing visibility.
- Add durable, best-effort `analysis_job_events` for phase completions (e.g. `phase_timing` events).

### Structured phase + workload stats
Minimal per-phase payload should include:
- `phase` (stable name aligned to progress phases)
- `duration_ms`
- key counters (kept small):
  - DuckDB: `sensor_count`, `bucket_count` or `rows`, `parquet_files` (if available)
  - Qdrant: `requests`, `hits`, `limit`
  - Scoring: `candidates_scored`, `episodes_total`

### Explainability / “why ranked” coverage
- `related_sensors_v1` already returns `why_ranked` + `episodes`.
- Other ranked outputs (e.g. `event_match_v1`) currently return rank + raw components, but do not surface a consistent `why_ranked` object.

## Recommended Minimal Code Changes

1) **Runner-level tracing span**
   - Wrap each job execution in a `tracing::info_span!("analysis_job", ...)` including `job_id/job_type/job_key/created_by`.
   - Emit lifecycle logs for `claimed`, `completed`, `failed`, `canceled` including `duration_ms`.

2) **Phase timing events (DB)**
   - Append best-effort `analysis_job_events` with kind `phase_timing` at each major phase boundary:
     - aligned to `progress.phase` names (and a few job-specific subphases as needed).
   - Include `duration_ms` + small counters; do not include large arrays.

3) **Standardize `timings_ms` keys (additive)**
   - Add `job_total_ms` to all job results (keep existing keys for backward compatibility).
   - Keep `*_ms` keys strictly “milliseconds”; move counts out of `timings_ms` (or add a separate `counts` map) where possible.

4) **Add `why_ranked` to other ranked jobs (additive)**
   - For `event_match_v1`, add a `why_ranked` summary per candidate (reuse `TsseWhyRankedV1`).
   - Populate: `episode_count`, `best_lag_sec`, `best_window_sec`, `coverage_pct`, `score_components` (`f1`, `overlap`, `n_focus`, `n_candidate`).
