# TICKET-0029: forecast.solar public plan PV forecast integration

**Status:** Open

## Description
Add Forecast.Solar PV production forecasts to the controller stack using **Forecast.Solar Public (free)** endpoints and surface them in the dashboard with clear units and operator-friendly PV setup UX.

This integration must:
- Support **per-node PV configuration** (only nodes with Renogy charge controllers need this initially).
- **Poll** Forecast.Solar on a predictable cadence and on-demand.
- **Persist raw forecast points indefinitely** (no retention policy), but **serve bounded/downsampled windows** to keep UI/API fast and predictable.
- Provide dashboard graphs that **overlay Forecast.Solar predicted PV vs Renogy measured PV** to validate real-world performance.

### Forecast.Solar Public plan configurables (from `https://api.forecast.solar/swagger.yaml`)
Public endpoints (no API key):
- `GET /estimate/{lat}/{lon}/{dec}/{az}/{kwp}`
- `GET /estimate/watts/{lat}/{lon}/{dec}/{az}/{kwp}`
- `GET /estimate/watthours/{lat}/{lon}/{dec}/{az}/{kwp}`
- `GET /estimate/watthours/day/{lat}/{lon}/{dec}/{az}/{kwp}`

Configurable parameters (Public):
- `lat` latitude (`-90..90`)
- `lon` longitude (`-180..180`)
- `dec` declination/tilt degrees (`0..90`)
- `az` azimuth degrees (`-180..180`; West=90, South=0, East=-90 per docs)
- `kwp` installed peak power in kWp (`>0`, `multipleOf 0.001`)
- optional query `time=utc` (ISO-8601 timestamps)

Non-Public endpoints are out of scope (e.g., weather requires a Professional key).

## Scope
- [ ] DB schema for PV forecast points (Timescale-friendly; keep raw indefinitely; bounded query endpoints)
- [ ] Controller poller + on-demand poll endpoint
- [ ] Per-node PV setup UX (diagrams/interactive figures; explicit units)
- [ ] Analytics graphs overlay predicted vs measured PV power (and optionally daily kWh)
- [ ] Provider status surfaced in the dashboard (last success/error/updated)

## Acceptance Criteria
- [ ] A node can be configured with PV parameters (lat/lon/tilt/azimuth/kWp) in the dashboard and saved to the controller.
- [ ] The controller stores Forecast.Solar forecast points in the DB indefinitely and returns bounded series windows for UI requests.
- [ ] The Analytics page shows:
  - predicted PV power for the configured node(s) (W)
  - measured PV power from Renogy telemetry for the same node(s) (W)
  - explicit units/legends and helpful empty/error states
- [ ] Forecast polling failures do not break the dashboard; errors are visible in a status panel and recover automatically when the provider recovers.
- [ ] Relevant tests pass (unit/integration/UI). If full E2E cannot be run on the production host, document the specific clean-state blocker and what remains.

## Notes
Implementation notes:
- Prefer Rust-first (controller poller + storage + routes).
- Use UTC timestamps end-to-end and store times as `timestamptz`.
- Keep API payloads predictable: cap points returned (e.g., 24h @ 5m = 288 points; 168h @ 1h = 168 points; daily = 7–14 points).
