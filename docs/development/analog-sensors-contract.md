# Analog sensors contract (controller ↔ node-agent)

This document defines the contract for **hardware-backed analog sensors** configured from the dashboard and executed on Pi node-agents.

**Hard rule:** Production is fail‑closed; simulation is test/dev only and cannot be enabled via the dashboard.

## Terminology
- **Preset (controller classification):** the sensor “kind” used by the controller/dashboard for grouping and analytics (e.g., `voltage`, `pressure`, `water_level`). Stored in the controller DB (`sensors.type`).
- **Driver (node execution):** how the node reads the sensor (e.g., `analog`, `pulse`, `renogy_bt2`). Stored in the controller DB as `sensors.config.driver_type` and pushed into `node_config.json` as `sensor.type`.
- **Analog backend (node hardware):** which implementation reads analog inputs on a Pi node (`ads1263`, `disabled`, `simulated`).

## Data model
### Controller → node-agent config
The controller pushes `node_config.json` sensor entries where:
- `sensor.type` is the **driver**. For analog hardware sensors, this must be `analog`.
- `sensor.channel` is the ADC input index.
- `sensor.negative_channel` (optional) selects differential mode.

Single-ended:
- `channel = AINx`, `negative_channel = null` (uses `AINCOM` as the negative reference).

Differential:
- `channel = AIN+`, `negative_channel = AIN−`.

### 4–20 mA current-loop conversion (node-agent)
If any of these are set:
- `current_loop_shunt_ohms`
- `current_loop_range_m`

…the node-agent interprets the ADC reading as **shunt voltage**, computes loop current, and maps it to depth.

Defaults (industrial convention):
- `current_loop_zero_ma = 4.0`
- `current_loop_span_ma = 16.0`
- `fault_low_ma = 3.5`
- `fault_high_ma = 21.0`

Faults:
- `quality=1` → low current fault (open circuit / sensor fault)
- `quality=2` → high current fault (overrange / wiring fault)
- `quality=3` → configuration error (treated as fault)
- `quality=4` → backend unavailable / read failed (treated as offline / no publish)

**Fail-closed behavior:** for current-loop sensors, any non-zero quality is treated as a fault and telemetry is suppressed (no publish). This prevents “plausible” clamped depths (0 or max) from being displayed when the loop is open/shorted or misconfigured.

## Fail-closed behavior (production)
If the analog backend is not enabled/healthy:
- Analog reads return **unavailable**, and the node-agent publishes **no telemetry** for those sensors.
- The dashboard must show backend/health so operators understand why the sensor is offline.

Simulation is supported but requires explicit opt-in and must be clearly labeled.

## Node status contract (“what backend is active?”)
The node-agent publishes in its status payload:
- `analog_backend`: `ads1263 | disabled | simulated`
- `analog_health`: `{ ok: bool, chip_id?: string, last_error?: string, last_ok_at?: timestamp }`

The controller stores and exposes these fields without SSH so the dashboard can:
- warn when SPI is disabled / backend is unhealthy,
- block adding analog sensors unless the backend is healthy (or explicitly in simulation mode).

## See also
- ADR: `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
- Runbook: `docs/runbooks/reservoir-depth-pressure-transducer.md`
