# RUN-20260114 — Tier A WS-2902 short ingest token/path (installed controller `0.1.9.118`)

**Goal:** Refresh the already-installed controller to ship the WS-2902 ingest token length + short `/api/ws/<token>` path fix (Tier A). **No DB/settings reset.**

## Upgrade / refresh (installed controller)

- Preconditions:
  - `curl -fsS http://127.0.0.1:8800/healthz` → `{"status":"ok"}`
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.118.dmg`
- Pointed setup-daemon at the new DMG:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.118.dmg"}'`
- Upgraded:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
  - Note: setup-daemon log included an `xattr` "Permission denied" warning while attempting quarantine cleanup on the user-owned DMG; the upgrade still succeeded (expected for locally-built, non-quarantined artifacts).
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
  - `curl -fsS http://127.0.0.1:8800/api/status` → `current_version: 0.1.9.118`

## WS-2902 smoke

- Short ingest route responds (unknown token expected):
  - `curl -i http://127.0.0.1:8000/api/ws/ffffffffffffffffffffffff` → `404 Unknown token`

## Follow-ups

- Hardware validation: CS-76 (confirm the WS-2902-class station custom server UI accepts `/api/ws/<token>` without truncation and uploads successfully).
