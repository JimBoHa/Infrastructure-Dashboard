# TICKET-0005: Reservoir depth pressure transducer integration (4–20 mA current loop)

**Status:** Open

## Description
Integrate a reservoir depth pressure transducer into the Farm Dashboard system **via a node** so the depth reading can be collected, buffered, and reported to the controller reliably (no dropped config traffic, no event-loop blocking).

Reference note (verbatim design/wiring guidance):
- `docs/runbooks/reservoir-depth-pressure-transducer.md`

This ticket is a long-form requirements dump for the related `NA-*` / analytics tasks in `project_management/TASKS.md`.

## Scope
- Node-side ingestion of a 2‑wire **4–20 mA current loop** transducer powered from 24 VDC (24 V never enters the Pi; the node measures loop current via a shunt resistor / ADC).
- Support ~10 sensors @ 2 Hz without blocking the node HTTP server/event loop:
  - ADC/BLE I/O runs off the HTTP event loop (background task + thread offload or separate process).
  - Queue/backpressure behavior is explicit so inbound TCP config is processed, not dropped.
- Practical ADC constraints:
  - If using ADS1263 scanning sequential channels, set SPS ≥ 50 (recommend 100) to meet the conversion budget.
- Convert voltage → mA → depth with explicit fault detection (open circuit / overrange) and clear status reporting.
- Provide a mapping strategy so the controller can surface the depth reading (at minimum in Trends; optionally also wired into Analytics Water as `reservoir_depth`).

## Out of Scope (for this ticket)
- Field/hardware validation of the full outdoor wiring run (surge/ground potential; isolators; enclosure details).
- Final calibration against real reservoir measurements (sensor offsets, temperature drift).

## Acceptance Criteria
- A node can be configured with a reservoir depth transducer channel, shunt resistance, and range, and will publish depth telemetry reliably at the configured cadence.
- Node config endpoints remain responsive during sampling (no blocking loop on the HTTP event loop).
- Fault modes are observable (status + alarm/telemetry markers when current is out of expected range).
- Automation exists to validate the non-blocking sampling architecture and conversion math without requiring the physical sensor.

## Notes
- Controller Analytics Water output (`GET /api/analytics/water`):
  - Unit strategy: `reservoir_depth[].value` is reported in **feet** (ft) for consistency with legacy analytics naming (`reservoir_depth_ft`). If the publishing sensor’s unit is inches (`in`) or meters (`m`), the controller converts to feet before emitting the series.
  - Mapping strategy: the controller selects the “reservoir depth” sensor from `sensors` where `type='water_level'` using:
    1) explicit `sensors.config.analytics_role == 'reservoir_depth'` (preferred), else
    2) heuristics (`name` or `config.location` contains “reservoir”).
- Prefer Rust for new node-side samplers if it materially improves isolation/reliability; keep the orchestration/config surface stable.
