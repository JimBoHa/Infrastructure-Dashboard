# TICKET-0030: hyperlocal weather forecast (open-meteo) hourly + weekly

**Status:** Open

## Description
Add a hyperlocal **hourly + weekly** weather forecast to the controller/dashboard using a publicly available internet API.

Decision: use **Open‑Meteo** (no key required) for:
- hourly forecasts (next 48–72 hours)
- daily/weekly forecasts (next 7–14 days)

Requirements:
- Provide a UX to configure **forecast coordinates** (latitude/longitude) with clear units and validation.
- Poll on a cadence + on-demand refresh; persist raw forecast points indefinitely (no retention policy).
- Serve bounded/downsampled windows to keep API payloads predictable and fast.
- Render clear graphs in the dashboard with units and a visible last-updated/provider status.

## Scope
- [ ] DB schema for weather forecast points (Timescale-friendly; keep raw indefinitely; bounded query endpoints)
- [ ] Controller poller + on-demand poll endpoint
- [ ] Setup Center config UI for coordinates (lat/lon) + status
- [ ] Dashboard graphs for hourly + weekly forecasts (units, legends, empty/error states)

## Acceptance Criteria
- [ ] Operators can set latitude/longitude in the dashboard (validated ranges) and save it to the controller.
- [ ] Controller polls Open‑Meteo and persists raw hourly + daily forecast points indefinitely.
- [ ] Dashboard shows:
  - hourly forecast (next 48–72h) graph(s) with explicit units
  - weekly forecast (next 7–14d) summary graph(s) with explicit units
  - provider status + last updated timestamp
- [ ] API responses are bounded (no unbounded range queries) and downsample as needed for longer windows.
- [ ] Relevant tests pass (unit/integration/UI). If full E2E cannot be run on the production host, document the clean-state blocker and what remains.

## Notes
Suggested variables (final selection at implementation time):
- temperature (°C)
- precipitation (mm)
- wind speed (m/s) and direction (°)
- cloud cover (%)
