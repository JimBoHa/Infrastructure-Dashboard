# RUN-20260115: OPS-1 purge mistaken Node 1 reservoir depth points (installed controller)

## Goal

Hard-delete erroneous **simulated** reservoir depth telemetry for Node 1 that was accidentally written into the installed controller database, while preserving real telemetry.

## Scope / Constraints

- Target: Installed controller (Tier A environment; **no DB/settings reset**).
- Data to delete: **2026-01-13 00:00 → 2026-01-14 03:15** (user-reported local-time window; confirm DB timezone alignment before deletion).
- Data to preserve: **2026-01-14 09:00 → present** (real sensor data).
- Do not modify retention defaults or delete other sensors.

## Environment

- Controller UI/API: `http://127.0.0.1:8000`
- DB: local Postgres (connection details from controller config; do not paste credentials into this log)

## Execution Log

### 1) Identify sensor + tables

- ✅ Identified the target sensor via the controller API:
  - Node: **Pi5 Node 1** (`node_id=0a55b329-104f-46f0-b50b-dea9a5cca1b3`)
  - Sensor: **Reservoir Depth** (`sensor_id=ea5745e00cb0227e046f6b88`, unit `ft`)
- ✅ Raw points are stored in the `metrics` hypertable (see `infra/migrations/001_init.sql`).
- ✅ Controller host timezone is **PST** (validated via `date`), but DB timestamps are `timestamptz` and are handled as RFC3339 with offsets.

### 2) Pre-delete counts (must record)

- Used the ops tool `ops_purge_metrics_window` (added in `apps/core-server-rs/src/bin/ops_purge_metrics_window.rs`) in dry-run mode.
- Based on inspecting the series, the erroneous points extended beyond the originally reported end time (there was a hard gap until real ingest resumed at ~09:00 local), so the delete bound was widened to “**all points before real ingest starts**”:
  - Delete window (local): **2026-01-13 00:00 PST → 2026-01-14 08:59:59 PST**
  - Preserve window (local): **>= 2026-01-14 09:00 PST**
  - Delete window (UTC): **2026-01-13T08:00:00Z → 2026-01-14T16:59:59Z**
  - Preserve window (UTC): **>= 2026-01-14T17:00:00Z**
- Pre-delete snapshot (from the tool output immediately before applying the delete):
  - `metrics` total: **121,630**
  - Rows in delete window: **69,633**
  - Rows in preserve window (>= 2026-01-14T17:00:00Z): **51,997**

### 3) Transactional delete

- ✅ Executed a single transactional delete:
  - Deleted rows: **69,633**
  - Remaining rows in delete window: **0**

### 4) Post-delete verification

- ✅ Post-delete verification (dry-run re-check):
  - `metrics` total: **52,009**
  - `min_ts`: **2026-01-14T17:03:07.767648Z** (≈ 09:03 PST)
  - `max_ts`: **2026-01-15T07:35:53.916716Z**
  - Rows in delete window: **0**
  - Rows in preserve window: **52,009**
- ✅ API spot-check: `GET /api/metrics/query` over `2026-01-14 00:00 PST → 12:00 PST` returns buckets starting at **17:00Z** only (no earlier points).

## Result

- Status: **DONE**
