# 0005. Pi 5 GPIO backend + fail-closed analog (ADS1263)

* **Status:** Accepted
* **Date:** 2026-01-13

## Context
We use Raspberry Pi 5 nodes for on-device I/O and publish telemetry upstream. For analog sensors (0–10V, 4–20 mA via shunt), the production path is the Waveshare ADS1263 HAT over SPI0 plus a few GPIO lines (CS/DRDY/RST).

Two reliability problems emerged:

1) **GPIO library mismatch on Pi 5**
   - `RPi.GPIO` is not a good long-term choice on Pi 5 / Bookworm-era stacks (RP1, character-device GPIO stacks).
   - We need a GPIO approach that works reliably on Pi 5 and is compatible with systemd services.

2) **“Plausible fake values” are worse than “no data”**
   - The node-agent historically defaulted to a stub “ADS1115” analog driver that emitted synthetic voltages.
   - If the ADS1263 HAT is misconfigured (SPI disabled, wiring wrong, missing deps), the system could still emit plausible-looking analog telemetry, masking faults and undermining operator trust.

## Decision
Production is fail‑closed; simulation is test/dev only and cannot be enabled via the dashboard.

1) **GPIO on Pi 5**
   - Use `gpiozero` devices for ADS1263 pin control (CS/DRDY/RST).
   - Prefer the `lgpio` backend when available (character-device GPIO).
   - Use `spidev` for SPI transfers.

2) **Fail-closed analog in production**
   - In production mode, if the ADS1263 backend is not enabled/healthy, analog sensors publish **no telemetry** (sensors become offline) and the reason is surfaced via node status (`analog_backend`, `analog_health`).
   - Simulation is still supported for CI/E2E and developer velocity, but it must be an **explicit opt-in** and must be **visibly indicated** end-to-end. The dashboard must not be able to enable simulation mode.

3) **Testing strategy (no production stubs)**
   - Unit tests mock hardware imports (`spidev`, GPIO backends) using `sys.modules` injection, rather than shipping a production “fake ADC” that can produce plausible values.

## Consequences
* Better: operators see clear “no data / backend unhealthy” instead of plausible fake voltages; debugging is faster and safer.
* Better: Pi 5 GPIO is handled via a supported character-device stack.
* Cost: adds/standardizes runtime deps (`gpiozero`, `lgpio`) on Pi nodes.
* Cost: requires explicit surfacing of backend/health in status and dashboard UI.
