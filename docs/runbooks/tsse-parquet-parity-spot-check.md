# Runbook: TSSE Parquet Parity Spot-Check (Postgres <-> Parquet)

This runbook provides a low-risk, read-only spot-check to confirm that the TSSE Parquet lake matches Postgres metrics for a few sensors and a small time window.

## Prereqs
- Controller has Postgres running and the Parquet lake populated (replication enabled).
- Repo checkout with Rust tooling available (`cargo`).

## Safety Notes
- Read-only: does not write to Postgres or Parquet.
- Keep the window small (<= 1-2 hours) to avoid heavy scans.
- Prefer a window **older than** `computed_through_ts` (replication watermark) to avoid expected lag.

## Step 1: Pick sensor IDs
Option A (Dashboard): copy sensor IDs from the Sensors/Outputs tab.

Option B (Postgres):

```bash
DB_URL=$(python3 - <<'PY'
import json
path = "/Users/Shared/FarmDashboard/setup/config.json"
with open(path, "r") as f:
    print(json.load(f).get("database_url", ""))
PY
)

/usr/local/farm-dashboard/native/postgres/bin/psql "$DB_URL" -c \
  "SELECT sensor_id, name FROM sensors ORDER BY created_at DESC LIMIT 10;"
```

## Step 2: Run the spot-check (recommended)

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_parity_check -- \
  --mode api \
  --api-base-url http://127.0.0.1:8000 \
  --auth-token-file /path/to/token.txt \
  --start 2026-01-24T11:00:00Z \
  --end   2026-01-24T12:00:00Z \
  --sensor-ids <sensor_id_1>,<sensor_id_2> \
  --report reports/tsse-lake-parity-YYYYMMDD_HHMM.md
```

Optional overrides:
- `--sample 5` (server picks sensors when `--sensor-ids` is omitted)
- `--fail-on-mismatch` (non-zero exit if mismatches are found)

## Output Interpretation
- **OK**: Postgres point counts match Parquet point counts for the checked window.
- **MISMATCH**: investigate replication lag (ensure the window end is older than `computed_through_ts`).
- **No Parquet files**: replication may be behind or the lake path is incorrect.

## Alternate: direct filesystem spot-check (advanced)

If you are running as the controller service user (or otherwise have permission to read the lake
paths), you can use `ops_tsse_parity_spotcheck`. This is still read-only, but may fail with
permission errors on hardened Tier‑A installs.

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin ops_tsse_parity_spotcheck -- \
  --sensor-id <sensor_id_1> \
  --sensor-id <sensor_id_2> \
  --window-minutes 60
```

## If You See Mismatches
1) Re-run with an older `--end` time (older than `computed_through_ts`).
2) Check replication logs in core-server output for errors.
3) If data is missing for a larger window, run the backfill job and re-check.

## Notes
- This is a spot-check, not a full audit. It is meant for quick operator verification after replication changes or incidents.
