# RUN-20260113 — Tier A Hardware sensors are Pi-only (installed controller `0.1.9.113`)

## Goal
- Tier A validation (installed controller; **no DB/settings reset**): hardware sensors can only be configured on Raspberry Pi node-agent nodes, and the dashboard does not expose “Add sensor” for Core/Emporia/external nodes.

## Installed controller version
- `/usr/local/farm-dashboard/state.json` → `current_version: "0.1.9.113"`

## Validation (UI) — screenshots captured + viewed
- Token created (not committed): `/tmp/tier_a_api_token_20260113.txt`
- Command:
  - `cd apps/dashboard-web && node scripts/web-screenshots.mjs --no-core --no-web --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/tmp/tier_a_api_token_20260113.txt --out-dir=manual_screenshots_web/20260113_tier_a_hw_sensors_0.1.9.113`
- Output folder (captured + viewed):
  - `manual_screenshots_web/20260113_tier_a_hw_sensors_0.1.9.113/`
- Evidence screenshots (viewed):
  - `manual_screenshots_web/20260113_tier_a_hw_sensors_0.1.9.113/sensors_add_sensor.png` (Pi node shows “Add sensor” row; opens “Add sensor” drawer)
  - `manual_screenshots_web/20260113_tier_a_hw_sensors_0.1.9.113/sensors_core.png` (Core node: no “Add sensor” row)

## Validation (API) — non-node-agent nodes rejected
- Example (Emporia node): `GET /api/nodes/c640d20e-e1b1-4bc8-bf79-5582482684e0/sensors/config` returns `400 Bad Request`:
  - `Hardware sensors can only be configured on Raspberry Pi nodes.`

