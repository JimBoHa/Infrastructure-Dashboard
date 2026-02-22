# Renogy Pi 5 Simulator Validation (renogy-bt ingest)

This runbook is for simulator-only validation. For real hardware deployment, use
`docs/runbooks/renogy-pi5-deployment.md`.

## Prerequisites
- Raspberry Pi 5 simulator runbook completed: `docs/runbooks/pi5-simulator.md`.
- Local MQTT broker (default: `mqtt://127.0.0.1:1883`).
- `poetry` installed (node-agent runs via `poetry run`).

## 1) Generate a Renogy deployment bundle
```bash
python tools/renogy_node_deploy.py bundle \
  --node-name "Renogy Sim" \
  --node-id "renogy-sim-01" \
  --bt2-address "AA:BB:CC:DD:EE:FF" \
  --collector renogy-bt \
  --ingest-token "renogy-sim-token" \
  --mqtt-url "mqtt://127.0.0.1:1883" \
  --output build/renogy-sim
```

## 2) Start the simulator using the bundle config
Run the node-agent with the bundle config and disable simulation so external ingest is used:

```bash
python tools/pi5_simulator.py \
  --config-path build/renogy-sim/node_config.json \
  --mqtt-url "mqtt://127.0.0.1:1883" \
  --port 9400 \
  --no-simulation
```

If node-agent dependencies are only available through Poetry, run:
```bash
cd apps/node-agent && poetry run python ../../tools/pi5_simulator.py \
  --config-path ../../build/renogy-sim/node_config.json \
  --mqtt-url "mqtt://127.0.0.1:1883" \
  --port 9400 \
  --no-simulation
```

## 3) POST a sample Renogy payload
```bash
curl -X POST http://127.0.0.1:9400/v1/renogy-bt \
  -H "Authorization: Bearer renogy-sim-token" \
  -H "Content-Type: application/json" \
  -d '{
    "pv_power": 120,
    "pv_voltage": 18.5,
    "pv_current": 6.4,
    "battery_percentage": 72,
    "battery_voltage": 12.6,
    "battery_current": 2.1,
    "battery_temperature": 25.2,
    "controller_temperature": 28.3,
    "load_power": 45.0,
    "load_voltage": 12.2,
    "load_current": 3.2,
    "runtime_hours": 8.4
  }'
```

## 4) Verify telemetry publishes to MQTT
```bash
mosquitto_sub -h 127.0.0.1 -p 1883 -t "iot/renogy-sim-01/+/telemetry" -C 5
```

If `mosquitto_sub` is not installed locally, use the bundled binary from the installer (`/usr/local/farm-dashboard/native/mosquitto/bin/mosquitto_sub`) or point `FARM_MOSQUITTO_BIN` to it.
