# Node Agent (Generic Pi 5 Stack)

## North-star (for Pi nodes)
Every Raspberry Pi 5 node runs the **same installed software stack**. Features are enabled/disabled per-node via config/capabilities (no separate “Renogy image”, “sensor node image”, etc.).

Canonical requirements live in:
- `project_management/tickets/TICKET-0015-pi5-generic-node-stack-(single-image-feature-toggles).md`

## Services (systemd)
All Pi 5 nodes install the same baseline systemd units:
- `node-agent.service` (primary)
- `renogy-bt.service` (secondary; installed everywhere, enabled only when configured)
- `node-agent-optional-services.path` / `node-agent-optional-services.service` (watches `node_config.json` changes and enables/disables optional services like `renogy-bt.service`)

Additional “support” units are also shipped with the node-agent kit:
- `node-agent-logrotate.timer` / `node-agent-logrotate.service`
- `node-agent-backup-verify.timer` / `node-agent-backup-verify.service`

## Non-blocking architecture constraints
The node-agent must remain responsive under load and satisfy systemd watchdog expectations.

Hard rules:
- No hardware I/O in HTTP handlers or MQTT callbacks.
- Each timing-sensitive bus has a single owner (ADC SPI, 1-wire, Modbus/RS-485, outputs GPIO).
- The control plane reads cached values only.
- Buffers are bounded by policy (no unbounded growth during uplink loss).

Design target:
- Event-loop stall <50 ms under concurrent ADC/1-wire/MQTT/BLE activity.

## Sensor I/O patterns
### Analog (0–10V + 4–20 mA)
- ADC sampling runs in a bus-owner worker (thread/process).
- **Hard rule:** Production is fail‑closed; simulation is test/dev only and cannot be enabled via the dashboard.
- Contract: `docs/development/analog-sensors-contract.md`
- For 10 channels × 2 Hz, ADS1263 data rate must be ≥50 SPS (recommend 100 SPS).
- 4–20 mA sensors are measured via per-channel shunt resistors and converted to engineering units; faults (<4 mA / >20 mA) surface as `quality` flags.

Runbook (pressure transducer):
- `docs/runbooks/reservoir-depth-pressure-transducer.md`

### Pulse inputs (flow/rain)
- Capture must not rely on busy polling; use a counter-based approach (kernel/DMA/external counter).
- Telemetry reporting uses deltas at publish cadence (not raw cumulative counts).

### Bus sensors (I2C / 1-wire / RS-485 Modbus)
- Reads can block (e.g., DS18B20 conversion time); they must be isolated in bus-owner workers.
- Publish/UI reads cached values only; never block on bus reads.

## Configuration / deployment entrypoints
- **Production Pi 5 deployment (only supported path):** Dashboard → Deployment → Remote Pi 5 Deployment (SSH). Runbook: `docs/runbooks/pi5-deployment-tool.md`.
- Deprecated (dev-only) imaging helpers: `python tools/build_image.py pi-imager-profile` and `tools/flash_node_image.sh`.
- Pi simulator runbook: `docs/runbooks/pi5-simulator.md`

## Optional services (feature toggles without SSH)
Optional secondary services are shipped in the generic node stack but must not run unless the node config enables them.

Mechanism:
- `node-agent-optional-services.path` watches `/opt/node-agent/storage/node_config.json` (atomic writes via the node-agent API/restore flows) and triggers `node-agent-optional-services.service`.
- `node-agent-optional-services.service` runs `/usr/local/bin/node-agent-optional-services`, which enables/disables `renogy-bt.service` based on `renogy_bt2` config state.

This keeps the “one-click” dashboard UX honest: applying a Renogy preset can update `node_config.json` and the node will start/stop `renogy-bt.service` automatically without requiring SSH access after install.

## Validation
Fast local validation (no hardware):
- `make ci-node`
- `cd apps/node-agent && poetry run pytest`

Full-stack E2E validation (controller path):
- `make e2e-installer-stack-smoke`
- `make e2e-web-smoke`

Hardware validation is tracked separately as “Validate … on hardware” tasks (see `project_management/TASKS.md`).
