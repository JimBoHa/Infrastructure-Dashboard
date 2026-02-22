# TSE-0004: Parquet “analysis lake” spec (90d hot, sharded partitions)

Priority: P0
Status: Done (tracked as TSSE-5 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Define and implement the on-disk layout and metadata for an analysis-optimized metrics projection suitable for DuckDB scans on a single Mac mini.

## Key Constraints
- 90-day hot horizon must be fast.
- Postgres remains unbounded archive (authoritative).
- Avoid millions of tiny files.
- Must support eventual NAS placement for cold partitions.
- macOS-only controller runtime; files must live under the controller `data_root` (default `/Users/Shared/FarmDashboard`) and be writable by the service user (default `_farmdashboard`).

## Scope
- Choose Parquet schema for metrics projection:
  - required columns: `sensor_id`, `ts` (UTC), `value`, `quality`, optional `samples`, optional `source`.
- Partitioning + sharding strategy:
  - partition by date (`date=YYYY-MM-DD`)
  - shard by `hash(sensor_id) % N` to keep file counts manageable
  - sort rows by `(sensor_id, ts)` inside files
- File sizing targets:
  - avoid tiny files; target “hundreds of MB” per shard-day as feasible
- Manifest/watermark:
  - record what partitions exist, compaction status, and the “computed_through_ts” watermark
- Disk budget and retention policy for hot 90d.

## Implemented Layout (2026-01-23)
- Root paths (macOS only, under `data_root`):
  - hot lake: `${data_root}/storage/analysis/lake/hot`
  - cold lake (optional NAS): `${data_root}/storage/analysis/lake/cold` (via `CORE_ANALYSIS_LAKE_COLD_PATH`)
  - scratch: `${data_root}/storage/analysis/tmp`
- Dataset root: `metrics/v1`
- Partition layout:
  - `.../metrics/v1/date=YYYY-MM-DD/shard=NN/part-<run_id>.parquet`
  - backfill files: `backfill-<run_id>.parquet`
  - compaction output: `compact-<run_id>.parquet`
- Atomicity:
  - Parquet writes use `*.tmp` files in the target directory and `rename()` to finalize.
  - Manifest/state updates are written via temp file in `_state/` and persisted atomically.

## Manifest + Watermark (2026-01-23)
- Location: `${hot_path}/_state/manifest.json`
- Schema (current fields):
  - `datasets.<dataset>.computed_through_ts` (watermark)
  - `datasets.<dataset>.partitions.<YYYY-MM-DD>.location` = `hot|cold`
  - `datasets.<dataset>.partitions.<YYYY-MM-DD>.updated_at` (RFC3339)
  - `datasets.<dataset>.partitions.<YYYY-MM-DD>.last_compacted_at` (RFC3339)
  - `datasets.<dataset>.partitions.<YYYY-MM-DD>.file_count` (parquet files across shard dirs)
- Replication watermark is also stored in `${hot_path}/_state/replication.json` as `computed_through_ts` and mirrored into the manifest.

## Retention / Cold Policy (2026-01-23)
- Hot retention is enforced after replication ticks:
  - partitions older than `CORE_ANALYSIS_HOT_RETENTION_DAYS` are moved to cold if configured
  - otherwise they are deleted (hot-only deployment)
- Manifests are updated to reflect `location=cold` after moves.

## Compaction Trigger (2026-01-23)
- Compaction runs per shard-day when parquet file count exceeds 10.
- Output is a single `compact-<run_id>.parquet` file sorted by `(sensor_id, ts)`.

## Inspector (2026-01-23)
- `tsse_lake_inspector` outputs JSON including:
  - hot + cold paths
  - per-date partition stats (parquet file counts/bytes)
  - replication watermark metadata

## Integration points (verified)
- Installer config has a `data_root` (see `apps/farmctl/src/config.rs`), and launchd services run as a dedicated service user in prod.
- The spec must define canonical on-disk roots (suggested):
  - Analysis lake root: `${data_root}/storage/analysis/lake`
  - Qdrant storage root: `${data_root}/storage/qdrant`
  - Analysis job scratch: `${data_root}/storage/analysis/tmp`
- The chosen paths must be surfaced to the runtime via explicit env vars and/or setup config fields (so tests can relocate them).

## Collab Harness (REQUIRED)
- Worker A: propose Parquet schema + partition/shard design.
- Worker B: propose manifest format + atomic update strategy.
- Worker C: NAS-ready path abstraction and migration plan.

## Acceptance Criteria
- Written spec includes:
  - partition/shard scheme
  - file naming + atomic write rules
  - manifest/watermark rules
  - compaction triggers
  - hot 90d retention policy
- A minimal “lake inspector” CLI (Rust) can list partitions, shard counts, and watermarks.
