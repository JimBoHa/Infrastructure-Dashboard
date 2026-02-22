# RUN-20260111 Tier A — Phase 6 Fleet Ops Telemetry (0.1.9.77)

**Goal:** Validate fleet-ops node health telemetry end-to-end on the installed controller (Tier A). No DB/settings reset.

## Bundle

- Installed controller upgraded to `0.1.9.77` via setup-daemon (`http://127.0.0.1:8800`).
- Health OK: `curl -fsS http://127.0.0.1:8000/healthz`.

## Evidence (Tier A)

### Node snapshot fields on `/api/nodes`

- Pi5 Node 2 shows non-null snapshot fields:
  - `memory_percent`, `memory_used_bytes`
  - `network_latency_ms`, `network_jitter_ms`, `uptime_percent_24h`

Example check:
```bash
curl -fsS http://127.0.0.1:8000/api/nodes | jq '.[] | select(.name=="Pi5 Node 2") | {status, memory_percent, memory_used_bytes, network_latency_ms, network_jitter_ms, uptime_percent_24h}'
```

### Node health trend history via `/api/metrics/query`

- Deterministic node-health sensor IDs query returns points (example for `network_latency_ms`):
```bash
curl -fsS 'http://127.0.0.1:8000/api/metrics/query?sensor_ids[]=84cb1f6e9fdb2a2933c58d50&start=2026-01-11T03:32:32.022510%2B00:00&end=2026-01-11T03:52:32.022510%2B00:00&interval_seconds=60'
```

### UI verification

- Node detail → **Health** section shows snapshot and “Show trends” renders the chart.
- Screenshot: `manual_screenshots_web/tier-a-nodehealth/nodes_node2_health_trends_v2.png`.

## Tests

- Node-agent: `cd apps/node-agent && .venv/bin/python -m pytest -q` (pass)
- Telemetry-sidecar: `cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml` (pass)
- Core server: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- Dashboard web smoke: `cd apps/dashboard-web && CI=1 npm run test:smoke` (pass)
- OpenAPI coverage: `make rcs-openapi-coverage` (pass)

