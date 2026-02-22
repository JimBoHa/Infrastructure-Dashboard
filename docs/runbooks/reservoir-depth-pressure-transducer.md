# Reservoir depth pressure transducer (ADS1263 + 4–20 mA)

Configure and validate a 4–20 mA depth transducer on a Pi 5 node using the Waveshare **High‑Precision AD HAT** (ADS1263).

**Hard rule:** Production is fail‑closed; simulation is test/dev only and cannot be enabled via the dashboard.

**Default wiring assumption for this project:** `AIN0` measures the shunt voltage vs `AINCOM` (single‑ended).

## Code locations
- ADS1263 driver: `apps/node-agent/app/hardware/ads1263_hat.py`
- Background sampler (keeps ADC reads off the HTTP loop): `apps/node-agent/app/hardware/background_sampler.py`
- Current-loop conversion + quality markers: `apps/node-agent/app/services/publisher.py` (`_convert_current_loop_depth`)

## Pi 5 requirements (production)
- SPI0 enabled (`/dev/spidev0.0` present). Production deploy-over-SSH enables this automatically (it will set `dtparam=spi=on` and reboot once if needed).
- Node runtime deps: `spidev`, `gpiozero`, `lgpio` (Pi 5 uses character-device GPIO; avoid `RPi.GPIO`).

## Wiring (low-side shunt; recommended)
Use a shunt resistor to convert loop current into a safe voltage:

- 24V+ → transducer **red**
- transducer **black** → top of shunt resistor
- bottom of shunt resistor → 24V− (0V)
- ADS1263:
  - `AIN0` → top of shunt resistor
  - `AINCOM` → 24V− (0V)

### Expected shunt voltage (163Ω)
For a 4–20 mA loop:

- 4 mA × 163Ω ≈ **0.652 V**
- 20 mA × 163Ω ≈ **3.26 V**

If readings are near 0V or pinned near Vref, treat as wiring/config fault and check `analog_health.last_error`.

## Node config (ADS1263)
Enable the HAT and set a safe data rate (≥50 SPS; recommend 100 SPS):

```json
{
  "ads1263": {
    "enabled": true,
    "data_rate": "ADS1263_100SPS",
    "scan_interval_seconds": 0.25
  }
}
```

## Sensor config (reservoir depth current loop)
Configure a `water_level` sensor entry and publish at ~2 Hz:

```json
{
  "sensor_id": "reservoir-depth",
  "name": "Reservoir Depth",
  "type": "analog",
  "unit": "ft",
  "location": "Reservoir",
  "channel": 0,
  "interval_seconds": 0.5,
  "current_loop_shunt_ohms": 163.0,
  "current_loop_range_m": 5.0
}
```

## Telemetry quality codes (current loop)
- `quality=0`: OK
- `quality=1`: low current fault (open circuit / sensor fault)
- `quality=2`: high current fault (overrange / wiring fault)
- `quality=3`: config error (treated as fault)
- `quality=4`: backend unavailable / read failed

**Fail-closed (production):** any non-zero quality is treated as a fault and telemetry is suppressed (no publish). The controller should show the sensor as **offline/unavailable** rather than displaying a plausible-but-wrong depth.

## Tier‑A verification (installed controller; no resets)
1. In the dashboard: Nodes → **Pi5 Node 1** → Hardware sensors
   - Confirm `analog_backend=ads1263` and `analog_health.ok=true`.
2. In the dashboard: Sensors & Outputs → Pi5 Node 1
   - Confirm “Reservoir Depth” is present and updates continuously.
3. Sanity-check the raw electrical signal (optional but recommended):
   - Convert displayed depth back to expected loop current and confirm shunt voltage is plausible (~0.65–3.26 V).

## Troubleshooting quick hits
- `analog_health.last_error` mentions `spidev` / `No such file`:
  - SPI0 is likely disabled; re-run the dashboard “Deploy over SSH” job (it enables SPI0 and reboots automatically).
- `analog_health.last_error` mentions `gpiozero` / GPIO backend:
  - Ensure the node-agent venv has `gpiozero` + `lgpio` installed and restart `node-agent`.
- Reads are stable but incorrect:
  - Confirm `channel=0` (AIN0) and `AINCOM` wiring to 24V− (low-side shunt reference).
