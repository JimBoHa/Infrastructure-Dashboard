# RUN-20260206 Tier A — DW-221 (0.1.9.251-related-unified-v2)

## Context

- **Date:** 2026-02-06
- **Task:** **DW-221** — Trends: Related Sensors v2 unified refresh (Simple/Advanced + unified backend job)
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and validate the unified Related Sensors flow on the installed stack.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.250-tsse36-ui-polish` (from `http://127.0.0.1:8800/api/status`)
- Rollback target (previous version): `0.1.9.249-derived-builder-guardrails`

## Build (controller bundle DMG)

- **Version:** `0.1.9.251-related-unified-v2`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.251-related-unified-v2.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.251-related-unified-v2.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.251-related-unified-v2 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.251-related-unified-v2.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.251-related-unified-v2.log
```

Result:

- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.251-related-unified-v2.dmg`
- `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.251-related-unified-v2.dmg`

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.251-related-unified-v2.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Upgrade result:

- `Upgraded to 0.1.9.251-related-unified-v2`

Post-upgrade installed version:

- `current_version`: `0.1.9.251-related-unified-v2`
- `previous_version`: `0.1.9.250-tsse36-ui-polish`

(from `curl -fsS http://127.0.0.1:8800/api/status | jq '.result | {current_version, previous_version}'`)

## Validation

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Evidence (captured + viewed)

- Screenshot capture run:

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

- Full capture folder: `manual_screenshots_web/20260206_001950/`
- DW-221 evidence folder:
  - `manual_screenshots_web/tier_a_0.1.9.251_dw221_related_sensors_unified_20260206_0023/trends_related_sensors_large_scan.png`
  - `manual_screenshots_web/tier_a_0.1.9.251_dw221_related_sensors_unified_20260206_0023/trends_related_sensors_scanning.png`

## Notes

- The installed Trends UI shows the unified Related Sensors panel with Simple mode controls, quick suggestions flow, and ranked evidence cards/preview on the upgraded installed bundle.
- One screenshot step for `trends_cooccurrence` in the generic sweep reported a non-fatal locator timeout and fell back to viewport capture; this did not affect DW-221 evidence screenshots above.
