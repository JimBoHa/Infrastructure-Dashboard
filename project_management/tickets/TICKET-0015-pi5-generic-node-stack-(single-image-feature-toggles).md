# TICKET-0015: Pi 5 Generic Node Stack (single image, feature toggles)

## Goal
All Raspberry Pi 5 nodes ship the **same installed software stack**. Per-node features/functions are enabled/disabled via config/capabilities (no separate “Renogy image”, “sensor node image”, etc.).

This is intended to keep deployments/updates simple, reduce drift, and make E2E + field debugging consistent.

## Requirements (high level)

### Base assumptions
- Pi nodes run **Linux + systemd** (Raspberry Pi OS Lite 64-bit).
- Controller is **macOS-only** (no Linux support required for the controller).
- Node-agent must keep hardware I/O off async hot paths (FastAPI event loop) to satisfy systemd watchdog expectations.

### OS/core services (always present)
- `systemd` + `journald`
- Networking (Ethernet/Wi‑Fi; DHCP; DNS)
- Bluetooth/BlueZ (required for Renogy BLE + iOS BLE provisioning)
- Time sync (`systemd-timesyncd`/`chrony`)

### systemd-managed services installed on every Pi 5 node
- `node-agent.service` (primary)
  - Telemetry + commands (MQTT publish/subscribe; buffering/backpressure handling)
  - Local API (status/health/config/provisioning)
  - Identity + config (MAC-based identity; adoption state; secrets/config store)
  - Operations (heartbeat; metrics/log export)
  - Storage (config + queues + local history/backups)
  - Local config UI (local display interface) **optional** / disabled by default
  - Sensor I/O + sampling
    - Analog (ADC: 0–10V + 4–20 mA via shunt resistors)
    - Pulse inputs (flow/rain): counter-based capture (no busy polling)
    - Bus sensors (I2C/SPI/1‑Wire/RS‑485 Modbus): same pattern—bus-owner worker + cached values
    - Scheduling: fixed-rate sampling loops; publish cadence per sensor config; COV sensors publish on change
    - Fault/quality: stale sample/open-loop/out-of-range/read errors → quality flags
  - Outputs (relay/contact control; safety gating/interlocks; capability-gated)

- `renogy-bt.service` (secondary)
  - BLE collection from Renogy charge controller via BT-2
  - Decode + expose a stable local feed consumed by node-agent
  - Installed everywhere but enabled only when the node’s config enables Renogy

### Performance expectations (design target)
A single Raspberry Pi 5 generic node must sustain:
- 20 analog samples/sec (10 channels × 2 Hz)
- DS18B20 1-wire reads that can block up to 750 ms per read (12-bit)
- Concurrent MQTT/HTTP/BLE workloads with event-loop stall <50 ms and bounded buffers

### Determinism / architecture constraints
- One bus owner per timing-sensitive bus (ADC SPI, 1-wire, Modbus/RS-485, output GPIO)
- No hardware I/O inside HTTP handlers or MQTT callbacks; handlers read cached values only
- Pulse capture must not rely on user-space polling; use kernel/DMA/external counter, and read back deltas on a cadence
- All queues must be bounded by policy sized to the uplink outage duration D (no unbounded growth)

## Non-goals (for this ticket)
- Hardware validation/field testing (tracked as separate “Validate … on hardware” tasks)

## Related tickets / references
- Pressure transducer + ADS1263 (implementation + validation): `TICKET-0005-reservoir-depth-pressure-transducer-integration.md`
- Client requested deployment features: `TICKET-0006-client-feature-requests-v2-overview.md`
