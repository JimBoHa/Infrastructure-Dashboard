# TICKET-0032: Pi 5 ADS1263 analog contract + fail-closed backend (no “ADS1115” stubs)

**Status:** Open

## Description
The project must support **hardware-backed analog sensors** (0–10V + 4–20 mA via shunt) on Raspberry Pi 5 nodes using the Waveshare ADS1263 HAT, configured end-to-end from the dashboard.

Historically, the node-agent could emit plausible-looking analog values even when the ADS1263 backend was not actually enabled/healthy (legacy “ADS1115” stub). For operations/safety and operator trust, this must change:

- Production installs must **fail closed** for analog (no silent simulation).
- Simulation remains available for CI/E2E/dev, but must require explicit opt-in and be obvious in status/UI.

## Scope
- Define and document the “analog contract” (single-ended vs differential mapping, quality/fault semantics, backend/health reporting).
- Replace the legacy “ads1115” naming/defaults with a generic analog driver concept (`type=analog`) and explicit backend reporting (`ads1263|disabled|simulated`).
- Pi 5 GPIO handling for ADS1263 uses `gpiozero` (+ `lgpio` where available) + `spidev` (no `RPi.GPIO` dependency required).
- Surface backend/health to the controller and dashboard so operators can diagnose without SSH.
- Harden dashboard UX to prevent creating analog hardware sensors on non-Pi nodes and to prevent accidental simulation enablement.

## Acceptance Criteria
- Dashboard “Add hardware sensor” flow:
  - Only available for Pi node-agent nodes.
  - Analog sensors cannot be enabled unless the node reports `analog_backend=ads1263` and `analog_health.ok=true` (unless the node is explicitly running in simulation mode with a banner).
- Node-agent:
  - Reports `analog_backend` and `analog_health` in its status payload.
  - Does **not** publish synthetic analog telemetry in production when ADS1263 is unhealthy; sensors go offline with clear reason.
- Reservoir depth (4–20 mA, 163Ω shunt, IN0/COM) works on Node 1 with plausible readings and correct unit conversion; Tier A evidence captured.

## References
- ADR: `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
- Contract: `docs/development/analog-sensors-contract.md`
- Runbook: `docs/runbooks/reservoir-depth-pressure-transducer.md`
