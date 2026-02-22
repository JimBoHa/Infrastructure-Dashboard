# TICKET-0019: WS-2902: controller-initiated TCP IP integration

**Status:** Done (scoped to push-only)

## Description
The original client request described a “TCP/IP connect + pull” style integration for WS‑2902‑class weather stations. In practice, WS‑2902 consoles typically support **custom server uploads** (Weather Underground / Ambient-style querystring), which is more reliable and easier to support on a local LAN.

This ticket closes the spec gap by explicitly scoping WS‑2902 to **push-only**:
- The controller creates a tokenized ingest endpoint.
- The operator configures the station to upload to that endpoint.
- The product **must not claim** controller-initiated “connect/pull” semantics in user-facing UI/docs.

If controller-initiated polling is desired later, it should be tracked as a separate ticket with a concrete protocol spec, compatibility matrix (firmware/app), and a deterministic simulator.

## Scope
- [x] Confirm product scope: push-only (no polling).
- [x] Ensure dashboard UX and docs describe “custom server upload” configuration (not polling).

## Acceptance Criteria
- [x] No user-facing copy claims WS‑2902 controller polling/TCP connect.
- [x] Runbook documents push-based setup and token rotation.

## Notes
Related implementation:
- Core API: `apps/core-server-rs/src/routes/weather_stations.rs` (tokenized ingest endpoint + status + rotation)
- Dashboard UI: `apps/dashboard-web/src/features/nodes/components/WeatherStationModal.tsx`
- Runbook: `docs/runbooks/ws-2902-weather-station-setup.md`
