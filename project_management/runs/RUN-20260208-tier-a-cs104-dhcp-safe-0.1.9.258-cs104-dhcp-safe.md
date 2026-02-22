# RUN-20260208 Tier A — CS-104 (0.1.9.258-cs104-dhcp-safe)

## Context

- **Date:** 2026-02-08
- **Task:** **CS-104** — DHCP-safe node-agent addressing (mDNS hostname + MAC-matched heartbeat IP refresh)
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and validate the controller remains healthy while node addressing changes no longer rely on stale `nodes.ip_last`.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.256-related-preview-context` (from `http://127.0.0.1:8800/api/status`)
- Rollback target (previous version) DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg`
- Worktree gate: clean (Tier A hard gate)

## Build (controller bundle DMG)

- **Version:** `0.1.9.258-cs104-dhcp-safe`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.258-cs104-dhcp-safe.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.258-cs104-dhcp-safe.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.258-cs104-dhcp-safe \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.258-cs104-dhcp-safe.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.258-cs104-dhcp-safe.log
```

Result:

- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.258-cs104-dhcp-safe.dmg`
- `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.258-cs104-dhcp-safe.dmg`

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.258-cs104-dhcp-safe.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Upgrade result:

- `Upgraded to 0.1.9.258-cs104-dhcp-safe`

Post-upgrade installed version:

- `current_version`: `0.1.9.258-cs104-dhcp-safe`
- `previous_version`: `0.1.9.256-related-preview-context`

(from `curl -fsS http://127.0.0.1:8800/api/status`)

## Validation

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Nodes list endpoint: `GET http://127.0.0.1:8000/api/nodes` (Bearer token from `/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`) → **200**
- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Notes

- A prior Tier‑A upgrade attempt to `0.1.9.257-cs104-dhcp-safe` was rolled back due to `GET /api/nodes` returning `500 Database error` after upgrade. Root cause was a Postgres float type decode mismatch (`nodes.ping_*` and `nodes.mqtt_broker_rtt_*` are `double precision` but Rust `NodeRow` expected `Option<f32>`). This was fixed by casting those columns to `real` in the `/api/nodes` and adoption response queries.
- DHCP churn validation on real node hardware remains tracked as **CS-105**.
