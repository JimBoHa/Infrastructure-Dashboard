# Raspberry Pi 5 Simulator Runbook

This runbook spins up a local Raspberry Pi 5 node-agent simulator that publishes deterministic telemetry without physical hardware.

## Prerequisites
- Local MQTT broker reachable from the node-agent (default: `mqtt://127.0.0.1:1883`).
- `poetry` installed (the simulator launches `apps/node-agent` with `poetry run`).
- Optional full-stack registration: core server running + a bearer token with `config.write`.
- If the node-agent dependencies are not installed globally, run the simulator via `cd apps/node-agent && poetry run python ../../tools/pi5_simulator.py ...`.

## 1) Start the simulator (local-only)
```bash
python tools/pi5_simulator.py \
  --node-id "pi5-sim-01" \
  --node-name "Pi 5 Simulator" \
  --mqtt-url "mqtt://127.0.0.1:1883" \
  --port 9300
```

Defaults:
- Config is written to `storage/pi5_sim/<node-id>/node_config.json`.
- Renogy BT-2 metrics are simulated by default (disable with `--no-renogy`).
- Adoption token defaults to `pi5-sim-token`.

## 2) Start the simulator (full stack registration)
This mode pre-registers the node, sensors, and outputs in the core server and uses the core node UUID for MQTT topics (required for telemetry-sidecar status updates + output command topics).

```bash
CORE_URL="http://127.0.0.1:8000"
CORE_TOKEN="replace-with-config-write-token"

python tools/pi5_simulator.py \
  --node-id "pi5-sim-01" \
  --node-name "Pi 5 Simulator" \
  --mqtt-url "mqtt://127.0.0.1:1883" \
  --port 9300 \
  --register-core \
  --core-url "$CORE_URL" \
  --core-token "$CORE_TOKEN"
```

Notes:
- The simulator logs the core node UUID and the adoption token it issued (unless you pass `--adoption-token`).
- The config bundle is written to `storage/pi5_sim/<core-node-uuid>/node_config.json`.
- Pass `--mac-eth`/`--mac-wifi` for stable MAC bindings across runs.
- When registering against a seeded Sim Lab DB, the simulator de-dupes sensor/output IDs that already exist on other nodes.
- Renogy simulator sensors use the canonical IDs from `apps/node-agent/README.md` (ex: `renogy-pv-power`, `renogy-batt-soc`, `renogy-load-power`).

## 3) Renogy BT-2 simulator validation
See `docs/runbooks/renogy-pi5-simulator.md` for the Renogy bundle + ingest workflow.

## 4) Verify the node is up
```bash
curl http://127.0.0.1:9300/v1/status
```

You should see simulated sensors (temperature, moisture, pressure, flow, power, solar, etc) and outputs.

## 5) Adopt in the dashboard (optional)
1. Open the dashboard and run **Scan** in the Nodes tab.
2. Adopt using the default token (`pi5-sim-token`) or the one passed via `--adoption-token`.

## 6) Generate config without running
If you only want the config bundle for later use:
```bash
python tools/pi5_simulator.py --write-config-only
```

## Troubleshooting
- **No telemetry**: confirm the MQTT broker URL is reachable and the broker is running.
- **Renogy metrics not needed**: restart with `--no-renogy` to remove the Renogy sensor set.
- **Port already in use**: pass a different `--port` value.
