# RUN — Tier A — Sensors & Outputs “Add sensor” row — 0.1.9.109

## Goal
Validate the Sensors & Outputs UX change where hardware sensor configuration is opened via an inline “Add sensor” row per node.

## Environment
- Installed controller dashboard: `http://127.0.0.1:8000`
- Bundle upgraded: `FarmDashboardController-0.1.9.109.dmg`
- No DB/settings reset (Tier A requirement).

## Evidence (captured + viewed)
- Sensors page baseline (shows per-node Sensors tables + “Add sensor” row at the bottom of the table):
  - `manual_screenshots_web/20260112_142836/sensors.png`
- “Add sensor” click flow (drawer opens to the existing hardware sensor editor; new draft sensor is expanded):
  - `manual_screenshots_web/20260112_142836/sensors_add_sensor.png`

## How screenshots were captured
From `apps/dashboard-web`:
```bash
node scripts/web-screenshots.mjs \
  --no-web --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/tmp/tier_a_api_token.txt \
  --out-dir=manual_screenshots_web/20260112_142836
```

