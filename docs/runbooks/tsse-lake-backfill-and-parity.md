# TSSE Lake Backfill + Parity Spot-Check (TSE-0005)

This runbook is for **developer/operator validation** of the TSSE Parquet analysis lake on a macOS controller.

Goals:
- Run a 90‑day backfill into the Parquet lake (job-based; no manual SQL exports).
- Confirm the lake watermark (`computed_through_ts`) advances.
- Spot-check basic **Postgres ↔ Parquet** parity for a small sample (counts over a time window).

## Prereqs

- Controller services running (Tier‑A host):
  - core-server: `http://127.0.0.1:8000/healthz`
  - qdrant: `http://127.0.0.1:6333/healthz`
- An API token with the required capabilities to create analysis jobs.
- Lake paths configured (defaults shown below):
  - hot: `/Users/Shared/FarmDashboard/storage/analysis/lake/hot`
  - tmp: `/Users/Shared/FarmDashboard/storage/analysis/tmp`

## 1) Inspect current lake state

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_inspector -- \
  --mode api \
  --api-base-url http://127.0.0.1:8000 \
  --auth-token-file /path/to/token.txt
```

Look for:
- `datasets.metrics/v1.computed_through_ts`
- `replication.computed_through_ts`

Note: the controller’s lake files are owned by the service user and may not be readable directly
from a developer shell. Use `--mode api` on Tier‑A hosts.

## 2) Trigger a 90‑day backfill job

Recommended: use the helper CLI (handles auth + polling).

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_backfill -- \
  --auth-token-file /path/to/token.txt \
  --days 90 \
  --replace-existing \
  --wait
```

If you prefer raw API calls, you can POST manually (replace `TOKEN`):

```bash
curl -fsS -X POST http://127.0.0.1:8000/api/analysis/jobs \
  -H "Authorization: Bearer TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "job_type":"lake_backfill_v1",
    "params":{"days":90,"replace_existing":true},
    "dedupe":false
  }'
```

Save the returned `job.id`, then poll:

```bash
curl -fsS http://127.0.0.1:8000/api/analysis/jobs/<JOB_ID>
curl -fsS http://127.0.0.1:8000/api/analysis/jobs/<JOB_ID>/result
```

## 3) Re-inspect lake watermark + partitions

Re-run the inspector (same command as Step 1) and confirm:
- partitions exist for the expected `date=YYYY-MM-DD` range
- `computed_through_ts` is present and recent

## 4) Spot-check Postgres ↔ Parquet parity (counts)

This is a **sanity check**, not a full proof.

Recommendation: pick a window that ends **before** `computed_through_ts` to avoid false mismatches from in-flight replication.

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_parity_check -- \
  --mode api \
  --api-base-url http://127.0.0.1:8000 \
  --auth-token-file /path/to/token.txt \
  --start 2026-01-01T00:00:00Z \
  --end   2026-01-01T01:00:00Z \
  --sample 5 \
  --report reports/tsse-lake-parity-YYYYMMDD_HHMM.md
```

If you want to pin the sample:

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_parity_check -- \
  --mode api \
  --api-base-url http://127.0.0.1:8000 \
  --auth-token-file /path/to/token.txt \
  --start 2026-01-01T00:00:00Z \
  --end   2026-01-01T01:00:00Z \
  --sensor-ids sensor-a,sensor-b \
  --report reports/tsse-lake-parity-YYYYMMDD_HHMM.md \
  --fail-on-mismatch
```

## If parity mismatches occur

- First check whether your requested window exceeded the lake watermark:
  - `tsse_lake_inspector` → `replication.computed_through_ts`
- If the window is within watermark and mismatches persist:
  - Re-run backfill with `replace_existing=true` for the affected range.
  - If the issue is repeatable, treat it as a replication correctness bug and track it as a new task (do not mark TSSE-6 Done).
