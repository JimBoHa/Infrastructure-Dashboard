# RUN-20260123 Tier A — DW-189/DW-190/DW-191 (Map refactor) — 0.1.9.199

## Summary

Validated the Map tab refactor on the **installed controller** (Tier A; no DB/settings reset). Confirmed the new Map tab loads and offline pack + base-layer UI renders correctly, with screenshot evidence captured and reviewed.

## Versions

- Previous installed: `0.1.9.198`
- Updated installed: `0.1.9.199`

## Commands / Evidence

### Preflight (installed controller healthy)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
```

### Repo gate (Tier A hard gate)

```bash
cd /Users/FarmDashboard/farm_dashboard
git status --porcelain=v1 -b
```

### Build bundle (DMG)

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.199 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.199.dmg \
  --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.199.log
```

### Configure + upgrade installed controller

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.199.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

### Tier A smoke

```bash
make e2e-installed-health-smoke
```

### UI screenshots (captured + reviewed)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

- Screenshot dir: `manual_screenshots_web/20260122_211457/`
- Map screenshot: `manual_screenshots_web/20260122_211457/map.png`

## Outcome

- Tier A: PASS (installed controller refreshed to `0.1.9.199`).
- `make e2e-installed-health-smoke`: PASS.
- Screenshot evidence captured and reviewed; Map tab renders successfully post-upgrade.

