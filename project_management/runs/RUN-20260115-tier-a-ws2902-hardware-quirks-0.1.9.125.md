# RUN-20260115 — Tier A WS-2902 on-hardware ingest quirks + dashboard validation (installed controller `0.1.9.125`)

**Goal:** Get the real WS2902_1 station (`10.255.8.39`) uploading to the installed controller using the short `/api/ws/<token>` path and confirm data renders in the dashboard UI. **No DB/settings reset.**

## Preconditions

- Setup-daemon health:
  - `curl -fsS http://127.0.0.1:8800/healthz` → `{"status":"ok"}`
- Core-server health:
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`

## Diagnosis (real station behavior)

- The station’s “Customized → Wunderground” uploader sends observations as WU-style key/value pairs.
- Firmware quirk observed:
  - If the configured path does **not** end with `?`, the station appends the query payload directly to the path (no `?` delimiter), producing a request like:
    - `/api/ws/<token>ID=...&tempf=...`
  - If the configured path **does** end with `?`, the station produces a normal URL:
    - `/api/ws/<token>?ID=...&tempf=...`
- The station also emits sentinel “missing” values for some fields (e.g. `baromin=-9999`).

## Fix (controller)

- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.125.dmg`
- Pointed setup-daemon at the new DMG:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.125.dmg"}'`
- Upgraded:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Confirmed:
  - `curl -fsS http://127.0.0.1:8800/api/status` → `current_version: 0.1.9.125`

## Station configuration (WS2902_1)

- Station IP: `10.255.8.39`
- Customized upload enabled.
- Protocol: Wunderground-style.
- Controller:
  - Host: `10.255.8.66`
  - Port: `8000`
  - **Path:** `/api/ws/79e028f445193fee54c52cd0`
  - Interval: `30s`

Note: After the controller fix, the station is accepted even if the path is mis-formed (missing `?`), but setting the path with a trailing `?` is still recommended for interoperability with other servers.

## Dashboard validation (Tier A)

- Screenshot evidence (captured + viewed):
  - `manual_screenshots_web/20260115_ws2902/nodes_ws2902_card.png`
  - `manual_screenshots_web/20260115_ws2902/sensors_ws2902.png`

## Cleanup

- Soft-removed the earlier non-functional nodes so they are no longer visible and their original names remain reusable:
  - “Weather station — Weather Station 1”
  - “Weather station — Weather Station 2”

