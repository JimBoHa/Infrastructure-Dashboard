# RUN-20260118 — Tier A — CS-86 Sensor series integrity (0.1.9.153)

- **Date:** 2026-01-18
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.153
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.153.dmg`

## Scope

- CS-86: Fail-closed guards and audit tooling to prevent a single `sensor_id` from mixing sources or semantic identity over time (no derived/forecast sensors receiving direct metric ingest; prevent post-history identity mutation).

## Build bundle

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.153 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.153.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.153.dmg`.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.153.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.153` (previous `0.1.9.152`).

Notes:
- `farmctl upgrade` emitted `xattr: [Errno 13] Permission denied` for the local DMG path, but the upgrade completed successfully.

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Integrity audit (installed controller DB)

Dry-run audit (no deletion):

```bash
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin ops_audit_sensor_series_integrity -- \
  --config /Users/Shared/FarmDashboard/setup/config.json
```

Result:
- No `forecast_points` sensors had `metrics` rows.
- No `derived` sensors had `metrics` rows.

