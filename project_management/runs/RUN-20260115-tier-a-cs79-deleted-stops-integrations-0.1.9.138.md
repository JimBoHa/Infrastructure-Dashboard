# RUN-20260115 — Tier A — CS-79 Deleted nodes stop controller integrations — 0.1.9.138

**Date:** 2026-01-15

## Goal
Validate that deleting nodes/sensors preserves history but stops controller-owned pollers/integrations from continuing to spend resources on deleted entities.

## Build + Upgrade
1. Confirm clean worktree (farmctl bundle hard gate):
   - `git status --porcelain=v1 -b`

2. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.138 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.138.dmg --native-deps build/native-deps`

3. Upgrade installed controller via setup daemon:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.138.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`

4. Health:
   - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
   - `curl -fsS http://127.0.0.1:8800/api/status` → `current_version: 0.1.9.138`

## Validation
### A) Forecast polling does not include deleted nodes
- Create a throwaway node, set `pv_forecast.enabled=true`, and create a Map placement (these are the two controller-owned selection sources).
- Trigger `/api/forecast/poll` (Open‑Meteo current + Forecast.Solar).
- Delete the node.
- Confirm the node’s Map placement is removed (no longer returned by `/api/map/features`).
- Trigger `/api/forecast/poll` again and confirm it succeeds.

### B) WS‑2902 ingest is blocked after node deletion
- Create a WS‑2902 integration (`POST /api/weather-stations/ws-2902`) and capture its `node_id` and `token`.
- Delete the node.
- Attempt `GET /api/ws/<token>?tempf=...` and confirm it returns `403 Integration disabled` (no metrics or node status writes).

## Evidence (Tier A)
- Screenshot (captured + viewed): `manual_screenshots_web/tier_a_0.1.9.138_cs79_2026-01-15_195801215Z/01_nodes.png`

