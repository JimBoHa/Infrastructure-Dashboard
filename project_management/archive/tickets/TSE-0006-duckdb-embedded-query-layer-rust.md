# TSE-0006: DuckDB embedded query layer (Rust) for Parquet reads

Priority: P0
Status: Done (tracked as TSSE-7 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Provide a Rust API for reading high-res metrics from the Parquet analysis lake via embedded DuckDB, with predictable performance and bounded memory.

## Scope
- Embed DuckDB (no separate DuckDB daemon). Preferred placement: inside the production controller runtime (`apps/core-server-rs`) as a dedicated analysis module/service so the pipeline stays unambiguous (Qdrant is the only additional daemon).
- Provide query helpers:
  - fetch a sensor series for `[start,end]` at native resolution (no forced downsampling)
  - fetch aligned series for a set of sensors (candidate batch) for a given interval grid
  - fetch “window slices” around episodes for preview
- Push down filtering: only read necessary columns/partitions/shards.
- Ensure queries work with local disk and future NAS paths.
 - Concurrency: define a bounded “analysis threadpool” strategy so heavy DuckDB scans and exact scoring do not starve the interactive API.

## Implemented Query Layer (2026-01-23)
- `DuckDbQueryService` (Rust) in `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs`.
- Queries supported:
  - `read_metrics_points_from_lake`: native-resolution points for sensor IDs + time range.
  - `read_metrics_buckets_from_lake`: bucketed averages at a requested interval.
- Concurrency:
  - Bounded via a Tokio semaphore (`analysis_max_concurrent_jobs`, default 2) with DuckDB `PRAGMA threads=2`.
- Pruning:
  - Partition + shard selection via `list_parquet_files_for_range` using `date=` + `shard=` layout.
  - Manifest-aware location picking (hot/cold), fallback to filesystem scan.
- Cold-path reads:
  - DuckDB reads from cold partitions when manifest locations are `cold` (tested).

## Benchmark Harness (2026-01-23)
- `tsse_duckdb_bench` CLI:
  - Runs a representative `points` or `buckets` query over the lake and prints rows/sec.
  - Uses the same partition+shard pruning logic as production scans.

## Collab Harness (REQUIRED)
- Worker A: DuckDB Rust crate integration and performance notes.
- Worker B: query design (partition pruning, shard selection).
- Worker C: memory/CPU limits and concurrency rules.

## Acceptance Criteria
- Demonstrate partition pruning works (only relevant `date=` partitions scanned).
- Demonstrate shard pruning works (only relevant shard files scanned for a sensor_id set).
- Provide unit/integration tests verifying correct data returned.
- Provide a benchmark harness measuring read throughput for typical requests.
