# RUN-20260122 Tier A — DW-188 (Trends Related sensors: week+1m large scans) — 0.1.9.198

## Goal

Validate that Trends → Related sensors supports **Range = 7 days** with **Interval = 1 min** without failing on the core-server metrics guardrail (`Requested series too large … max 25000`), by using adaptive batching plus a large-scan confirm/progress/cancel UX.

Ticket:
- DW-188 (Related sensors week+1m support + confirm/progress/cancel UX)

## Host / Preconditions (Tier A)

- Setup daemon: `curl -fsS http://127.0.0.1:8800/healthz`
- Core server: `curl -fsS http://127.0.0.1:8000/healthz`

## Repo state (required Tier-A gate)

- Repo: `/Users/FarmDashboard/farm_dashboard`
- Commit: `5e7cb29`
- Worktree: clean (`git status --porcelain=v1 -b`)

## Installed version

- Before: `0.1.9.197`
- After: `0.1.9.198`

Verify:
- `curl -fsS http://127.0.0.1:8800/api/status` → `current_version` / `previous_version`

## Tests (pre-upgrade)

- `make ci-web-smoke` (PASS; warnings only)
- `cd apps/dashboard-web && npm test` (PASS)
- `cd apps/dashboard-web && npm run build` (PASS)

## Build controller bundle DMG

Output paths:
- DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.198.dmg`
- Log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.198.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.198 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.198.dmg \
  --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.198.log
```

## Configure setup daemon bundle path

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.198.dmg"}'
```

## Upgrade (refresh installed controller)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

## Validation (Tier A)

- Installed smoke:
  - `make e2e-installed-health-smoke` (PASS)

- UI screenshots captured:
  - Command:
    - `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
  - Output dir:
    - `manual_screenshots_web/20260122_002811/`
  - Evidence:
    - `manual_screenshots_web/20260122_002811/trends_related_sensors_large_scan.png` (7d + 1m large-scan warning + Continue/Cancel)
    - `manual_screenshots_web/20260122_002811/trends_related_sensors_scanning.png` (progress bar + Cancel scan)

**Note:** Tier‑A policy requires screenshots to be **captured and viewed**; open at least the two evidence screenshots above to satisfy the “viewed” requirement.

