# RUN-20260121 — Tier A — DW-178 Trends co-occurring anomalies (0.1.9.193)

- **Date:** 2026-01-21
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.193
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.193.dmg`

## Scope

- DW-178: Add a Trends “Co-occurring anomalies” panel that:
  - Detects per-series spike/change events using robust MAD z-score on deltas.
  - Surfaces time buckets where multiple sensors have events in the same Interval bucket.
  - Supports “focus scan” (1 selected focus sensor → scan all sensors) to find other sensors spiking at the same time.
  - Highlights selected buckets on the Trend chart (markers) and includes in-section Keys.

## Build controller bundle DMG

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.193 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.193.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: DMG created.

## Refresh installed controller (Upgrade)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.193.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.193`.

## Web build + smoke

```bash
make ci-web-smoke
cd apps/dashboard-web && npm run build
```

Result: `PASS`.

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier-A screenshots (Playwright)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Saved screenshots to `manual_screenshots_web/20260121_073030`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260121_073030/trends_cooccurrence.png` (Co-occurring anomalies panel + markers)
- `manual_screenshots_web/20260121_073030/analytics.png` (Analytics Overview renders)

