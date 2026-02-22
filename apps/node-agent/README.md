# Node Agent

Python-based FastAPI service that runs on each Pi/ESP node. It exposes a local UI, exports configuration, and publishes telemetry/heartbeat messages to the core MQTT broker.

## Features

- Local web UI on port 9000 with `/` for status, `/healthz`, and `/v1/config` for exporting effective configuration.
- Mesh + BLE provisioning:
  - `/v1/mesh` exposes summary/topology; `/v1/mesh/join` opens join window; `/v1/mesh/remove` bans a device.
  - `tools/mesh_pair.py` CLI opens join, lists topology, and removes devices.
  - BLE provisioning manager handles Wi‑Fi + adoption token exchange; fallback `/v1/provisioning/session` for HTTP/CLI.
- Telemetry publisher using `aiomqtt` that sends sensor data to `iot/{nodeId}/{sensorId}/telemetry` and node heartbeats to `iot/{nodeId}/status`.
- Hardware abstraction layer with ADS1263 HAT analog + GPIO pulse inputs (fail-closed when analog hardware is disabled/unavailable).
- Hardened `systemd` packaging with watchdog integration plus timers for log rotation and backup verification.

## Configuration

Environment variables (`NODE_` prefix) control the agent. Defaults aim at a lab Pi. Override via `/etc/node-agent.env` to persist across boots.

| Variable | Description | Default |
| --- | --- | --- |
| `NODE_NODE_ID` | Unique ID used in MQTT topics | `pi-node` |
| `NODE_NODE_NAME` | Human readable name | `Field Node` |
| `NODE_MQTT_URL` | Broker URL | `mqtt://127.0.0.1:1883` |
| `NODE_MQTT_USERNAME` / `NODE_MQTT_PASSWORD` | Optional broker credentials | empty |
| `NODE_HEARTBEAT_INTERVAL_SECONDS` | Heartbeat cadence | `5.0` |
| `NODE_TELEMETRY_INTERVAL_SECONDS` | Telemetry cadence | `30.0` |
| `NODE_SENSORS` | JSON array of sensor configs | `[ {"sensor_id": "demo-ads", "type": "analog", "channel": 0, "unit": "V"} ]` |
| `NODE_AGENT_BACKUP_ROOT` | Directory containing JSON config backups | `/opt/node-agent/storage/backups` |
| `NODE_AGENT_BACKUP_MAX_AGE_HOURS` | Backup age threshold checked by nightly verification | `36` |
| `NODE_AGENT_JOURNAL_VACUUM_DAYS` | Days of journald history to keep for node agent | `14` |

Example `.env`/`/etc/node-agent.env`:

```bash
NODE_NODE_ID=pi-greenhouse-1
NODE_NODE_NAME="Greenhouse North"
NODE_MQTT_URL=mqtt://core.local:1883
NODE_SENSORS='[
  {"sensor_id": "temp_1", "type": "analog", "channel": 0, "unit": "V"},
  {"sensor_id": "flow_a", "type": "pulse", "channel": 1, "unit": "pulses"}
]'
```

## Renogy BT-2 telemetry (Rover charge controller)

Renogy charge-controller nodes use a BT-2 BLE module to poll Modbus registers locally. Configure the BLE settings in `node_config.json` and add `renogy_bt2` sensor entries that map to Renogy metrics.
Runbook: `docs/runbooks/renogy-pi5-deployment.md`.

Minimal example (`node_config.json` excerpt):

```json
{
  "renogy_bt2": {
    "enabled": true,
    "address": "AA:BB:CC:DD:EE:FF",
    "unit_id": 1,
    "poll_interval_seconds": 10
  },
  "sensors": [
    { "sensor_id": "renogy-pv-power", "name": "PV Power", "type": "renogy_bt2", "metric": "pv_power_w", "unit": "W", "interval_seconds": 10 },
    { "sensor_id": "renogy-batt-soc", "name": "Battery SOC", "type": "renogy_bt2", "metric": "battery_soc_percent", "unit": "%", "interval_seconds": 10 },
    { "sensor_id": "renogy-load-power", "name": "Load Power", "type": "renogy_bt2", "metric": "load_power_w", "unit": "W", "interval_seconds": 10 }
  ]
}
```

Optional overrides: `service_uuid`, `write_uuid`, and `notify_uuid` can be set if the BT-2 uses non-default BLE characteristics. Use `NODE_MQTT_URL` in `/etc/node-agent.env` to point telemetry at the core broker.

## Observability (logs + traces)

The node agent emits JSON logs with `request_id`, `trace_id`, and `span_id` fields. Incoming HTTP requests accept
`X-Request-ID` and echo it in responses. MQTT telemetry payloads include generated `request_id` values for
correlation.

Relevant environment variables:

| Variable | Description | Default |
| --- | --- | --- |
| `NODE_LOG_LEVEL` | Log verbosity (`INFO`, `DEBUG`, ...) | `INFO` |
| `NODE_OTEL_ENABLED` | Enable OpenTelemetry tracing | `false` |
| `NODE_OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint (collector or SaaS) | `http://127.0.0.1:4317` |
| `NODE_OTEL_EXPORTER_OTLP_HEADERS` | OTLP auth headers (`key=value,...`) | empty |
| `NODE_OTEL_SAMPLE_RATIO` | Trace sampling ratio | `1.0` |

