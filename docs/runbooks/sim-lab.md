# Runbook: Sim Lab (Start From Closed)

This runbook starts from a fully stopped state (no app terminals open). It boots the Sim Lab stack, opens the UI, and shows how to shut everything down.

## Prereqs
- Native services running (Postgres/Mosquitto/Redis via launchd or installer)
- Repo dependencies installed (`make bootstrap`)

## Recommended Path (one command)

1) Start the full stack (infra + core + web + sidecar + Sim Lab) from the repo root:

```bash
make demo-live
```

Leave this terminal running.

2) Open the dashboards:
- Main dashboard: http://127.0.0.1:3001
- Sim Lab console: http://127.0.0.1:3001/sim-lab

3) In the Sim Lab console:
- Click "Arm Controls" (enables actions for ~60 seconds).
- Choose scenario/seed/time multiplier if desired.
- Click "Start".

4) In the main dashboard:
- Go to Nodes, click +, run a scan, adopt a node.
- Confirm telemetry is live (Nodes, Sensors, Trends).

## Alternative Path (mocks only + manual core/web)

Use this if you want to run core/web manually or without the full `demo-live` stack.

1) Start the Sim Lab mocks:

```bash
PORT=9101 FIXTURE_FILE=tools/sim_lab/fixtures/ble_advertiser.json python3 tools/sim_lab/http_json_server.py
PORT=9102 FIXTURE_FILE=tools/sim_lab/fixtures/mesh_coordinator.json python3 tools/sim_lab/http_json_server.py
PORT=9103 FIXTURE_FILE=tools/sim_lab/fixtures/forecast.json python3 tools/sim_lab/http_json_server.py
PORT=9104 FIXTURE_FILE=tools/sim_lab/fixtures/rates.json python3 tools/sim_lab/http_json_server.py
```

2) Start core server pointing at fixtures:

```bash
CORE_FORECAST_API_BASE_URL=http://127.0.0.1:9103 \
CORE_FORECAST_API_PATH=/forecast.json \
CORE_ANALYTICS_RATES__API_BASE_URL=http://127.0.0.1:9104 \
CORE_ANALYTICS_RATES__API_PATH=/rates.json \
make core
```

3) Start the dashboard (new terminal):

```bash
FARM_CORE_API_BASE=http://127.0.0.1:8000 make web
```

4) Open the dashboards:
- Main dashboard: http://127.0.0.1:3001
- Sim Lab console: http://127.0.0.1:3001/sim-lab

## Shutdown

1) Stop any running `make demo-live` / `make core` / `make web` terminals with `Ctrl+C`.

2) Stop Sim Lab mocks (close the fixture server terminals or send `Ctrl+C` to each).

3) Stop native services if needed (launchd/farmctl).

## Troubleshooting
- If the dashboard says "Failed to fetch", ensure `FARM_CORE_API_BASE` is set to `http://127.0.0.1:8000` and restart core + web.
- If native services are not running, start Postgres/MQTT/Redis via launchd/farmctl and retry.
- If MQTT is not running and no broker is on port 1883, set `FARM_MOSQUITTO_BIN` to the native Mosquitto binary (ex: `/usr/local/farm-dashboard/native/mosquitto/bin/mosquitto`) and rerun.
- If ports are in use (8000/3001/8100/1883/5432), stop the conflicting process and retry.
