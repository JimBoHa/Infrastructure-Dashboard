# RUN-20260204 Tier A — CS-101 (0.1.9.247-derived-lag-buckets)

## Context

- **Date:** 2026-02-04
- **Task:** **CS-101** — Metrics: derived `lag_seconds` must work across bucket intervals (7d Trends)
- **Goal:** Refresh the installed controller (Tier A; no DB/settings reset) and confirm a real temp-comp derived sensor no longer goes blank on Trends at 7d (30m buckets).

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.246-temp-comp-lag` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)
- Rollback target (previous bundle):
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.246-temp-comp-lag.dmg`

## Build (controller bundle DMG)

- **Version:** `0.1.9.247-derived-lag-buckets`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.247-derived-lag-buckets.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.247-derived-lag-buckets.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.247-derived-lag-buckets \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.247-derived-lag-buckets.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.247-derived-lag-buckets.log
```

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.247-derived-lag-buckets.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Post-upgrade installed version: `0.1.9.247-derived-lag-buckets` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)

## Validation

- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Evidence

- API proof (7d + 30m buckets returns non-empty derived history):

```bash
TOKEN=\"$(cat /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt)\"
END=$(date -u \"+%Y-%m-%dT%H:%M:%SZ\")
START=$(date -u -v-7d \"+%Y-%m-%dT%H:%M:%SZ\")

curl -fsS \\
  -H \"Authorization: Bearer $TOKEN\" \\
  \"http://127.0.0.1:8000/api/metrics/query?sensor_ids=adab2ed19bb1b9dfe189fa81&start=${START}&end=${END}&interval=1800&format=json\" \\
  | jq '.series[0] | {sensor_id, points: (.points | length)}'
```

- Screenshot (captured + viewed):
  - `manual_screenshots_web/tier_a_0.1.9.247-derived-lag-buckets_cs101_trends_7d_2026-02-04_062902444Z/trends_7d_temp_comp_depth.png`

## Notes

- Root cause: derived evaluation looked up inputs at `epoch - lag_seconds` and required an **exact bucket-epoch match**. When `lag_seconds` is not divisible by the query `interval` (e.g., 12,300s lag with 1,800s buckets), lookups miss bucket boundaries and the derived series becomes empty.
- Fix: when the desired lookup epoch is misaligned, floor it to the nearest bucket boundary; also expand the derived-input read window by one interval on either side so the snapped bucket is available. Aligned lookups and real data gaps keep prior semantics (no carry-forward).