## Running locally

```bash
poetry install
poetry run uvicorn app.main:app --reload --port 9000
```

Environment file (optional):
```bash
cp .env.example .env
```

Provisioning helpers:
- Open a provisioning session: `curl -X POST http://127.0.0.1:9000/v1/provisioning/session -d '{"device_name":"Node A","wifi_ssid":"FarmWiFi"}'`
- Start BLE server automatically on startup; status visible on the `/` UI card.
- Mesh CLI: `python tools/mesh_pair.py scan` or `python tools/mesh_pair.py adopt --timeout 120`.

## Offline first-boot config

If BLE provisioning (NA-21) is unavailable or you are bulk-imaging nodes, the agent can apply a one-time first-boot JSON file.

- Generate the file with `tools/node-agent-firstboot-generator.html` (open locally in a browser) and download `node-agent-firstboot.json`.
- Place it next to the node config file (default directory: `/opt/node-agent/storage/`) as `node-agent-firstboot.json`.
  - Optional override: set `NODE_FIRSTBOOT_PATH`.
- If you include an `adoption_token`, it must be issued by the controller (obtain it from the dashboard adoption flow / deployment output, then paste it into the generator).
- On startup, the agent reads the file once, applies any fields present, then deletes it.

Supported keys:
```json
{
  "node": { "node_id": "pi-node-001", "node_name": "Field Node 001", "adoption_token": "deadbeefcafebabe" },
  "wifi": { "ssid": "FarmWiFi", "password": "secret" }
}
```

To pre-seed a full sensor list for bulk provisioning, start from `apps/node-agent/storage/node_config.example.json` and copy it to the node config path (default: `/opt/node-agent/storage/node_config.json`) before booting (or restore a controller backup onto the node). This file is **runtime state** and is intentionally **gitignored**. In production, the sensor list is typically managed from the dashboard after adoption and pushed to the node-agent.

## Systemd setup

1. Copy `systemd/node-agent.service` to `/etc/systemd/system/node-agent.service`.
2. Copy `systemd/node-agent-logrotate.service`/`.timer` and `systemd/node-agent-backup-verify.service`/`.timer` to `/etc/systemd/system/`.
3. Copy `systemd/logrotate/node-agent` to `/etc/logrotate.d/node-agent`.
4. Copy the scripts in `apps/node-agent/scripts` to `/usr/local/bin/` (rename without extension, e.g. `node-agent-logrotate`).
   - Include `node-agent-optional-services.py` as `/usr/local/bin/node-agent-optional-services` (enables/disables optional services like `renogy-bt.service` based on `node_config.json`).
5. Create the service user (used by `node-agent.service`):

```bash
sudo adduser --system --group --no-create-home farmnode
```

6. Ensure offline runtime deps are present (production kits ship these; no WAN required on the Pi):

```bash
# Python deps are vendored in the kit at:
#   /opt/node-agent/vendor
#
# pigpio (pigpiod.service) debs are shipped at:
#   /opt/node-agent/debs/*.deb
#
# Install pigpio from the shipped debs (optional unless using pulse counters):
if [ -d /opt/node-agent/debs ] && ls /opt/node-agent/debs/*.deb >/dev/null 2>&1; then
  sudo dpkg -i /opt/node-agent/debs/*.deb
fi
sudo chown -R farmnode:farmnode /opt/node-agent
```

7. Copy `systemd/node-agent.env.sample` to `/etc/node-agent.env` and adjust values (optional).
8. Enable the service and timers:

```bash
sudo systemctl daemon-reload
sudo systemctl enable node-agent.service \
  node-agent-logrotate.timer \
  node-agent-backup-verify.timer \
  node-agent-optional-services.path
sudo systemctl start node-agent.service
sudo systemctl start node-agent-optional-services.service
```

Logs stream via `journalctl -u node-agent -f`.

### Log rotation & backup verification timers

- `node-agent-logrotate.timer` runs nightly. It executes `/usr/local/bin/node-agent-logrotate` which runs `logrotate` against `/etc/logrotate.d/node-agent` and vacuums the `journalctl` history for `node-agent.service`.
- `node-agent-backup-verify.timer` runs nightly to execute `/usr/local/bin/node-agent-verify-backups`, ensuring the newest backup under `NODE_AGENT_BACKUP_ROOT` is newer than the configured threshold. Exits with a non-zero code if backups are stale or missing so failures surface in `systemctl status`.

## Image packaging workflow

Use the helper script to generate Raspberry Pi Imager assets:

```bash
# Pi 5 kit (overlay + Imager first-run script + checksums)
python tools/build_image.py pi-imager-profile --workspace build/pi-node/imager --force
```

The imager profile writes to `build/pi-node/imager/dist/`:

- `node-agent-firstrun.sh`: self-contained first-run script (overlay embedded); select this in Raspberry Pi Imager (**Run custom script on first boot**).
- `node-agent-overlay.tar.gz`: same overlay as a standalone file (for support/debugging; not required by Imager).
- `node-agent-imager.json`: saved profile metadata describing the operator steps.
- `VERSION` and `SHA256SUMS`: traceability + verification metadata.

## Testing / CI

E2E smoke (full stack):
```bash
make e2e-web-smoke
```

Node-agent unit tests:
```bash
cd apps/node-agent
PYTHONPATH=. poetry run pytest
```
