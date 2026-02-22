# RUN-20260118 — Tier A — CS-87 + DW-149 Derived Sensor Function Library (0.1.9.162)

- **Date:** 2026-01-18
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.162
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.162.dmg`
- **Ticket:** `project_management/archive/archive/tickets/TICKET-0036-derived-sensors-expand-function-library-cs-87-dw-149.md`

## Scope

- CS-87: Expand derived-sensor expression function library (math + trig + conditionals).
- DW-149: Derived sensor builder lists the full library + insert helpers; makes trig units explicit (radians).

## Tests

```bash
cargo test --manifest-path apps/core-server-rs/Cargo.toml
make ci-web-smoke
```

## Build bundle

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds

cargo run --manifest-path apps/farmctl/Cargo.toml --release -- \
  bundle \
  --version 0.1.9.162 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.162.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.162.dmg`.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.162.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.162` (previous `0.1.9.161`).

## Health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier-A UI evidence (screenshots)

Captured + opened (viewed):

- `manual_screenshots_web/tier_a_0.1.9.162_cs87_dw149_20260118_074353/derived_sensor_function_library.png`

Notes:
- The screenshot captures Sensors & Outputs → Add sensor drawer → Derived tab, with “More functions” expanded, showing the full function library.
