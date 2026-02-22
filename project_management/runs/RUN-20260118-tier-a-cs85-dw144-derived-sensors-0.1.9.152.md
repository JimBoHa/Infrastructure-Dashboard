# RUN-20260118 — Tier A — CS-85 + DW-144 Derived Sensors (0.1.9.152)

- **Date:** 2026-01-18
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.152
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.152.dmg`

## Scope

- CS-85: Derived sensors (computed from other sensors; query-time evaluation; no ingest; no implicit fills).
- DW-144: Derived sensor creation UI in Sensors & Outputs → Add sensor drawer + derived definition transparency in Sensor detail.
- Sensors & Outputs: no nodes auto-expanded on navigation (collapsed-by-default UX).

## Build bundle

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.152 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.152.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.152.dmg`.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.152.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.152` (previous `0.1.9.151`).

## Tier-A UI evidence (screenshots)

Captured + opened (viewed):

- `manual_screenshots_web/tier_a_0.1.9.152_derived_sensors_2026-01-18_070954307Z/01_sensors_nodes_collapsed.png`
- `manual_screenshots_web/tier_a_0.1.9.152_derived_sensors_2026-01-18_070954307Z/02_derived_sensor_builder_created.png`

Playwright command:

```bash
cd apps/dashboard-web
FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.152 \
npx playwright test playwright/derived-sensors-tier-a.spec.ts --project=chromium-mobile
```

Notes:
- The test creates a derived sensor via the UI and then soft-deletes it via API cleanup (keeps data).

## API smoke (derived query)

Confirmed that derived sensors evaluate in `/api/metrics/query` (non-empty time-series) against a real, non-forecast input sensor on the installed controller.

