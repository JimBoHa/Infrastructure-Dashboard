# RUN-20260115-tier-a-dw122-dw123-detail-ux-0.1.9.126

## Goal
- Tier A validation (installed controller; no DB/settings reset): consolidate detail UX:
  - DW-122: remove Node detail drawer; Node detail is a dedicated page.
  - DW-123: remove Sensor detail page UX; sensor details live in the sensor drawer (keep `/sensors/detail?id=...` as redirect).

## Tests (local)
- `make ci-web-smoke` (pass)

## Refresh installed controller (no reset)
- Built bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.126.dmg`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.126.dmg"}'`
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` → `Upgraded to 0.1.9.126`
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
  - `curl -fsS http://127.0.0.1:8800/api/status` → `current_version: 0.1.9.126`

## Screenshots (captured + viewed)
- Command:
  - `cd apps/dashboard-web && npm run screenshots:web -- --no-core --no-web --api-base=http://127.0.0.1:8000 --base-url=http://127.0.0.1:8000 --auth-token-file=/tmp/fd_codex_auth_token.txt --focus-node-id=621eaead-22e9-4af5-8363-89e39a1eba3f --out-dir=manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600`
- Output folder (viewed):
  - `manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600/`
- Evidence screenshots (viewed):
  - Nodes list (no drawer UX): `manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600/nodes.png`
  - Node detail page (canonical): `manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600/nodes_621eaead-22e9-4af5-8363-89e39a1eba3f.png`
  - Sensor detail redirect → drawer: `manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600/sensors_da0b3467ba81595f3b6801e0.png`

