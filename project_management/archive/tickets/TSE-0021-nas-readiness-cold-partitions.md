# TSE-0021: NAS readiness (cold partition placement + config + smoke tests)

Priority: P2
Status: Done (tracked as TSSE-22 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Prepare the Parquet analysis lake to optionally place older/cold partitions on a NAS over 10GbE while keeping hot 90d local.

## Scope
- Path abstraction:
  - separate `hot_path` (local SSD) and `cold_path` (NAS)
- Partition movement tool (Rust): move partitions safely and update manifests.
- DuckDB configuration for reading from NAS path.
- Smoke tests to validate mixed local+NAS queries.

## Implemented (2026-01-23)
- Config paths:
  - `CORE_ANALYSIS_LAKE_HOT_PATH` + `CORE_ANALYSIS_LAKE_COLD_PATH`.
- Manifest records per-date `location=hot|cold`.
- Partition move tool:
  - `tsse_lake_move_partition --date YYYY-MM-DD --target hot|cold [--apply] [--force]`.
  - Atomic rename with copy+delete fallback for cross-filesystem moves.
  - Updates manifest location + file counts on completion.
- Queries:
  - DuckDB reads use manifest location and fall back to hot if cold is missing.
- Tests:
  - Cold-path read test validates DuckDB can scan a cold partition.

## Collab Harness (REQUIRED)
- Worker A: storage layout + migration.
- Worker B: DuckDB query performance over NAS.
- Worker C: runbook.

## Acceptance Criteria
- Can move partitions without breaking queries.
- Manifests accurately reflect locations.
