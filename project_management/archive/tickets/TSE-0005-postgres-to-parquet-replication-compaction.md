# TSE-0005: Postgres → Parquet replication (incremental + backfill + compaction)

Priority: P0
Status: In Progress (tracked as TSSE-6 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Build a Rust replication pipeline that continuously exports metrics from Postgres/Timescale into the Parquet analysis lake (TSE-0004), supporting:
- initial backfill (90d)
- incremental updates (near-real-time)
- late-arriving data handling
- compaction to control file counts

This is a hard dependency for the “no analysis fallback” requirement: similarity scans must be able to rely on Parquet+DuckDB as the primary read path.

## Source tables (verified)
- Primary metrics: `metrics(sensor_id, ts, value, quality)` hypertable (Timescale). Created in `infra/migrations/001_init.sql`.
- Forecast series (if included in similarity): `forecast_points(...)` hypertable. Created in `infra/migrations/020_forecast_points.sql`.
- Derived sensors are computed in Rust today inside `/api/metrics/query` (`apps/core-server-rs/src/routes/metrics.rs`) by evaluating expressions over bucketed inputs; decide whether TSSE treats derived sensors as:
  - on-demand computed (focus/candidate) OR
  - materialized into the lake as a separate pipeline.

## Scope
- Backfill job:
  - export last 90 days for all sensors into partitioned/sharded Parquet
- Incremental job:
  - export new metrics since watermark
  - write new Parquet segments atomically
  - update manifest
- Late data strategy:
  - define tolerance window (e.g., allow 48h late)
  - route late points to “delta segments” and rely on compaction
- Compaction:
  - background merge segments into larger files
  - enforce ordering and remove duplicates

## Implemented Pipeline (2026-01-23)
- Incremental replication service:
  - Runs every `CORE_ANALYSIS_REPLICATION_INTERVAL_SECONDS` (default 60s).
  - Uses an `inserted_at` watermark (`replication.json:last_inserted_at`) and a replication lag guard (`CORE_ANALYSIS_REPLICATION_LAG_SECONDS`, default 300s).
  - Window: `metrics.inserted_at > last_inserted_at AND <= (now - lag)`; first run exports the last hour.
  - Writes per-date/per-shard CSV segments, converts via DuckDB `COPY` into Parquet, and atomically renames into the hot lake.
  - Updates manifest partition locations + file counts and dataset watermark (`computed_through_ts`).
- Backfill job:
  - Job type: `lake_backfill_v1` (create via `POST /api/analysis/jobs`).
  - Parameters: `days` (default 90), `replace_existing` (default true).
  - Exports by `ts` range but capped by the stable `inserted_at` watermark to avoid ingesting uncommitted points.
  - Writes `backfill-<run_id>.parquet` segments per shard/day.
- Late data strategy:
  - Late points are captured based on `inserted_at` watermark (not `ts`).
  - Replication lag provides a safety buffer; compaction merges/dedupes overlaps when triggered.

## Compaction + File Count Control (2026-01-23)
- Trigger: per shard/day, if parquet file count exceeds 10.
- Action: DuckDB reads all files and writes a single `compact-<run_id>.parquet` sorted by `(sensor_id, ts)` with `GROUP BY` to drop duplicates.
- Old segment files are removed after the compacted file is finalized.
- File counts and `last_compacted_at` are recorded in `manifest.json`.

## Verification / Spot Checks (Operator)
- Compare counts for a sensor + window:
  - Postgres: `SELECT count(*) FROM metrics WHERE sensor_id = '<id>' AND ts BETWEEN ...`
  - DuckDB: use `tsse_duckdb_bench --mode points` over the same window to validate row count + throughput.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable (DB): efficient export queries (COPY), indices, backfill strategy.
- Deliverable (Lake): atomic Parquet writes + compaction implementation.
- Deliverable (Ops): macOS launchd integration + resource throttling.

Visibility:
- Provide a runbook for: backfill, incremental sync, compaction, repair.

## Acceptance Criteria
- Can backfill 90d without manual steps (one command/runbook).
- Incremental sync keeps Parquet within a bounded lag (e.g., <5 minutes behind Postgres under normal load).
- Compaction keeps file counts bounded.
- End-to-end correctness: spot-check counts/timestamps between Postgres and Parquet for sample sensors.
