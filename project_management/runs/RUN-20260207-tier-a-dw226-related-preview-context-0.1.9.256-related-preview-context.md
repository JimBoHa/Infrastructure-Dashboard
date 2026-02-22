# RUN-20260207 Tier A — DW-226 (0.1.9.256-related-preview-context)

## Context

- **Date:** 2026-02-07
- **Task:** **DW-226** — Trends: Related Sensors preview context widening + episode highlight (fix “episode preview too zoomed-in on x-axis”)
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and validate the Related Sensors “Episodes” preview chart is interpretable by default.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.255-related-diurnal-penalty` (from `http://127.0.0.1:8800/api/status`)
- Rollback target (previous version) DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.255-related-diurnal-penalty.dmg`
- Worktree gate: clean (Tier A hard gate)

## Build (controller bundle DMG)

- **Version:** `0.1.9.256-related-preview-context`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.256-related-preview-context.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.256-related-preview-context \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.256-related-preview-context.log
```

Result:

- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg`
- `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg`

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Upgrade result:

- `Upgraded to 0.1.9.256-related-preview-context`

Post-upgrade installed version:

- `current_version`: `0.1.9.256-related-preview-context`
- `previous_version`: `0.1.9.255-related-diurnal-penalty`

(from `curl -fsS http://127.0.0.1:8800/api/status`)

## Validation

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Evidence (captured + viewed)

Screenshot capture run:

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

- Full capture folder: `manual_screenshots_web/20260207_004704/`
- DW-226 evidence (captured + viewed):
  - `manual_screenshots_web/20260207_004704/trends_related_sensors_scanning.png`
  - `manual_screenshots_web/20260207_004704/trends_related_sensors_large_scan.png`

## Notes

- Related Sensors preview now defaults to a widened preview window around the selected episode (Context: `Auto`) while highlighting the actual episode interval (band + start/end markers), which prevents the chart from being “too zoomed-in” on the x-axis by default.

