# RUN-20260210 Tier A — DW-249 (`0.1.9.262-dw249-missingness`)

## Summary
- Date: 2026-02-10
- Scope: Tier A validation for DW-249 / TICKET-0058 (Unified v2 data quality filtering + missingness/coverage surfacing)
- Commit: `206f712`
- Installed baseline before upgrade: `0.1.9.261-ts9-node-health-visible`
- Upgrade target: `0.1.9.262-dw249-missingness`
- Bundle path: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.262-dw249-missingness.dmg`

## Preconditions
- Setup daemon health:
  - `curl -fsS http://127.0.0.1:8800/healthz` -> `{"status":"ok"}`
- Core server health:
  - `curl -fsS http://127.0.0.1:8000/healthz` -> `{"status":"ok"}`
- Rollback target captured from status before upgrade:
  - current version: `0.1.9.261-ts9-node-health-visible`

## Build (controller bundle DMG)
```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.262-dw249-missingness \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.262-dw249-missingness.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.262-dw249-missingness.log
```

Result:
- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.262-dw249-missingness.dmg`

## Upgrade (setup-daemon)
Set bundle path:
```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.262-dw249-missingness.dmg"}'
```

Trigger upgrade:
```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Post-upgrade version check:
- `current_version`: `0.1.9.262-dw249-missingness`
- `previous_version`: `0.1.9.261-ts9-node-health-visible`

(from `farmctl status --config /Users/Shared/FarmDashboard/setup/config.json`)

## Tier A Validation
Health checks:
```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
/usr/local/farm-dashboard/bin/farmctl health --config /Users/Shared/FarmDashboard/setup/config.json
```

Output:
- setup-daemon: `{"status":"ok"}`
- core-server: `{"status":"ok"}`
- `farmctl health`: core_api/dashboard/mqtt/database/redis all `ok`

Installed smoke:
```bash
make e2e-installed-health-smoke
```

Output:
- `e2e-installed-health-smoke: PASS`

## UI Evidence (captured + viewed)
Screenshot sweep command:
```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Screenshot directories:
- `manual_screenshots_web/20260210_142534/`
- `manual_screenshots_web/tier_a_0.1.9.262-dw249-missingness_unified_preview_2026-02-10_223632415Z/`

Viewed Trends evidence:
- `manual_screenshots_web/tier_a_0.1.9.262-dw249-missingness_unified_preview_2026-02-10_223632415Z/trends_related_sensors_unified_preview.png`

Additional captured Trends artifacts:
- `manual_screenshots_web/20260210_142534/trends.png`
- `manual_screenshots_web/20260210_142534/trends_cooccurrence.png`
- `manual_screenshots_web/20260210_142534/trends_related_sensors_large_scan.png`
- `manual_screenshots_web/20260210_142534/trends_related_sensors_scanning.png`
- `manual_screenshots_web/20260210_142534/trends_short_range.png`

## Outcome
- Tier A: PASS on installed controller.
- Installed version serving DW-249 changes: `0.1.9.262-dw249-missingness`.
- Tier B clean-host E2E remains tracked by cluster ticket `DW-98`.

