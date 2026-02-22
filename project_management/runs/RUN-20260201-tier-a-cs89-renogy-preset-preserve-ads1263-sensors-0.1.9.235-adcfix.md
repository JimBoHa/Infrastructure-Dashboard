# RUN-20260201 — Tier A + Hardware — CS-89 Renogy preset must not wipe ADS1263 sensors (0.1.9.235-adcfix)

- **Date:** 2026-02-01
- **Tier:** A (installed controller refresh; **no DB/settings reset**) + hardware validation (Pi 5 Node 1)
- **Controller version:** `0.1.9.235-adcfix`
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.235-adcfix.dmg`

## Problem statement

Pi5 Node 1 has ADS1263/ADC HAT sensors configured (`Reservoir Depth`, `ADC0 Voltage`, `ADC1 Voltage`, `Node1 DC Current to Loads`) but they stopped updating in the node dashboard shortly after Renogy kWh sensors were added (`PV Energy (today/total)`).

Root cause: `POST /api/nodes/{node_id}/presets/renogy-bt2` pushed the controller’s `nodes.config.sensors` snapshot back to node-agent via `/v1/config`. That snapshot contains **only Renogy sensors** (hardware sensors live in `desired_sensors`), so the node-agent’s sensor list was overwritten and ADS1263 sampling stopped (because analog inputs are derived from configured sensors).

## Fix (core-server)

Updated Renogy preset apply logic to:
- fetch the live node-agent config (`GET http://<node>:9000/v1/config`)
- merge Renogy sensors into the existing sensor list (preserving ADS1263/analog sensors)
- push the merged config back to the node-agent (`PUT /v1/config`)

## Restore Node 1 ADS1263 sensors (hardware)

Node id:
- Pi5 Node 1: `0a55b329-104f-46f0-b50b-dea9a5cca1b3` (`10.255.8.170`)

Push desired hardware sensors config to node-agent (merges with existing sensors; does not wipe Renogy):

```bash
curl -fsS -H "Authorization: Bearer <token>" \
  "http://127.0.0.1:8000/api/nodes/0a55b329-104f-46f0-b50b-dea9a5cca1b3/sensors/config"

curl -fsS -H "Authorization: Bearer <token>" -H "Content-Type: application/json" \
  -X PUT "http://127.0.0.1:8000/api/nodes/0a55b329-104f-46f0-b50b-dea9a5cca1b3/sensors/config" \
  --data-binary @/tmp/node1_sensors_config_update_req.json
```

Verification on the node (local dashboard state includes ADS1263 sensors and values):

```bash
ssh node1@10.255.8.170 'curl -fsS http://127.0.0.1:9000/v1/display/state | jq ".sensors | map({name, sensor_id, value})"'
```

Controller ingest verification (latest timestamps advance):
- `ea5745e00cb0227e046f6b88` Reservoir Depth
- `11f4bdb5739774c9c7ba668b` ADC0 Voltage
- `9ab25497bae062b8c35c4001` ADC1 Voltage
- `37c26eb6bb138091994f28dd` Node1 DC Current to Loads

## Refresh installed controller (Tier A)

Build + refresh:

```bash
python3 tools/rebuild_refresh_installed_controller.py \
  --output-dir /Users/Shared/FarmDashboardBuildsDirty \
  --allow-dirty \
  --version 0.1.9.235-adcfix
```

Bundle outputs:
- DMG: `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.235-adcfix.dmg`
- Log: `/Users/Shared/FarmDashboardBuildsDirty/logs/bundle-0.1.9.235-adcfix.log`

Installed health smoke:

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Regression test (no wipe on renogy preset apply)

Re-apply Renogy preset (should be idempotent, and must not delete ADS1263 sensors from node-agent):

```bash
curl -fsS -H "Authorization: Bearer <token>" -H "Content-Type: application/json" \
  -X POST "http://127.0.0.1:8000/api/nodes/0a55b329-104f-46f0-b50b-dea9a5cca1b3/presets/renogy-bt2" \
  --data-binary '{"bt2_address":"10:CA:BF:AA:83:07","poll_interval_seconds":30,"mode":"ble"}'

ssh node1@10.255.8.170 'curl -fsS http://127.0.0.1:9000/v1/config | jq ".sensors | map(.name)"'
```

Result:
- Preset apply response: `status=already_configured`, `warning=null`
- Node-agent config includes both Renogy sensors and ADS1263 sensors (18 total on Node1 at time of validation).

