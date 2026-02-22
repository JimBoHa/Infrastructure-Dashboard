# Sim Lab (Local Mocks)

This directory provides local mock services so CI/dev can exercise adoption and UI flows without
physical hardware or external dependencies beyond the native stack.

## Services (ports)

- **MQTT**: `1883` (Mosquitto)
- **Node simulator**: publishes heartbeat + telemetry to MQTT (no HTTP port)
- **BLE advertiser mock**: `9101` (HTTP JSON)
- **Mesh coordinator mock**: `9102` (HTTP JSON)
- **Forecast fixture**: `9103` (HTTP JSON)
- **Utility rate fixture**: `9104` (HTTP JSON)
- **Sim Lab control API**: `8100` (FastAPI control plane)

## Start the fixtures (manual)

The E2E harness starts these automatically. To run manually, launch the JSON fixture servers:

```bash
PORT=9101 FIXTURE_FILE=tools/sim_lab/fixtures/ble_advertiser.json python3 tools/sim_lab/http_json_server.py
PORT=9102 FIXTURE_FILE=tools/sim_lab/fixtures/mesh_coordinator.json python3 tools/sim_lab/http_json_server.py
PORT=9103 FIXTURE_FILE=tools/sim_lab/fixtures/forecast.json python3 tools/sim_lab/http_json_server.py
PORT=9104 FIXTURE_FILE=tools/sim_lab/fixtures/rates.json python3 tools/sim_lab/http_json_server.py
```

You also need a running MQTT broker (native Mosquitto via launchd or the installer).

## Core-server config (fixture providers)

```bash
export CORE_FORECAST_PROVIDER=http
export CORE_FORECAST_API_BASE_URL=http://127.0.0.1:9103
export CORE_FORECAST_API_PATH=/forecast.json

export CORE_ANALYTICS_RATES__ENABLED=true
export CORE_ANALYTICS_RATES__PROVIDER=http
export CORE_ANALYTICS_RATES__API_BASE_URL=http://127.0.0.1:9104
export CORE_ANALYTICS_RATES__API_PATH=/rates.json
```

## Sim Lab runner integration (optional)

```bash
python tools/sim_lab/run.py \\
  --forecast-fixture-url http://127.0.0.1:9103/forecast.json \\
  --rates-fixture-url http://127.0.0.1:9104/rates.json \\
  --control-port 8100
```

To enable predictive alarms during Sim Lab runs, pass `--predictive-enabled`.

## Sim Lab control API

- Base URL (default): `http://127.0.0.1:8100`
- Important env vars (set by `tools/sim_lab/run.py`):
  - `SIM_LAB_STORAGE_DIR`: directory for node config snapshots (`storage/sim_lab`)
  - `SIM_LAB_NODE_HOST`: host for node-agent HTTP endpoints (default `127.0.0.1`)
  - `SIM_LAB_RESTART_QUEUE`: fallback restart queue path
  - `SIM_LAB_ARM_TTL_SECONDS`: arm window (seconds, default `60`)
- Dashboard wiring: set `NEXT_PUBLIC_SIM_LAB_API_BASE` to the control API base URL.

## Quick smoke (Playwright)

This smoke script starts a demo-mode core server. For production-mode coverage, run
`FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke` followed by `make e2e-web-smoke`.

```bash
FARM_SIM_LAB_API_BASE=http://127.0.0.1:8000 \
FARM_SIM_LAB_BASE_URL=http://127.0.0.1:3005 \
node apps/dashboard-web/scripts/sim-lab-smoke.mjs
```
