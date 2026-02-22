# FEATURE-007: Weather Station (WS-2902) — One-Click Setup + 30s Trending

## Summary
Add a “Weather Station (WS-2902)” integration that can be configured by a non-technical operator and trends the default weather data points every **30 seconds**. Prefer a **push-based** integration using the station console’s “custom server” upload capability rather than inventing a bespoke TCP protocol.

## Business goal
Make local weather a first-class data source for the farm dashboard, enabling correlation between weather and irrigation/solar/other farm telemetry.

## Raw inputs (from feature checklist)
- Add a button to the webpage used to discover Pi5 nodes to connect to a TCP/IP based weather station (WS-2902) and pull default data points:
  - temperature
  - wind speed
  - wind direction
  - rain sensor info
  - UV
  - solar radiation
  - barometric pressure
- Trend at 30 seconds intervals.
- Close to one-click configuration; easy for non-technical people.

## Key correction (based on vendor-supported capabilities)
WS-2902-class consoles typically support uploading observations to a configurable server endpoint using Weather Underground and/or Ambient-style querystring protocols. This allows the station to “push” data to the farm dashboard server over HTTP, which is simpler and more reliable than polling.

## Scope
### In scope
- Dashboard UI wizard to add a WS-2902 integration:
  - generates/assigns an ingest endpoint URL (with token)
  - displays exact values the user must enter into the station app (server host, path, port, interval, protocol type)
- Ingest endpoint implementation:
  - accept station uploads
  - parse parameters
  - map to platform sensors/metrics
  - store/trend at 30s
- Health/status display (last update timestamp, missing fields).

### Out of scope
- Supporting every possible weather station model.
- Building a generic weather network ingestion platform.
- Forecast providers (separate existing feature area).

## Functional requirements
### FR1. Integration wizard (non-technical)
- Add a UI action: “Add Weather Station”.
- Wizard collects:
  - station nickname
  - upload protocol choice (Wunderground vs Ambient-style)
  - desired upload interval (default 30s)
- Wizard outputs:
  - ingest endpoint host + port + path
  - step-by-step instructions to configure the station/app
  - a “Test ingestion” button that waits for the next upload and confirms receipt

### FR2. Ingest endpoint
- Accept station uploads via HTTP (GET with query params is acceptable for MVP).
- Authenticate uploads using a per-station token in:
  - query param, header, or unique path segment (choose one and document it)
- Parse and store the required fields:
  - temperature
  - wind speed
  - wind direction
  - rain
  - UV
  - solar radiation
  - barometric pressure
- Handle units explicitly and convert to platform standard units (document choice).

### FR3. Auto sensor creation + trending
- On integration creation, the platform auto-creates a set of sensors for the weather station (deterministic IDs, normal naming).
- Sensors are configured for 30s trending cadence (or “ingest on update” if station pushes at ~30s).

### FR4. Status and troubleshooting
- Integration status page shows:
  - last upload timestamp
  - upload count last 24h
  - which required fields were missing in the last payload
- Provide a copyable “example payload” log (redacted).

## Security requirements
- Ingest endpoint must not be anonymously writable.
- Token rotation must be supported (invalidate old token and issue new one).

## Non-functional requirements
- Must handle at least **1 upload per 30s** continuously without memory growth.
- Data must be resilient to occasional missing uploads; mark staleness rather than failing.

## Repo boundaries (where work belongs)
- `apps/core-server/`
  - add an “integrations/weather-station” model and ingest endpoint
  - update OpenAPI + generated clients
- `apps/dashboard-web/`
  - integration wizard UI + status page
- `docs/runbooks/`
  - WS-2902 setup guide with screenshots/field mapping.

## Acceptance criteria
1) A user can create a WS-2902 integration in the dashboard in < 2 minutes.
2) The station is configured with the provided server settings and begins uploading.
3) Within 2 minutes, the platform displays all required data points and trends them at ~30s cadence.
4) Disabling the integration stops accepting uploads.
5) Token rotation works (old token rejected; new token accepted).
6) `make e2e-web-smoke` remains green (no regressions).

## Test plan
- Unit tests: payload parsing + unit conversions + token auth.
- Integration test: simulated station uploader sends payloads at 30s cadence; verify stored metrics.
- Manual: validate on a real WS-2902 station on LAN.

## Dependencies
- Requires the core-server integration framework and sensor creation paths (already present for other sources).

## Risks / open questions
- Confirm the exact station firmware/app path for configuring custom server upload and supported protocols for the target WS-2902 model.
- Decide which protocol to support for MVP (Wunderground vs Ambient) based on easiest field mapping.
