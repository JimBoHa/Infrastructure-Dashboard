# RUN-20260209 Tier A — DW-230 (`0.1.9.259-dw230-trends-bestfit`)

## Summary
- Date: 2026-02-09
- Scope: Tier A validation for DW-230 (Trends chart analysis toolbar v2: drag best-fit windows, multi-window fit cards, explicit save/update/delete, persisted best-fit hydration)
- Commit: `54216b9`
- Installed baseline before upgrade: `0.1.9.258-cs104-dhcp-safe`
- Upgrade target: `0.1.9.259-dw230-trends-bestfit`
- Bundle path: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.259-dw230-trends-bestfit.dmg`

## Preconditions
- Setup daemon health:
  - `curl -fsS http://127.0.0.1:8800/healthz` -> `{"status":"ok"}`
- Core server health:
  - `curl -fsS http://127.0.0.1:8000/healthz` -> `{"status":"ok"}`
- Rollback target captured from setup status/config before upgrade:
  - current version: `0.1.9.258-cs104-dhcp-safe`
  - previous version: `0.1.9.256-related-preview-context`

## Build (controller bundle DMG)
```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.259-dw230-trends-bestfit \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.259-dw230-trends-bestfit.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.259-dw230-trends-bestfit.log
```

Result:
- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.259-dw230-trends-bestfit.dmg`
- `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.259-dw230-trends-bestfit.dmg`

## Upgrade (setup-daemon)
Set bundle path:
```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.259-dw230-trends-bestfit.dmg"}'
```

Trigger upgrade:
```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Upgrade result highlights:
- `farmctl returncode: 0`
- `farmctl stdout: Upgraded to 0.1.9.259-dw230-trends-bestfit`
- benign note from farmctl stderr: `xattr ... No such xattr: com.apple.quarantine`

Post-upgrade version check:
- `current_version`: `0.1.9.259-dw230-trends-bestfit`
- `previous_version`: `0.1.9.258-cs104-dhcp-safe`

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
- `farmctl health`: core API/dashboard/mqtt/database/redis all `ok`

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

Screenshot directory:
- `manual_screenshots_web/20260209_005651/`

Viewed Trends evidence:
- `manual_screenshots_web/20260209_005651/trends.png`

Additional captured Trends artifacts:
- `manual_screenshots_web/20260209_005651/trends_short_range.png`
- `manual_screenshots_web/20260209_005651/trends_related_sensors_large_scan.png`
- `manual_screenshots_web/20260209_005651/trends_related_sensors_scanning.png`

## Outcome
- Tier A: PASS on installed controller.
- Installed version serving DW-230 UX changes: `0.1.9.259-dw230-trends-bestfit`.
- Tier B clean-host E2E remains tracked by cluster ticket `DW-98`.
