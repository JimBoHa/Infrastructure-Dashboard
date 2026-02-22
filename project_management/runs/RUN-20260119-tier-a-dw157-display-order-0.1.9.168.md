# RUN-20260119 — Tier A DW-157 Display order (installed controller `0.1.9.168`)

## Goal
- Tier A validation (installed controller; **no DB/settings reset**): Verify node + sensor ordering is persisted in the controller DB and rendered consistently across dashboard surfaces after refresh.

## Installed controller version
- `/usr/local/farm-dashboard/state.json` → `current_version: "0.1.9.168"`
- Setup bundle path (setup-daemon config): `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.168.dmg`

## Upgrade / refresh (Tier A)
- Built controller bundle DMG:
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.168 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.168.dmg --native-deps /usr/local/farm-dashboard/native`
- Pointed setup daemon at stable bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.168.dmg"}'`
- Upgraded via setup daemon (no admin; launchd KeepAlive restarts services):
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/healthz`

## Evidence: persisted order (API + UI)
- Verified node order persistence:
  - `PUT /api/nodes/order` (swap first two ids) then `GET /api/nodes` shows the swapped order.
- Verified per-node sensor order persistence:
  - `PUT /api/nodes/{node_id}/sensors/order` (swap first two sensor ids for the selected node) then `GET /api/sensors` reflects the new per-node order.
- Verified UI reflects persisted order (modal reflects the new ordering):
  - Screenshot captured + viewed:
    - `manual_screenshots_web/tier_a_0.1.9.168_dw157_order_ui_20260119T070322Z/01_reorder_modal_after_api_reorder.png`
  - Run metadata (ids before/after + selected node) recorded at:
    - `manual_screenshots_web/tier_a_0.1.9.168_dw157_order_ui_20260119T070322Z/run.json`

## Notes
- `make e2e-web-smoke` is **not** runnable on this host because the installed controller intentionally keeps services running; the E2E harness requires a clean-state preflight and aborts when it finds running controller processes (expected for Tier A).
- Tier B clean-host validation should be tracked under the existing Trends/Web cluster ticket(s) when scheduled (e.g. `DW-114` / `DW-98`), but DW-157 itself is Tier-A validated here.

