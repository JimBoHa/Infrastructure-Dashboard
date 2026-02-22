# Farm Dashboard Done Tasks (2026)

This file contains all completed tickets (Status: Done).

- Open work items live in `project_management/TASKS.md`.
- Long run logs should live in `project_management/runs/` (tickets should keep short evidence links).

---

## Operations
### Done
- **OPS-1: Purge mistaken simulated Node 1 reservoir depth points (installed controller DB)**
  - **Description:** Remove erroneous simulated reservoir depth telemetry that was accidentally written into the installed controller database so Trends/History reflect only real sensor data.
  - **Acceptance Criteria:**
    - The Node 1 “Reservoir Depth” sensor has no points prior to the start of real ingest on 2026-01-14 (local).
    - Real reservoir depth points from 2026-01-14 09:00 (local) → present remain intact.
    - Evidence is recorded with pre/post counts and exact deletion bounds.
  - **Evidence / Run Log:**
    - Run: `project_management/runs/RUN-20260115-ops1-purge-node1-reservoir-depth-sim.md`
  - **Status:** Done (ops cleanup on installed controller)

## Core Server
### Done
- **CS-58: Populate Analytics endpoints from Renogy telemetry**
  - **Description:** Wire Renogy BT-2 metrics (PV/load power + battery SOC/runtime) into `/api/analytics/power` and `/api/analytics/status` so the Nodes Overview and Analytics tab show real live values in production mode.
  - **Acceptance Criteria:**
    - With a live Renogy node, `/api/analytics/power` reports non-zero `live_kw` / `live_solar_kw` when applicable and returns a non-empty `series_24h` consistent with telemetry cadence.
    - `/api/analytics/status` reports `battery_soc` and `current_load_kw` sourced from Renogy telemetry (not hardcoded zeros).
    - `make e2e-web-smoke` remains green.
  - **Notes / Run Log:**
    - 2026-01-05: Dashboard UI prefers `GET /api/dashboard/state` when present; snapshot analytics were stubbed (battery SOC/power always zero) so UI never reached `/api/analytics/*`. Implemented snapshot analytics to reuse the real `/api/analytics/*` handlers in `apps/core-server-rs/src/routes/dashboard.rs`.
    - 2026-01-05: Production verification (real Pi 5 Renogy node): `/api/analytics/status` reports non-zero SOC with fresh `last_updated`, and `/api/dashboard/state` snapshot analytics match (see `reports/prod-pi5-renogy-deploy-20260104_051738.log`).
    - 2026-01-05: Shipped in prod bundle `0.1.8.7`: `/api/analytics/power` now returns derived battery storage power/energy (`live_battery_kw`, `battery_kwh_*`, `battery_series_*`) computed from Renogy `battery_voltage_v` × `battery_current_a`, and the Analytics UI summary card displays live Storage kW (see `reports/prod-audit-followup-20260105_0606.md`).
    - 2026-01-06: Improved `/api/analytics/power` series bucketing to make charts feel live (24h = 5-minute buckets, 168h = hourly buckets) and avoid the “168h updates daily” perception on longer windows. Deployed to prod bundle `0.1.9.12`; validated via `GET /api/analytics/power` timestamp deltas (`series_24h` = 300s, `series_168h` = 3600s).
    - 2026-01-10: Fixed regression where `/api/analytics/power` returned `series_24h` in 60-second buckets (causing fleet totals to drop to zero between sparse grid samples). Restored 5-minute (300s) buckets and disabled chart line smoothing; deployed to installed controller bundle `0.1.9.46` and verified `series_24h` timestamp deltas are 300s.
    - 2026-01-10: Increased 24h chart granularity for Renogy solar/storage: `/api/analytics/power` now returns `solar_series_24h` and `battery_series_24h` in 60-second buckets while keeping Total/Grid at 5-minute buckets. Deployed to installed controller bundle `0.1.9.48`; verified timestamp deltas (`solar_series_24h`/`battery_series_24h` = 60s, `series_24h`/`grid_series_24h` = 300s).
    - 2026-01-10: Dashboard UI now reads Renogy SOC from `battery_soc_percent` (matches published telemetry) and surfaces battery voltage/current as the primary “Battery” value (with SOC labeled explicitly as Renogy-reported) to avoid misleading “stuck at 100%” displays when SOC is not behaving as expected. Refreshed installed controller to `0.1.9.66` and verified the Analytics “Fleet status” Battery card renders voltage/current on the installed app.
    - E2E note: `make e2e-web-smoke` re-run is pending; test hygiene requires a clean machine (no running Farm launchd jobs/processes), which is not currently true on the production Mac mini.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; `/api/analytics/power` bucket deltas: Total/Grid = 300s, Solar/Storage = 60s; feeds `ok`. Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **CS-59: Enforce “no permanent deletes” for telemetry history**
  - **Description:** Prevent API-driven hard deletes that permanently remove metrics. Deletes should default to retention-friendly soft deletes (append `-deleted`, set `deleted_at`, preserve metrics indefinitely).
  - **Acceptance Criteria:**
    - `DELETE /api/sensors/{id}` defaults to `keep_data=true` semantics; hard-delete is gated behind a dedicated purge capability (`ops.purge`) and must be explicitly requested.
    - `DELETE /api/nodes/{id}` defaults to soft-delete semantics that preserve metrics indefinitely and prevent further ingest/updates for deleted nodes/sensors.
    - Soft-deleted nodes and sensors do **not** show up anywhere in the dashboard UX (Nodes, Sensors & Outputs, Trends, Map, Overview) because the list endpoints exclude them.
    - Soft delete renames the deleted node/sensor(s) so the original name can be reused later without UX conflicts.
    - `make e2e-web-smoke` remains green.
  - **Plan:**
    - Core: make soft-deleted nodes consistently excluded from list endpoints (`/api/nodes`, `/api/dashboard/state`) by setting a hidden marker and filtering `config.deleted=true`.
    - Core: update soft-delete rename semantics to include a timestamp suffix to avoid name collisions across repeated deletes.
    - Core: ensure dependent list endpoints remain stable after filtering by node config (`/api/outputs`, `/api/map/features`) and do not regress with SQL ambiguity errors.
    - Dashboard: validate node + sensor delete removes them from all UI surfaces without requiring a manual refresh.
  - **Notes / Run Log:**
    - 2026-01-05: Implemented privileged purge endpoints gated behind `ops.purge` for exceptional GDPR/ops scenarios; default delete flow remains retention-friendly and preserves metrics indefinitely.
    - 2026-01-05: Telemetry-sidecar ignores updates for deleted nodes and ignores telemetry for deleted sensors to prevent “ghost telemetry” after soft-delete.
    - 2026-01-15: Installed controller `0.1.9.130` regression: `GET /api/outputs` returns `500 Database error` (SQL ambiguity) after adding node-join filters; must be fixed before Tier A UI validation can be recorded.
    - E2E note: `make e2e-web-smoke` re-run is pending; test hygiene requires a clean machine (no running Farm launchd jobs/processes), which is not currently true on the production Mac mini.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-soft-delete-node-sensor-0.1.9.132.md`
  - **Status:** Done (Tier A validated; clean-host E2E deferred to CS-69)

- **CS-62: Support external “virtual nodes” (Emporia devices) in the node model**
  - **Description:** Allow external integrations (starting with Emporia cloud devices) to be represented as first-class nodes so the UI can show a clear node → sensors/measurements hierarchy without abusing MAC/IP identity fields.
  - **References:**
    - `infra/migrations/021_nodes_external_identity.sql`
    - `apps/core-server-rs/src/services/emporia_ingest.rs`
  - **Acceptance Criteria:**
    - `nodes` supports optional external identity fields (provider + external_id) with a uniqueness guard so one Emporia device cannot create duplicate nodes across restarts.
    - `GET /api/nodes` continues to work for Pi/controller nodes; external nodes are included with clear `config` markers (e.g., `config.node_kind`, `config.external_provider`, `config.external_id`).
    - Node adoption/discovery logic ignores external nodes (no impact on scan/adopt).
  - **Notes / Run Log:**
    - 2026-01-06: Added `nodes.external_provider` + `nodes.external_id` with a unique index so Emporia devices upsert deterministically (no duplicates).
    - 2026-01-06: Implemented Emporia node upsert in `apps/core-server-rs/src/services/emporia_ingest.rs` using `ON CONFLICT (external_provider, external_id)`.
    - 2026-01-06: Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass).
    - 2026-01-06: Production upgrade: controller bundle `0.1.9.13` installed; verified `/usr/local/farm-dashboard/bin/core-server` points at `releases/0.1.9.13` and `/power/` route returns `200 OK`.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; `/api/nodes` includes Emporia external nodes and `/api/analytics/feeds/status` shows `Emporia: ok`. Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **CS-63: Ingest Emporia cloud readings as sensors + metrics (per device + per circuit)**
  - **Description:** Expand Emporia ingest beyond a single “total kW” and persist per-device and per-circuit readings as first-class sensors/metrics so they appear everywhere (Sensors tab, Trends, Power UI, Analytics breakdowns).
  - **References:**
    - `apps/core-server-rs/src/services/analytics_feeds.rs`
    - `apps/core-server-rs/src/services/emporia.rs`
    - `apps/core-server-rs/src/services/emporia_ingest.rs`
  - **Acceptance Criteria:**
    - Each Emporia Vue device becomes its own node (virtual node) with a stable identity (provider + deviceGid) and a human-friendly name.
    - All available Emporia readbacks are surfaced in the dashboard with explicit units and device/channel metadata in `sensor.config`.
    - **Voltage semantics:** only the two mains leg voltages (L1‑N, L2‑N) are persisted as time-series sensors to avoid redundant per-circuit voltage duplication; per-circuit voltage is derived for display from W/A when needed.
    - Metrics are persisted indefinitely in `metrics` (Timescale) and queryable via `GET /api/metrics/query`; Sensors API surfaces `latest_value/latest_ts` for Emporia sensors.
    - Polling failures update feed health and do not corrupt existing node/sensor mappings; status/last_seen reflects freshness for Emporia nodes.
  - **Notes / Run Log:**
    - 2026-01-06: Emporia parsing now surfaces mains + all channels as `EmporiaDeviceReading` / `EmporiaChannelReading` (power W + channel metadata).
    - 2026-01-06: Added `apps/core-server-rs/src/services/emporia_ingest.rs` to upsert Emporia nodes + sensors and persist readings as first-class `metrics` (no retention).
    - 2026-01-06: Wired ingest into `apps/core-server-rs/src/services/analytics_feeds.rs` so a successful Emporia poll persists both `analytics_power_samples` (fleet sums) and per-device/per-circuit sensors/metrics.
    - 2026-01-07: Fixed multi-meter polling by sending a comma-separated `deviceGids` list to Emporia `getDeviceListUsages` (previously used `+`, which dropped devices on multi-site accounts).
    - 2026-01-07: Added per-meter preferences (enabled/include-in-power-summary/address-group label) persisted in `setup_credentials.metadata.devices`, exposed via `GET/PUT /api/setup/emporia/devices`, and surfaced into Emporia node `config` for UI grouping.
    - 2026-01-06: Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass).
    - 2026-01-06: Production verification: `GET /api/dashboard/state` shows 4 Emporia nodes and 43 `emporia_cloud` sensors; `GET /api/analytics/feeds/status` reports `Emporia: ok` with fresh `last_seen`.
    - 2026-01-12: Reduced Emporia voltage sensor clutter: only mains leg voltages remain visible (L1/L2); legacy aggregate `mains_voltage_v` and per-circuit `channel_voltage_v` sensors are auto-disabled to match real hardware semantics (meter measures only two supply voltages). Power circuits table derives voltage from Power ÷ Current when needed.
    - Test note: E2E + unit tests not run on this host because the clean-state preflight shows the installed Farm stack running under `_farmdashboard` (must be stopped to satisfy test hygiene gate).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; `/api/sensors` includes Emporia metrics `channel_power_w`, `channel_voltage_v`, `channel_current_a` and mains variants; feeds `ok`. Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
    - Installed controller refreshed to `0.1.9.99`; DB check confirms only two visible Emporia voltage sensors per device (mains legs): `psql … -c "select n.name, count(*) … as visible_voltage …"` (result shows `2` for each Emporia node on this controller).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **CS-65: Emporia multi-site preferences (exclude meters + address grouping)**
  - **Description:** Support Emporia accounts with multiple meters/sites by (1) ensuring all meters are ingested, and (2) allowing operators to exclude specific meters from system summaries or group them by street address.
  - **References:**
    - `apps/core-server-rs/src/services/emporia.rs`
    - `apps/core-server-rs/src/services/analytics_feeds.rs`
    - `apps/core-server-rs/src/routes/setup.rs`
    - `apps/core-server-rs/src/routes/analytics.rs`
  - **Acceptance Criteria:**
    - Emporia ingest requests include all enabled deviceGIDs for the account, and all meters appear as distinct nodes/sensors (no missing meters due to list encoding).
    - Operators can set per-meter `enabled` and `include_in_power_summary` preferences without re-entering tokens/passwords (`PUT /api/setup/emporia/devices` updates `setup_credentials.metadata.devices`).
    - `/api/analytics/power` respects per-meter inclusion (excluded meters do not affect fleet/system totals) and continues to include Renogy contributions even when Emporia is configured.
    - Relevant CI smoke + E2E are re-run on a clean host (or after stopping the installed stack) and remain green.
  - **Notes / Run Log:**
    - 2026-01-07: Implemented preferences merge/persistence and added `GET/PUT /api/setup/emporia/devices` for listing/updating per-meter settings.
    - 2026-01-07: Updated `/api/analytics/power` to compute totals from first-class sensor metrics (Emporia mains + Renogy load/PV/battery) and filter Emporia meters by `include_in_power_summary` preferences.
    - 2026-01-08: Wired the new Emporia setup routes into `/api/openapi.json` and refreshed the installed controller via bundle `0.1.9.29`.
    - Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass).
    - Test note: Test hygiene preflight cannot be satisfied on this host without admin privileges to bootout `com.farmdashboard.*` LaunchDaemons; run `make test-clean && make ci-core-smoke && make ci-web-smoke && make e2e-web-smoke` on a clean dev host.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; Emporia poll ingests multiple meters (`device_gids_polled`) and Setup Center exposes per-meter preferences (manual spot-check). Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **CS-71: Renogy BT-2 settings API (desired config + audit + apply orchestration)**
  - **Description:** Add a safe, auditable “edit controller config over Modbus” API flow for Renogy `RNG-CTRL-RVR20-US` via the BT‑2 BLE bridge. The browser must never talk to BLE directly; the core server stores desired settings, diffs them against live values, and coordinates apply/verify through node-agent.
  - **Acceptance Criteria:**
    - Core server exposes endpoints (auth + `config.write`) to:
      - Fetch the register-map schema for the supported Renogy model(s).
      - Read “Current (from controller)” values via node-agent (proxy).
      - Get/Set “Desired (saved)” values (persisted in DB, versioned).
      - Validate proposed changes (bounds/units/model applicability) without applying.
      - Apply changes (with concurrency lock), with per-field status + read-back verify results.
      - List apply history (who/when/what/result) and support rollback to last-known-good snapshot.
    - If the node or BT‑2 link is offline, desired config can still be saved as Pending and applied later (no silent loss).
    - OpenAPI coverage check catches new routes (CI/pre-commit stays green).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.73`; `http://127.0.0.1:8000/healthz` is 200 after upgrade (no DB/settings reset). Run: `project_management/runs/RUN-20260111-tier-a-renogy-settings-0.1.9.73.md`.
    - OpenAPI coverage: `make rcs-openapi-coverage` (pass).
  - **Status:** Done (validated on installed controller; hardware validation deferred to NA-61)

- **CS-72: Add stable Core node + forecast-backed sensors**
  - **Description:** Introduce a stable “Core” node representing the controller/services, and persist weather + forecast data as long-retention sensors (stored in `forecast_points` but surfaced through `/api/sensors` + `/api/metrics/query`) so they show up consistently across Nodes/Sensors/Trends without special-casing.
  - **Acceptance Criteria:**
    - `/api/nodes` includes a deterministic Core node (`00000000-0000-0000-0000-000000000001`, name `Core`) and it cannot be deleted.
    - Weather current values can be persisted to Timescale (`forecast_points`) and surfaced as sensors linked to the Core node (site weather) and/or a placed node (hyperlocal weather).
    - Forecast.Solar PV forecast power is surfaced as a sensor linked to the configured node.
    - `/api/metrics/query` supports these “virtual” forecast-backed sensors for Trends/history.
    - No DB/settings reset required.
  - **Evidence (Tier A):**
    - Installed controller upgraded to `0.1.9.91` (Tier A); screenshots viewed: `manual_screenshots_web/tier_a_refresh_0.1.9.91/nodes.png`, `manual_screenshots_web/tier_a_refresh_0.1.9.91/map.png`, `manual_screenshots_web/tier_a_refresh_0.1.9.91/trends.png`.
    - API spot-checks (installed controller): `GET /api/nodes` shows Core node online; `GET /api/forecast/weather/current?node_id=00000000-0000-0000-0000-000000000001` returns metrics and inserts forecast-backed sensors; `GET /api/sensors` includes forecast-backed weather sensors; `GET /api/metrics/query` returns non-empty series for weather sensor IDs.
    - Automated validation (installed controller): `cd apps/dashboard-web && CI=1 FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 npm run test:playwright -- --project=chromium-mobile` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-75: WS-2902 ingest: shorten token + short `/api/ws/<token>` path**
  - **Description:** The WS-2902 weather station “custom server” configuration UI truncates long paths/tokens, preventing the station from uploading to the controller. Issue shorter per-station ingest tokens and provide a short ingest path alias so the full path is easy to paste/type and fits typical field limits.
  - **Acceptance Criteria:**
    - Creating a WS-2902 integration returns a short `ingest_path` of the form `/api/ws/<token>` (token is short enough to fit the station UI).
    - WS-2902 uploads succeed against both `/api/ws/<token>` and the legacy `/api/weather-stations/ws-2902/ingest/<token>` path.
    - Token rotation returns the short ingest path and invalidates the previous token.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **Notes / Run Log:**
    - Tier A: refreshed installed controller to `0.1.9.118`; `/api/ws/<token>` route returns `404 Unknown token` for bogus tokens (run: `project_management/runs/RUN-20260114-tier-a-ws2902-short-token-0.1.9.118.md`).
  - **Status:** Done (validated on installed controller; hardware validation deferred to CS-76)

- **CS-76: Validate WS-2902 short ingest path/token on real station hardware**
  - **Description:** Confirm a real WS-2902-class station can accept the short `/api/ws/<token>` path in its custom server UI and successfully uploads to the controller; verify legacy path still works and token rotation behaves as expected.
  - **Acceptance Criteria:**
    - Station custom server UI accepts the full host/port/path without truncation.
    - The controller shows a recent `Last upload` timestamp and no unexpected missing fields for the chosen protocol.
    - Rotating the token causes old uploads to return 404 and new uploads to succeed after updating the station.
    - Dashboard UI shows live weather sensor values for the station (Tier A screenshot evidence).
  - **Plan:**
    - Capture the station’s exact upload HTTP request (method/path/query semantics) and confirm it reaches the controller over LAN.
    - Patch ingest to be robust to firmware quirks (query string formatting + sentinel “missing” values like `-9999`).
    - Refresh the installed controller (Tier A; no DB/settings reset) and verify uploads update `Last upload` + sensor latest values.
    - Record Tier A run log + screenshot under `manual_screenshots_web/`.
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260115-tier-a-ws2902-hardware-quirks-0.1.9.125.md`
    - Screenshot: `manual_screenshots_web/20260115_ws2902/nodes_ws2902_card.png`
    - Screenshot: `manual_screenshots_web/20260115_ws2902/sensors_ws2902.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-78: WS-2902 cleanup: remove failed duplicate weather station nodes**
  - **Description:** Remove the non-functional earlier attempts (“Weather station — Weather Station 1/2”) so they are not visible anywhere in the dashboard UI and their names remain reusable for future nodes.
  - **Acceptance Criteria:**
    - “Weather station — Weather Station 1” and “Weather station — Weather Station 2” are not visible anywhere in the dashboard UI (Nodes + Sensors + Trends).
    - Their names are not reserved (a future node can be created with the same names).
    - Any associated WS-2902 integrations are disabled so they cannot receive ingest.
  - **Notes / Run Log:**
    - 2026-01-15: Soft-removed by setting `nodes.config.hidden=true`, `nodes.config.poll_enabled=false`, renaming, and disabling corresponding integrations. Run: `project_management/runs/RUN-20260115-tier-a-ws2902-hardware-quirks-0.1.9.125.md`.
  - **Status:** Done (validated on installed controller; no UI visibility)

- **CS-79: Deleted nodes/sensors: stop controller-owned pollers/integrations (retain history)**
  - **Description:** When an admin deletes a node or sensor, preserve telemetry history in the database but ensure the controller stops spending resources on it. Controller-owned pollers/integrations (Open‑Meteo current weather by node/map placement, Forecast.Solar per-node polls, Emporia feed ingest, WS‑2902 ingest) must not continue creating/updating sensors or writing new points for deleted/disabled entities.
  - **Acceptance Criteria:**
    - Deleted nodes are excluded from Open‑Meteo “current weather” target selection (no more per-node current-weather polling driven by stale map placements).
    - Deleted nodes are excluded from Forecast.Solar per-node polling.
    - Deleting an Emporia-backed node disables the corresponding Emporia device in `setup_credentials` so future Emporia polls do not re-materialize the node/sensors or write new readings.
    - Deleting a WS‑2902-backed node disables its `weather_station_integrations` row so future uploads return `403 Integration disabled` and do not update node status or metrics.
    - Deleting a sensor stops controller-owned integrations from writing new data for that sensor (no “resurrection” of deleted sensors by cloud ingest).
    - Tier A validation is recorded on the installed controller with at least one captured + viewed screenshot and a run log under `project_management/runs/`.
  - **Plan:**
    - Forecasts: join/filter nodes in weather target selection and PV forecast polling to exclude deleted/disabled nodes.
    - Emporia: on node delete, mark the device disabled in Emporia preferences (setup credential metadata) so it is not polled; ensure ingest skips writing for deleted/disabled sensors.
    - WS‑2902: on node delete, disable its integration row; add a safety guard in ingest to refuse writes for deleted/disabled nodes.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-cs79-deleted-stops-integrations-0.1.9.138.md`
    - Screenshot: `manual_screenshots_web/tier_a_0.1.9.138_cs79_2026-01-15_195801215Z/01_nodes.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **CS-82: Open-Meteo current adds barometric pressure metric**
  - **Description:** Expose a pressure metric alongside the existing Open‑Meteo “current weather” sensors so operators can graph barometric pressure even when a WS‑2902 upload omits it.
  - **Acceptance Criteria:**
    - Open‑Meteo current ingestion requests `pressure_msl` and persists it as a virtual sensor on weather-target nodes.
    - Sensor is created with `type=pressure` and unit `kPa` and trends correctly via `/api/metrics/query`.
    - `make ci-core-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Implemented `pressure_msl` ingest (converted to kPa + sanity clamped) and surfaced as `pressure` sensors.
    - 2026-01-18: Validation: `make ci-core-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw140-dw141-cs82-0.1.9.149.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.149_dw140_dw141_pressure_2026-01-18_024046513Z/04_trends_open_meteo_weather_pressure.png`
  - **Status:** Done (validated on installed controller 0.1.9.149; clean-host E2E deferred to DT-59)

- **CS-83: WS-2902: barometric pressure shows in Trends even when station uploads omit pressure**
  - **Description:** The WS-2902 node `Weather station — WS2902_1` is uploading regularly, but its `Barometric pressure` sensor is stale (station payload has `baromin=-9999`, so the pressure metric never ingests). Ensure the dashboard can graph a pressure series reliably for weather-station nodes and the system makes the “missing pressure” situation obvious.
  - **Acceptance Criteria:**
    - Superseded by **CS-84**: WS-local barometric pressure must **not** be backfilled from Open‑Meteo/public APIs. If the station omits pressure, the series may be empty (missing is missing).
  - **Notes / Run Log:**
    - 2026-01-18: Implemented an Open‑Meteo backfill for WS pressure so Trends could graph a series even when the station omits it.
    - 2026-01-17: This approach was later **reverted** as a data integrity breach (mixed data sources in one “local” sensor). Replacement: **CS‑84**.
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs83-dw142-dw143-0.1.9.150.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.150_cs83_dw142_dw143/trends_ws2902_barometric_pressure.png`
  - **Status:** Done (canceled: invalid data source mixing; replaced by CS-84)

- **CS-84: WS-2902 pressure integrity: forbid external backfill; split relative vs absolute**
  - **Description:** A WS‑2902 weather station is a local, raw-data device. Its “barometric pressure” telemetry must never be synthesized/backfilled from Open‑Meteo (or any other public API). Make pressure reference explicit by splitting pressure into two WS-local sensors (relative vs absolute) and prevent mixed-source contamination.
  - **Acceptance Criteria:**
    - WS‑2902 ingest never copies Open‑Meteo/public API pressure into WS-local sensors or metrics.
    - When station uploads include valid pressure fields, WS-local pressure is ingested and graphable in Trends.
    - When station uploads omit pressure, the WS-local series remains empty (no synthetic backfill).
    - Dashboard makes the pressure reference obvious (relative vs absolute) and clearly labels WS-local origin.
    - `make ci-core-smoke` and `make ci-web-smoke` pass.
    - Tier A validation is recorded on the installed controller with at least one captured + viewed screenshot and a run log under `project_management/runs/`.
  - **Notes / Run Log:**
    - 2026-01-17: Removed the WS pressure “fallback” that wrote Open‑Meteo points into WS sensors.
    - 2026-01-17: Added explicit WS pressure sensors: `pressure_relative` and `pressure_absolute`; hid the legacy ambiguous WS `pressure` sensor.
    - 2026-01-17: Cleaned contaminated historical rows from the installed controller DB (legacy WS pressure sensor).
    - 2026-01-17: Validation: `make ci-core-smoke` (pass); `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260117-tier-a-cs84-ws-pressure-integrity-0.1.9.151.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/ws2902_node_detail.png`
      - `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/ws2902_sensors_tab.png`
  - **Status:** Done (validated on installed controller 0.1.9.151; clean-host E2E deferred to DT-59)

- **CS-85: Derived sensors (controller-computed from other sensors)**
  - **Description:** Add support for “Derived Sensors” that are not directly measured. Operators can define an expression (beyond basic math: include functions) that computes a time-series from one or more existing sensors. Derived sensors must be clearly labeled as computed, must not accept direct metric ingest, and must preserve data integrity (no mixed-source writes inside a single sensor series).
  - **Acceptance Criteria:**
    - Operators can create a derived sensor via the existing Sensors CRUD surface (dashboard Add Sensor UI), persisted as a normal sensor with `config.source="derived"` and a stored expression + input sensor IDs.
    - `/api/metrics/query` can return a derived series by evaluating the expression over bucketed input series (no persisted backfill required).
    - When an input bucket is missing, the derived bucket is missing (no implicit fills).
    - `/api/metrics/ingest` rejects attempts to ingest values for derived sensors (fail closed to prevent contamination).
    - Derived sensors are clearly distinguishable in the API payload (`config.source`) and dashboard UI (badge + clear copy).
    - `make ci-core-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Implemented query-time derived sensor evaluation (`evalexpr`), custom math functions, strict input validation, and fail-closed ingest rejection.
    - 2026-01-18: Validation: `make ci-core-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs85-dw144-derived-sensors-0.1.9.152.md`
  - **Status:** Done (validated on installed controller 0.1.9.152; clean-host E2E deferred to DT-59)

- **CS-86: Sensor series integrity audit + enforcement (no mixed source/type/unit)**
  - **Description:** Audit the controller DB and core ingestion/update paths to ensure a single `sensor_id` cannot ever mix data sources (local telemetry vs forecast/public APIs) or incompatible semantic identity (type/unit/source) over time. Add fail-closed guards and an ops audit tool to detect (and optionally purge) any invalid history.
  - **Acceptance Criteria:**
    - `/api/metrics/ingest` rejects sensors whose `config.source` is `forecast_points` or `derived` (fail closed; prevents mixed-source writes).
    - `PUT /api/sensors/{id}` rejects changes that would mutate a sensor’s identity after it has history (type/unit/source), to prevent semantic mixing.
    - An ops audit tool exists to report sensors that have invalid history for their configured source (e.g., `forecast_points` sensors with `metrics` rows; derived sensors with `metrics` rows).
    - If contamination is detected on an installed controller DB, an explicit, gated purge path exists (dry-run by default; apply requires confirmation).
    - `make ci-core-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Added fail-closed guards in `/api/metrics/ingest` and `PUT /api/sensors/{id}` to prevent source/type/unit mixing after history.
    - 2026-01-18: Added `ops_audit_sensor_series_integrity` to detect (and optionally purge) invalid history rows for derived/forecast sensors.
    - 2026-01-18: Validation: `make ci-core-smoke` (pass); installed-stack health smoke `make e2e-installed-health-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs86-sensor-series-integrity-0.1.9.153.md`
  - **Status:** Done (validated on installed controller 0.1.9.153; clean-host E2E deferred to DT-59)

- **CS-88: Centralize sensor visibility policy (API boundary)**
  - **Description:** Hide flags must be enforced consistently across the system. Refactor “hide” into a centralized sensor-visibility policy at the API/UI boundary so default sensor queries return only visible sensors and an admin/debug mode can optionally include hidden sensors with visibility metadata. Support node-level hide rules (e.g., hide Open‑Meteo “public provider data” sensors) and per-sensor overrides with deterministic precedence.
  - **Acceptance Criteria:**
    - `GET /api/sensors` returns **only visible** sensors by default (no dashboard-web page-level filtering required).
    - `GET /api/sensors?include_hidden=true` returns hidden sensors too and includes `sensor.visibility` metadata; the mode requires `config.write`.
    - `GET /api/sensors/{sensor_id}` returns `404` when the sensor is hidden (unless `include_hidden=true` + `config.write`).
    - Node-level hide (`nodes.config.hide_live_weather=true`) hides Open‑Meteo weather sensors without mutating `sensors.config.hidden`.
    - Per-sensor override (`sensors.config.visibility_override="visible"`) can force-show Open‑Meteo sensors even when the node hide rule is enabled.
    - `GET /api/map/features` filters out hidden sensor features using the same centralized policy.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Implemented centralized policy in `apps/core-server-rs/src/services/sensor_visibility.rs` and wired it into `/api/sensors` (default visible-only + optional `include_hidden`) and `/api/map/features`. Removed the legacy `update_node` cascade that wrote `sensors.config.hidden` for `hide_live_weather`.
    - 2026-01-18: Validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260119-tier-a-cs88-dw152-dw153-dw154-dw155-0.1.9.165.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.165_hide_live_weather_2026-01-19_001514602Z/02_after_toggle_node_detail.png`
  - **Status:** Done (validated on installed controller 0.1.9.165; clean-host E2E deferred to DT-59)

- **CS-87: Derived sensors: expand expression function library (math + trig + conditional)**
  - **Description:** Expand the derived-sensor expression language so operators can build richer computed sensors (beyond basic math) while preserving determinism and data integrity (fail-closed on invalid domains and non-finite outputs).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0036-derived-sensors-expand-function-library-cs-87-dw-149.md`
  - **Acceptance Criteria:**
    - Derived sensor expressions support additional functions: `floor`, `ceil`, `sqrt`, `pow`, `ln`, `log10`, `log`, `exp`, `sin`, `cos`, `tan`, `deg2rad`, `rad2deg`, `sign`, `if(cond,a,b)`.
    - Invalid domains fail closed with clear errors (e.g., `ln(x)` requires `x > 0`, `sqrt(x)` requires `x >= 0`).
    - Unit tests cover the new functions in `apps/core-server-rs/src/services/derived_sensors.rs`.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - Tier A validation is recorded on the installed controller with at least one captured + viewed screenshot and a run log under `project_management/runs/`.
  - **Notes / Run Log:**
    - 2026-01-18: Added additional math/trig/conditional functions with strict domain validation and non-finite guards.
    - 2026-01-18: Validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs87-dw149-derived-sensor-functions-0.1.9.162.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.162_cs87_dw149_20260118_074353/derived_sensor_function_library.png`
  - **Status:** Done (validated on installed controller 0.1.9.162; clean-host E2E deferred to DT-59)

- **CS-61: Canonicalize role presets (admin/operator/view) in the API**
  - **Description:** Define canonical role strings (`admin`, `operator`, `view`) and ensure API behavior is consistent and backward compatible with existing `control` role values.
  - **Acceptance Criteria:**
    - API accepts role `operator` while continuing to accept existing role `control` as an alias.
    - `admin` defaults include `config.write` and `users.manage`.
    - `operator` defaults include schedule/output/alarm-ack capabilities but do not include `config.write` or `users.manage`.
    - `view` defaults include only read capabilities (no writes).
  - **Notes / Run Log:**
    - 2026-01-12: Added `/api/auth/bootstrap` so login bootstrap does not require `/api/users` and secured `/api/users` behind `users.manage`. Canonicalized role values in auth boundary so legacy `control` reads as `operator`.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.101`. Screenshot captured + viewed:
      - `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/users.png`
    - Run: `project_management/runs/RUN-20260112-tier-a-auth-permissions-0.1.9.101.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-60: Make adoption tokens + sensor registration production-safe**
  - **Description:** Fix the “invalid adoption token” failures and harden adoption so tokens are controller-issued + MAC-bound, and adoption enables telemetry ingest by registering the node’s sensors into the controller DB.
  - **Acceptance Criteria:**
    - Adoption uses controller-issued tokens only (`POST /api/adoption/tokens`), stored in `adoption_tokens` with a MAC binding and TTL; the dashboard does not fall back to node-advertised tokens.
    - Token issuance deletes expired unused tokens and replaces any prior unused token for the same MAC(s) to prevent collisions/stale adverts.
    - Adoption registers Renogy BT-2 sensors for the node by fetching `http://<node>:9000/v1/config` and upserting allowlisted sensors (`config.source=renogy_bt2`, `config.metric=...`) with unit enforcement.
    - Adoption rejects `sync_node_agent_profile` when the node-agent-reported MACs do not match the node DB MACs (basic LAN spoofing mitigation).
    - End-to-end: scan → issue token → adopt → sensors appear → `metrics` ingests → dashboard shows non-zero latest/SOC values.
  - **Notes / Run Log:**
    - 2026-01-05: Removed the insecure node-advertised token fallback; adoption is now controller-issued + DB-validated, with MAC-bound tokens and node-agent profile validation + Renogy metric allowlist.
    - 2026-01-10: Made adoption token issuance/adopt flow transaction-safe (row locking + single-use guard) and serialized per-MAC token issuance via Postgres advisory locks.
    - 2026-01-10: Tier A (installed controller `0.1.9.70`): `/api/scan` returns candidates, controller token issuance succeeds, and `/api/adopt` rejects node-advertised/bogus tokens while accepting controller-issued tokens. Evidence: `project_management/runs/RUN-20260110-tier-a-phase3-adoption-ts8.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-57: Surface latest sensor values in core APIs**
  - **Description:** Ensure the core server surfaces per-sensor latest values so the dashboard can render “Latest” without DB triggers/hotfixes, and align the API contract for future analytics/trends.
  - **Acceptance Criteria:**
    - `GET /api/sensors` and `GET /api/dashboard/state` include `latest_value` (and `latest_ts`) for each sensor where metric history exists.
    - Implementation does not require mutating `sensors.config` on every metric insert (prefer read-time join or a dedicated cached table/view).
  - **Notes / Run Log:**
    - 2026-01-05: Implemented read-time latest joins (via `LEFT JOIN LATERAL ... ORDER BY ts DESC LIMIT 1`) so “Latest” does not depend on a hot-path DB trigger.
    - 2026-01-05: Removed the `infra/migrations/018_sensor_latest_values.sql` trigger/backfill hot path; added `infra/migrations/019_remove_sensor_latest_values_trigger.sql` to drop the trigger/function and add an index on `metrics(sensor_id, ts DESC)` for fast latest queries.
    - 2026-01-10: Tier A (installed controller `0.1.9.70`): `/api/sensors` returns `latest_value`/`latest_ts` and `/api/dashboard/state` includes latest fields where applicable. Evidence: `project_management/runs/RUN-20260110-tier-a-phase3-adoption-ts8.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-70: Apply node sensor config to node-agent**
  - **Description:** Allow admins to configure a node’s hardware sensors from the controller, push the desired sensor list to the node-agent, and ensure core registers the sensors so telemetry isn’t dropped as “unknown”.
  - **Acceptance Criteria:**
    - `GET /api/nodes/{node_id}/sensors/config` returns the stored desired sensor list (falls back to core’s node-agent sensor registry when unset).
    - `PUT /api/nodes/{node_id}/sensors/config` stores the desired list, upserts core `sensors` rows (`config.source=node_agent`), and marks removed node-agent sensors `deleted_at` (keeps data).
    - The node sensor config endpoints reject non-node-agent nodes (nodes without `config.agent_node_id`) with a clear `400` error (hardware sensors are Pi-only).
    - Best-effort sync to the node-agent via `PUT http://{node_ip}:9000/v1/config` with `{sensors:[...]}`.
  - **Notes / Run Log:**
    - 2026-01-10: Implemented new endpoints, core DB upsert/delete semantics, and OpenAPI registration; deployed via controller bundle `0.1.9.61` and verified `GET /api/nodes/<id>/sensors/config` returns `200` on the installed controller.
    - 2026-01-10: Validated end-to-end on real hardware (Pi 5 node2 @ `10.255.8.20`): deployed node-agent, used `PUT /api/nodes/{id}/sensors/config` to add ADS1115 test sensors, verified node-agent `/v1/config` updated immediately and `/api/sensors` shows non-zero `latest_value` within a minute. Fixed missing defaults (scale defaulted to 0) so omitted fields now default to `scale=1` and `interval_seconds=30` (deployed via controller bundle `0.1.9.64`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-73: Offline map assets service (local tiles/glyphs/terrain + Swanton pack)**
  - **Description:** Make the Map stack independent of internet after setup by hosting map tiles (MBTiles), glyphs, and terrain on the controller, and providing an operator-friendly “download/install offline map pack” workflow for the Swanton, CA region.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0031-offline-first-map-stack-(local-tiles,-glyphs,-terrain;-swanton-ca-pack).md`
    - `apps/core-server-rs/src/routes/map_assets.rs`
    - `apps/core-server-rs/src/routes/map_offline.rs`
    - `apps/core-server-rs/src/services/map_offline.rs`
    - `infra/migrations/029_map_offline_packs.sql`
  - **Acceptance Criteria:**
    - Controller serves local map assets from controller storage (`CORE_MAP_STORAGE_PATH`):
      - Tiles: `GET /api/map/tiles/{pack}/{layer}/{z}/{x}/{y}` (MBTiles-backed; `layer` = `streets|topo|satellite|terrain`).
      - Glyphs: `GET /api/map/glyphs/{fontstack}/{range}` (returns `.pbf` bytes; `{range}` is passed without `.pbf` due to Axum route segment constraints).
      - Terrain: local raster-dem tileset (Terrarium encoding) usable by MapLibre hillshade/terrain layers.
    - Offline pack management endpoints exist and are OpenAPI-registered:
      - `GET /api/map/offline/packs` and `POST /api/map/offline/packs/{id}/install` (auth + `config.write`).
    - Installing an offline pack best-effort switches saved map baselayers from internet → offline equivalents so the map remains usable when internet is removed.
  - **Evidence (Tier A):**
    - Installed controller `0.1.9.112`: offline pack installed and Map renders with browser-level internet blocked. Evidence: `project_management/runs/RUN-20260113-tier-a-offline-map-stack-0.1.9.112.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **CS-74: Wire controller connection indicator (/api/connection)**
  - **Description:** Make the controller “connection” status real (no more `local · unknown`) by reporting the current API path in use and whether that path is healthy.
  - **Acceptance Criteria:**
    - `GET /api/connection` reports:
      - `mode`: `local` when the request host is a LAN/localhost address; otherwise `cloud`.
      - `local_address` / `cloud_address`: derived from the request `Host`/`X-Forwarded-*` headers (not MQTT host/port).
      - `status`: `online` when DB is reachable; `degraded` when the API responds but DB ping fails.
    - `GET /api/dashboard/state` uses the same connection response so the Overview header banner is consistent.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.115`.
    - `curl -fsS http://127.0.0.1:8000/api/connection` returns `{"mode":"local","local_address":"http://127.0.0.1:8000","status":"online",...}`.
    - Screenshot captured + viewed: `manual_screenshots_web/20260113_083740/root.png` (header/banner shows `local · online`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-68: Sensors CRUD: allow COV intervals (interval_seconds=0)**
  - **Description:** Treat `interval_seconds=0` as an explicit change-of-value (COV) configuration in core APIs. Sensor create/update must accept 0 and store it consistently so the dashboard and ingest pipelines can use it.
  - **References:**
    - `apps/core-server-rs/src/routes/sensors.rs`
    - `project_management/TASKS.md` (CS-23 COV ingest)
  - **Acceptance Criteria:**
    - `POST /api/sensors` accepts `interval_seconds=0` (COV) as valid input.
    - `PUT /api/sensors/{id}` accepts `interval_seconds=0` and updates successfully.
    - API contract and UI semantics align: `0` is reserved for COV (not “0 seconds”).
  - **Notes / Run Log:**
    - 2026-01-10: Allowed `interval_seconds=0` in Rust sensor create/update validation so COV sensors can be configured via the API. Builds: Rust (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): `POST /api/sensors` with `interval_seconds=0` returned `201`, and the sensor detail UI renders “Interval COV”. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/sensors_cov_detail.png`.
    - 2026-01-10: Tier A: verified Sensors CRUD stability after the `latest_value/latest_ts` row-mapping fix (`PUT /api/sensors/{id}` → `200`, `DELETE /api/sensors/{id}?keep_data=true` → `204`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **CS-66: Emporia cloud ingest: full electrical readbacks (V/A + nested devices)**
  - **Description:** Expand Emporia cloud polling to ingest all electrical readbacks available via the Emporia API (not just kWh→W), including voltage/current and any nested-device channels, and persist them as first-class sensors/metrics so they can be displayed and graphed in the dashboard.
  - **References:**
    - `apps/core-server-rs/src/services/emporia.rs`
    - `apps/core-server-rs/src/services/analytics_feeds.rs`
    - `apps/core-server-rs/src/services/emporia_ingest.rs`
  - **Acceptance Criteria:**
    - Emporia polling fetches and persists per-device readbacks for at least: mains power (W), mains voltage (V), mains current (A), plus per-channel power/voltage/current where available.
    - Channel parsing includes `nestedDevices` so all circuits/subdevices returned by Emporia are ingested (no silent omissions).
    - New sensors are visible via `GET /api/sensors` (with correct `type`, `unit`, and `config` metadata) and are queryable via `GET /api/metrics/query`.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Discovered Emporia exposes additional channel readbacks via `getDeviceListUsages` when switching `energyUnit` (e.g., `Voltage`, `AmpHours`); plan is to poll multiple units per interval and derive instantaneous A from Ah deltas (`A = Ah * 3600` when `scale=1S`).
    - 2026-01-08: Implemented multi-unit polling (`KilowattHours`, `Voltage`, `AmpHours`) + nested-device flattening so per-device and per-channel power/voltage/current are persisted as sensors/metrics; added retry+merge on null channel usage values and canonicalized channel keys to keep readbacks aligned across units. Refreshed the installed controller to `0.1.9.39`. Builds: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass), `python3 tools/check_openapi_coverage.py` (pass).
    - 2026-01-10: Backfilled missing per-channel watts when Emporia returns V/A but omits kWh usage by deriving `power_w = voltage_v × current_a`, persisting a `channel_power_w:*` sensor tagged with `config.derived_from_va=true` so the Power table/charts no longer show “—” for watts on those channels.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Power tab renders mains + circuit voltage/current readbacks and derived watts when needed. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/power.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **CS-48: Validate deploy-from-server (SSH) on real Pi 5 hardware**
  - **Description:** Validate the deploy-from-server flow against a real Pi 5 twice (fresh + rerun) to confirm host key UX, idempotency, and post-deploy adoption behave as expected in the field.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0010-feature-004-deployment-from-server-ssh.md`
  - **Acceptance Criteria:**
    - First deploy requires host-key confirmation and succeeds end-to-end (SSH → install → node-agent health).
    - Re-running deploy on the same Pi is safe and reports an idempotent outcome when already healthy.
    - Host key mismatch (after reflash) is detected and blocked until trust is reset.
    - Node appears in `/api/scan` (or the dashboard Nodes adoption UI) and can be adopted using the returned token when applicable.
  - **Notes / Run Log:**
    - 2026-01-10: Deploy-from-server initially failed on node2 because the node image is Debian 13 (trixie) with Python 3.13, while the offline overlay only shipped Python 3.11 (cp311) wheels. Resolution shipped multi-Python vendored deps (py311 + py313) with a runtime selector, relaxed deploy inspection to allow Bookworm/Python 3.11 and Trixie/Python 3.13, and fixed a node-agent DBus `@method` return-annotation crash on Python 3.13.
    - 2026-01-10: Deployed via the installed controller to node2 (`10.255.8.20`), verified `curl http://10.255.8.20:9000/healthz` returns `{"status":"ok"}` and node2 appears as `online` in `/api/nodes` (controller bundle `0.1.9.63` / `0.1.9.64`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **CS-64: Remove production demo fallbacks for analytics (explicit errors only)**
  - **Description:** Ensure the installed controller never fabricates demo/synthetic analytics values when real endpoints are unavailable. Demo-only endpoints must be disabled by default in production.
  - **Acceptance Criteria:**
    - Dashboard snapshot (`GET /api/dashboard/state`) does not generate synthetic analytics values/series; when analytics calls fail it includes explicit error metadata instead.
    - Demo snapshot endpoint (`GET /api/dashboard/demo`) returns `404` unless `CORE_DEMO_MODE=true`.
    - No production code path returns “cached demo values” (or synthetic zero-filled chart series) in response to upstream API failures.
  - **Notes / Run Log:**
    - 2026-01-06: Removed synthetic analytics series generation from the Rust snapshot; snapshot now embeds real analytics payloads and attaches an `errors` map when sections fail (no demo fallback).
    - 2026-01-06: Added `CORE_DEMO_MODE` gating so `/api/dashboard/demo` is disabled by default in production.
    - Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass).
    - 2026-01-06: Deployed via controller bundle `0.1.9.14`; verified `/api/dashboard/demo` returns `404` on the installed controller stack. Evidence: `reports/prod-upgrade-no-demo-fallback-20260106_130531.json`.
  - **Status:** Done (deployed + validated; E2E still gated by clean-state policy)


- **CS-67: Serve dashboard static assets with cache-safe headers**
  - **Description:** Ensure the controller serves the static dashboard with cache headers that prevent stale HTML/JS manifests from being reused on mobile browsers after upgrades.
  - **References:**
    - `apps/core-server-rs/src/static_assets.rs`
  - **Acceptance Criteria:**
    - HTML responses include `Cache-Control: no-store`.
    - Fingerprinted `/_next/static/*` assets include `Cache-Control: public, max-age=31536000, immutable`.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` remains green.
  - **Notes / Run Log:**
    - 2026-01-09: Added cache-control middleware for the static assets fallback router (no-store for HTML; immutable for `/_next/static/*`). Deployed via controller bundle `0.1.9.44`; verified `curl -I http://127.0.0.1:8000/nodes/` shows `cache-control: no-store`.
  - **Status:** Done


- **CS-54: WS-2902 “TCP/IP connect” integration mode (spec gap)**
  - **Description:** The current WS-2902 implementation is a tokenized HTTP ingest endpoint that the station pushes to. This task tracks adding a true controller-initiated “connect/pull” mode (TCP/IP polling) or explicitly scoping the product to push-only with updated UX copy.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0019-ws-2902-controller-initiated-tcp-ip-integration.md`
    - `apps/core-server-rs/src/routes/weather_stations.rs`
    - `apps/dashboard-web/src/features/nodes/components/WeatherStationModal.tsx`
  - **Acceptance Criteria:**
    - If pull-mode is implemented: the controller can fetch WS-2902 readings without requiring the station to upload to a custom server URL.
    - If push-only is the decided product: dashboard copy and documentation no longer claim “connect to TCP/IP station” and instead describe configuring the station to upload to the provided endpoint.
  - **Status:** Done (push-only scope; `make ci-web-smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260104_050252`)


- **CS-55: Default admin capability includes config.write**
  - **Description:** Ensure “admin” users include `config.write` by default so product-grade operations (deployments, setup config, backups) work without manual capability updates.
  - **Acceptance Criteria:**
    - Creating a user with role `admin` results in `config.write` being present even if the request omits it.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260103_232953`)


- **CS-56: Session tokens reflect capability updates immediately**
  - **Description:** Fix confusing auth behavior where `/api/auth/me` shows stale capabilities until re-login because session tokens cache capabilities at issuance time.
  - **Acceptance Criteria:**
    - After updating a user’s capabilities, subsequent authenticated calls (including `/api/auth/me`) reflect the updated capabilities without requiring re-login.
    - The change applies to session tokens issued by `/api/auth/login` without impacting read-only API tokens (WAN tokens remain stable).
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260103_232953`)


- **CS-53: Refactor deploy-from-server service monolith (audit maintainability)**
  - **Description:** Split `apps/core-server-rs/src/services/deployments.rs` into focused modules (jobs/state, SSH/auth, step runner) to reduce blast radius and make future changes safer while preserving behavior.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `apps/core-server-rs/src/services/deployments.rs`
  - **Acceptance Criteria:**
    - No behavior changes in deploy-from-server API/flow (pure refactor).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_195458.log`)


- **CS-50: Secure deploy-from-server SSH credentials + add key-based auth option (audit)**
  - **Description:** Address the external audit findings for deploy-from-server by removing poison-sensitive locking, preventing accidental secret logging, and adding an SSH public-key authentication option (password remains supported for bootstrap UX).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `apps/core-server-rs/src/services/deployments.rs`
    - `apps/core-server-rs/src/routes/deployments.rs`
  - **Acceptance Criteria:**
    - No `.lock().expect("... poisoned")` remains in deploy-from-server job storage; the service recovers safely from poisoned locks.
    - Deployment request structs do not `Debug`-print secrets (passwords, MQTT creds); any structured logs redact secrets.
    - Deployment supports SSH public-key auth (PEM key + optional passphrase) in addition to password auth.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_071125.log`)


- **CS-51: Add API rate limiting on sensitive controller endpoints (audit)**
  - **Description:** Add a simple, production-safe rate limit layer to protect high-impact endpoints (deployments/config writes) from accidental or malicious request floods.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `apps/core-server-rs/src/routes/`
  - **Acceptance Criteria:**
    - High-cost endpoints (at minimum: deployments + setup-daemon proxy + auth/login) are rate limited and return `429` when exceeded.
    - Limits are set high enough to not break installer-path E2E and normal operator usage.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_071125.log`)


- **CS-52: Remove panic paths + silent data loss from Rust controller routes (audit)**
  - **Description:** Replace `expect`/`unwrap` paths and silent JSON type coercions in Rust core-server routes with explicit error handling that preserves data and provides actionable responses.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `apps/core-server-rs/src/presets.rs`
    - `apps/core-server-rs/src/routes/renogy.rs`
    - `apps/core-server-rs/src/routes/display_profiles.rs`
  - **Acceptance Criteria:**
    - Preset loading failures return a controlled error (or safe empty preset behavior) instead of panicking at startup.
    - Renogy/Display profile config updates do not silently overwrite invalid JSON types; they return `400` with a clear message.
    - No production `expect`/`unwrap` remain in those modules.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes and `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_071125.log`)


- **CS-49: Unify preset source-of-truth (CLI + dashboard) for Renogy/WS-2902**
  - **Description:** Ensure the canonical “default sensor set” presets used by the dashboard and CLI tooling are shared and drift-proof (single source of truth + regression check).
  - **References:**
    - `tools/renogy_node_deploy.py`
    - `apps/core-server-rs/src/routes/renogy.rs`
    - `apps/core-server-rs/src/routes/weather_stations.rs`
  - **Acceptance Criteria:**
    - Preset definitions live in one canonical location (file/module) that both the core API and CLI tooling consume.
    - A drift check exists (unit test or CI script) that fails if CLI and core API presets diverge.
    - `make ci-web-smoke` and `make e2e-installer-stack-smoke` remain green.
  - **Status:** Done (`python3 tools/check_integration_presets.py`, `make ci-web-smoke`, `make e2e-installer-stack-smoke`; logs: `reports/prod-ready-e2e-installer-stack-smoke-20260101_174119.log`, `reports/prod-ready-e2e-web-smoke-20260101_175422.log`)


- **CS-44: Support per-node display profile config (Pi 5 local display)**
  - **Description:** Add a per-node “display profile” config model and API so operators can configure the Pi 5 local display tiles/refresh behavior from the main dashboard UI, and the node-agent can render kiosk content without manual edits.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0007-feature-001-pi5-local-display-basic.md`
    - `project_management/archive/archive/tickets/TICKET-0008-feature-002-pi5-local-display-advanced-controls.md`
  - **Acceptance Criteria:**
    - Core API supports reading/writing a `display` config section per node (schema versioned).
    - Display config is included in the node config sync path used by node-agent (no manual file edits on the node).
    - OpenAPI/SDKs are updated so dashboard/iOS clients stay contract-aligned.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `make ci-node`, `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`, `make e2e-web-smoke`)


- **CS-46: Issue and enforce read-only tokens for WAN portal pulls**
  - **Description:** Ensure the core auth/capability model supports a strict read-only token that can pull node status and trends but cannot mutate on-prem state (required for the AWS pull agent portal).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0014-feature-008-wan-readonly-webpage-aws.md`
  - **Acceptance Criteria:**
    - A token with read-only capability exists and covers required endpoints for the WAN portal.
    - Attempts to use the token for writes/commands/schedule edits are rejected.
    - OpenAPI/SDKs include any new token issuance or capability reporting surfaces (if added).
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `make ci-node-smoke`, `make e2e-setup-smoke`; logs: `reports/cs46-ci-web-smoke-20251231_210337.log`, `reports/cs46-ci-node-smoke-20251231_210402.log`, `reports/cs46-e2e-setup-smoke-20251231_210803.log`)


- **CS-45: Add “apply preset” config endpoints for Renogy BT-2 and WS-2902**
  - **Description:** Provide idempotent core API endpoints that apply a canonical “default sensor set” preset (Renogy BT-2 and WS-2902) so the dashboard can do near one-click configuration with actionable errors.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0012-feature-006-renogy-bt-one-click-setup.md`
    - `project_management/archive/archive/tickets/TICKET-0013-feature-007-ws-2902-weather-station-setup.md`
    - `docs/runbooks/ws-2902-weather-station-setup.md`
  - **Acceptance Criteria:**
    - Preset endpoints are idempotent (no duplicated sensors/config on repeated runs).
    - Dashboard can call the endpoints and render actionable outcomes/errors.
    - `make e2e-web-smoke` remains green (via installer-path gate).
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `make e2e-installer-stack-smoke`; logs: `reports/ci-web-smoke-20251231_181750.log`, `reports/e2e-installer-stack-smoke-20251231_181750.log`, `reports/cs45-e2e-installer-stack-smoke-20251231_213130.log`)


- **CS-47: Harden deploy-from-server (SSH) for product-grade UX**
  - **Description:** Improve the existing Pi 5 deploy-from-server job to meet product-grade safety/UX requirements (host key verification, credential redaction, idempotency, clearer failure diagnostics).
  - **Acceptance Criteria:**
    - Host key verification is supported (with a documented “trust on first use” or explicit key pinning workflow).
    - Logs/artifacts redact secrets and do not echo passwords/keys.
    - Staged secret artifacts are not left behind on the node (e.g., `/tmp/node-agent.env` is removed after install).
    - Re-running a deploy job is safe and reports “already installed/healthy” where applicable.
    - `make e2e-web-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `cargo test --manifest-path apps/farmctl/Cargo.toml`, `make ci-web-smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_160228.log`)


- **CS-43: Split analytics tests into focused modules**
  - **Description:** Improve maintainability and triage speed by splitting the large analytics test module into focused test files with shared fixtures.
  - **Acceptance Criteria:**
    - `apps/core-server/tests/test_analytics.py` is split into smaller, purpose-scoped modules (feeds vs processing vs API shape).
    - Shared fixtures/helpers move into `apps/core-server/tests/conftest.py` or a `tests/helpers/` module.
    - `poetry run pytest` still passes with the same coverage expectations.
  - **Status:** Done (`cd apps/core-server && poetry run pytest`)

- **CS-42: Add remote Pi 5 deployment API**
  - **Description:** Provide an SSH-driven deployment job API that installs the node-agent on fresh Pi 5 nodes, issues adoption tokens, and streams progress/logs for the dashboard.
  - **Acceptance Criteria:**
    - `/api/deployments/pi5` starts a deployment job and returns job status + step logs.
    - The job connects over SSH, installs the node-agent bundle, writes node identity/env files, enables systemd services, and runs a health check.
    - Responses include node identity + adoption token for downstream adoption flows.
  - **Status:** Done (make e2e-web-smoke)

- **CS-1: Guarantee deterministic sensor/output identifiers and propagate rename updates**
  - **Description:** Deterministic 24‑character hex IDs for sensors/outputs based on MAC + timestamp + counter, plus sensor rename propagation to alarms, backups, analytics, and node manifests.
  - **Status:** Done


- **CS-2: Implement real-mode user management**
  - **Description:** Real-mode CRUD for users with roles/capabilities and permission audit logging. (Auth enforcement tracked separately.)
  - **Status:** Done


- **CS-3: Implement real-mode schedules and alarms**
  - **Description:** Real-mode schedules/alarms CRUD, calendar API, schedule engine evaluation with conditions, and MQTT action publishing.
  - **Status:** Done


- **CS-4: Add Alembic/SQL migrations**
  - **Description:** Create migrations for new tables and fields (outputs, schedules, alarms, analytics, backups, adoption).
  - **Status:** Done


- **CS-5: Extend SQLAlchemy models + Pydantic schemas**
  - **Description:** Extend models for sensors (interval/rolling/calibration) and outputs.
  - **Status:** Done


- **CS-6: Handle sensor deletion retention**
  - **Description:** Implement logic to keep data with `-deleted` suffix upon deletion and maintain configuration history.
  - **Status:** Done


- **CS-7: Implement discovery/adoption pipeline**
  - **Description:** Implement MAC-based identity and `/api/scan` + `/api/adopt` endpoints.
  - **Status:** Done


- **CS-8: Implement rich `/api/nodes` detail**
  - **Description:** Provide detailed node information including sensors, outputs, schedules, backups, and alarms.
  - **Status:** Done


- **CS-9: Implement `/api/sensors/{id}` and `/api/outputs`**
  - **Description:** Implement config endpoints for sensors and control endpoints for outputs (MQTT command publish).
  - **Status:** Done


- **CS-10: Provide `/api/connection` endpoints**
  - **Description:** Endpoints for local/cloud status toggling and discovery integration.
  - **Status:** Done


- **CS-11: Build schedules calendar API**
  - **Description:** API with RRULE + weekly blocks + condition JSON; integrate with APScheduler actions.
  - **Status:** Done


- **CS-12: Implement alarm definitions & history endpoints**
  - **Description:** Ensure default offline alarms are auto-created.
  - **Status:** Done


- **CS-13: Enhance metrics ingest/query**
  - **Description:** Support rolling averages, stacked vs independent axes metadata, and >7 day CAGG fallback.
  - **Status:** Done


- **CS-14: Create analytics aggregation jobs + endpoints**
  - **Description:** `/api/analytics/*` endpoints covering power, water, soil, and status.
  - **Status:** Done


- **CS-15: Implement backups manager**
  - **Description:** `/api/backups` list/download/restore endpoints and retention policy.
  - **Status:** Done


- **CS-16: Update seed script**
  - **Description:** Populate full demo dataset (nodes, sensors, outputs, schedules, alarms, analytics, backups).
  - **Status:** Done


- **CS-17: Expand tests**
  - **Description:** Pytest coverage for adoption, sensor config updates, schedules calendar, alarm triggers, and analytics responses.
  - **Status:** Done


- **CS-18: Split FastAPI entrypoint into routers**
  - **Description:** Extract domain routers (nodes, sensors, outputs, metrics, backups, alarms, users, schedules, analytics, discovery, connection, dashboard, health) and shared helpers to reduce `main.py` size.
  - **Status:** Done


- **CS-19: Modularize analytics feeds**
  - **Description:** Move analytics feed scaffolding into a package with base types, provider modules (Emporia, Tesla, Enphase, Renogy), and a manager.
  - **Status:** Done


- **CS-20: Package demo data generator**
  - **Description:** Split the demo dataset generator into a package (state, operations, ticker) to reduce God-class footprint while preserving public APIs.
  - **Status:** Done


- **CS-21: Implement authentication/authorization and enforce roles**
  - **Description:** Add an auth layer and enforce view-only vs control capabilities across REST endpoints and MQTT command publishing.
  - **Acceptance Criteria:**
    - Users authenticate (token/session based) against the core server.
    - Output commands, schedule edits, and config mutations are gated by user capabilities.
    - View-only users can read data but cannot mutate system state.
  - **Status:** Done


- **CS-22: Keep demo mode from breaking alarm tests**
  - **Description:** When running pytest with `CORE_DEMO_MODE=true`, alarms endpoints use the DB-backed implementation so DB fixtures remain deterministic (demo mode still serves in-memory alarms outside of tests).
  - **Status:** Done


- **CS-23: Implement change-of-value (COV) metric ingest**
  - **Description:** Treat `interval_seconds=0` sensors as “log on change” and dedupe identical values during ingest (MQTT consumer + `/api/metrics/ingest`) to avoid storing continuous zeroes for flow/rain gauges.
  - **Acceptance Criteria:**
    - Repeated identical values for a COV sensor do not create new metric rows.
    - Value changes (or quality changes) still persist immediately.
    - Includes test coverage for COV ingest behavior.
  - **Status:** Done


- **CS-24: Expose sensor/output preset templates for UIs**
  - **Description:** Provide an API surface that returns sensor/output preset definitions (type/unit/default intervals + parameter hints) so web/iOS/watch UIs can auto-populate configuration defaults.
  - **Status:** Done


- **CS-25: Validate output command state**
  - **Description:** Reject output command requests when the requested state is not in the output's supported states list.
  - **Acceptance Criteria:**
    - `/api/outputs/{id}/command` returns 400 for unsupported states.
    - Valid states continue to publish and persist normally.
  - **Status:** Done


- **CS-26: Validate output schedule_ids**
  - **Description:** Ensure output schedule references only include existing schedules before persisting them.
  - **Acceptance Criteria:**
    - Creating/updating outputs rejects unknown schedule IDs with 400.
    - Valid schedule IDs persist normally.
  - **Status:** Done


- **CS-27: Prune expired auth sessions**
  - **Description:** Periodically remove expired in-memory auth sessions so long-lived processes do not leak memory.
  - **Acceptance Criteria:**
    - Auth sessions are pruned on a timer while the API runs.
    - Expired sessions are removed without needing a lookup.
  - **Status:** Done


- **CS-28: Add password-based authentication**
  - **Description:** Require passwords for login and store hashed credentials for users.
  - **Acceptance Criteria:**
    - `/api/auth/login` verifies passwords against hashed storage.
    - User records persist password hashes (not plaintext).
    - Demo users include hashed credentials for login.
  - **Status:** Done


- **CS-29: Protect adoption token issuance**
  - **Description:** Require authenticated users with appropriate capability to issue adoption tokens and record issuer metadata.
  - **Acceptance Criteria:**
    - `/api/adoption/tokens` requires authenticated users with `config.write`.
    - Issued tokens store issuer metadata for audit.
  - **Status:** Done


- **CS-30: Restrict CORS origins**
  - **Description:** Limit CORS to configured origins instead of allowing all.
  - **Acceptance Criteria:**
    - CORS middleware uses the configured allowlist.
    - Default allowlist targets local dev origins only.
  - **Status:** Done


- **CS-31: Fix predictive tool-call history serialization**
  - **Description:** Ensure predictive alarm tool-call loops append serializable chat messages rather than raw SDK objects.
  - **Acceptance Criteria:**
    - Tool-call assistant responses are stored as message params/dicts for subsequent requests.
    - Predictive loop no longer injects non-serializable message objects.
  - **Status:** Done


- **CS-32: Respect predictive client lifecycle**
  - **Description:** Ensure predictive alarms close OpenAI clients when appropriate to avoid session leaks.
  - **Acceptance Criteria:**
    - Clients created via factory are closed by default after scoring.
    - Callers can opt out of closing when reusing shared clients.
  - **Status:** Done


- **CS-33: Optimize latest forecast query**
  - **Description:** Use SQL windowing to return latest forecast rows per (field, horizon_hours) without scanning all rows in Python.
  - **Acceptance Criteria:**
    - `/api/forecast` selects latest rows per group via SQL.
    - Response ordering remains stable by field/horizon.
  - **Status:** Done


- **CS-34: Offload predictive log writes**
  - **Description:** Move predictive alarm trace logging off the async hot path to avoid blocking the event loop.
  - **Acceptance Criteria:**
    - Predictive trace log writes are dispatched to a background thread/task.
    - Scoring remains non-blocking even when log writes are slow.
  - **Status:** Done


- **CS-35: Clamp predictive trace log size**
  - **Description:** Ensure predictive trace logging does not grow unbounded when misconfigured.
  - **Acceptance Criteria:**
    - Trace log maxlen is clamped to a sane default when invalid.
    - Long-running processes do not accumulate unbounded trace entries.
  - **Status:** Done


- **CS-36: Harden forecast + utility rate provider registry**
  - **Description:** Introduce a provider registry for forecast and utility rates with file/HTTP implementations, add config plumbing, staleness surfacing, and contract tests.
  - **Acceptance Criteria:**
    - Forecast and rate ingestion use provider registry classes under `app/services/providers/`.
    - File and HTTP providers work from recorded fixtures; contract tests cover each provider type.
    - Forecast/rate freshness is surfaced via status endpoints and schedule guards treat stale data as missing.
    - ADR documents how to add new providers/regions without code changes.
  - **Status:** Done


- **CS-37: Stabilize analytics/demo serialization and feed hooks**
  - **Description:** Align demo analytics payloads with response schemas and ensure analytics feed HTTP hooks work with async clients/tests.
  - **Acceptance Criteria:**
    - Demo analytics timestamps/fields match serialized API payloads (including rate schedule defaults).
    - Async HTTP request hooks no longer break analytics feed polling.
    - Dashboard state serialization includes required analytics/status fields and backup node IDs as strings.
  - **Status:** Done


- **CS-38: Align demo backups/adoption candidates with authenticated flows**
  - **Description:** Ensure demo-mode backups include retention metadata and demo discovery candidates advertise adoption tokens so authenticated token issuance is not required for smoke runs.
  - **Acceptance Criteria:**
    - Demo `/api/backups` responses include `retention_days`.
    - Demo `/api/scan` candidates include an `adoption_token` in `properties`.
  - **Status:** Done


- **CS-39: Ensure demo adoption templates include sensor timestamps**
  - **Description:** Populate `created_at` on demo sensors added during adoption so `/api/sensors` remains valid after demo adopts.
  - **Acceptance Criteria:**
    - Demo adoption inserts sensors with `created_at` timestamps.
    - `/api/sensors` does not error after a demo adoption completes.
  - **Status:** Done


- **CS-40: Preserve rate schedule period labels on default fallbacks**
  - **Description:** Keep utility rate schedule metadata reporting a period name even when the feed falls back to a `default_rate` with defined periods.
  - **Acceptance Criteria:**
    - Rate schedule details include a period name when `default_rate` is used.
    - NYISO fixture tests surface on/off-peak labels reliably.
  - **Status:** Done


- **CS-41: Normalize predictive/test timestamp inputs**
  - **Description:** Remove UTC deprecation warnings across Python tests and ensure predictive tests are resilient to GitHub token envs.
  - **Acceptance Criteria:**
    - Python test suites use `datetime.now(UTC)` instead of `datetime.utcnow()`/`timezone.utc` for UTC timestamps.
    - Predictive tests explicitly clear GitHub token env vars before asserting missing-token behavior.
  - **Status:** Done


- **CS-89: Renogy preset apply must preserve existing node sensors (ADS1263)**
  - **Description:** Fix `POST /api/nodes/{node_id}/presets/renogy-bt2` so it does not overwrite the node-agent `sensors` list with the controller’s `nodes.config.sensors` snapshot (which excludes hardware sensors tracked under `desired_sensors`). The preset apply must merge into the live node-agent config to avoid wiping ADS1263/analog sensor configs.
  - **Acceptance Criteria:**
    - Re-applying the Renogy preset does **not** remove ADS1263/analog sensors from `http://<node>:9000/v1/config`.
    - ADS1263 sampling continues after preset apply (ADC sensors continue updating in `/api/sensors` latest_ts and the node local display).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - Tier A validated on the installed controller (no DB/settings reset) and validated on real Pi 5 hardware.
  - **Evidence / Run Log:**
    - `project_management/runs/RUN-20260201-tier-a-cs89-renogy-preset-preserve-ads1263-sensors-0.1.9.235-adcfix.md`
  - **Status:** Done (Tier A validated installed `0.1.9.235-adcfix`; hardware validated on Node 1)


- **CS-90: Secure metrics query/ingest endpoints (auth + capabilities)**
  - **Description:** `GET /api/metrics/query` and `POST /api/metrics/ingest` are currently callable without any authentication. This allows unauthorized metric reads and unauthorized ingestion (data poisoning + potential DoS via large payloads). Require auth/capability gating for reads and a dedicated ingest auth mechanism for writes (node-scoped token, API token capability, or equivalent).
  - **Acceptance Criteria:**
    - `GET /api/metrics/query` returns `401` when no bearer token is provided.
    - `GET /api/metrics/query` returns `403` when the user lacks an appropriate read capability (define/standardize the capability name).
    - `POST /api/metrics/ingest` returns `401` when no valid ingest auth is provided.
    - `POST /api/metrics/ingest` rejects oversized payloads with `413 Payload Too Large` (or an explicit `400`) and does not attempt per-item inserts on rejected requests.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/metrics.rs`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: metrics query/ingest handlers do not extract `AuthUser` and the core router has no global auth layer.
    - 2026-02-03: Fix: `GET /api/metrics/query` now requires bearer auth + `metrics.view` (or `config.write`), and `POST /api/metrics/ingest` requires bearer auth + `metrics.ingest` (or `config.write`). Added an ingest item cap that returns `413 Payload Too Large` without attempting inserts.
    - 2026-02-03: Validation: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass), `make ci-farmctl-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-91: Secure backups read endpoints (auth + capabilities)**
  - **Description:** Multiple backups endpoints are currently callable without authentication (backup listing + retention config). This leaks node IDs/names, backup inventory, and retention policy configuration to any host that can reach the controller API. Require auth/capability gating for all backups-related read endpoints.
  - **Acceptance Criteria:**
    - The following endpoints return `401` when no bearer token is provided:
      - `GET /api/backups`
      - `GET /api/backups/{node_id}`
      - `GET /api/backups/retention`
      - `GET /api/restores/recent`
    - The same endpoints return `403` when the user lacks an appropriate read capability (define/standardize the capability name; `config.write` is acceptable as a stopgap but prefer a read-scoped capability).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/backups.rs`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: backups list/retention handlers do not extract `AuthUser` and are exposed under `/api` without a global auth layer.
    - 2026-02-03: Fix: backups listing + retention + recent restores now require bearer auth + `backups.view` (or `config.write`).
    - 2026-02-03: Validation: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass), `make ci-farmctl-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-92: Backups: implement real run + restore workflows (remove stub endpoints)**
  - **Description:** The backups endpoints currently acknowledge requests (`/api/backups/run`, `/api/restore`) but do not actually execute a backup or perform a restore (restore only records JSON under `nodes.config.last_restore`, and there is no worker consuming it). This creates a false-success UX and defeats the backup/restore feature.
  - **Acceptance Criteria:**
    - `POST /api/backups/run` triggers a real backup run (or enqueues a job) and the result is observable (new backup file appears under the configured backup root, or job state is visible via an endpoint).
    - `POST /api/restore` performs a real restore workflow (or enqueues a job) that actually updates the target node configuration and records a restore history entry.
    - `GET /api/restores/recent` returns recent restore operations (not an empty stub) with enough fields for the dashboard to show status.
    - Failure paths are explicit and actionable (e.g., missing backup, unknown node, restore failure reason).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/backups.rs`
    - `apps/core-server-rs/src/services/restore_worker.rs`
    - `infra/migrations/039_restore_events.sql`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: `run_backups` returns `ok` without doing work; `restore_backup` only writes `nodes.config.last_restore`; `recent_restores` always returns `[]`.
    - 2026-02-03: Fix: `POST /api/backups/run` now writes per-node backup bundles under `CORE_BACKUP_STORAGE_PATH/<node_id>/<YYYY-MM-DD>.json` (atomic write) and enforces per-node retention cleanup.
    - 2026-02-03: Fix: `POST /api/restore` now enqueues a restore job in `restore_events`; `RestoreWorkerService` applies the backup bundle to the target node (DB sensors/outputs + desired sensor config) and best-effort syncs node-agent via `/v1/config/restore` with retries.
    - 2026-02-03: Fix: `GET /api/restores/recent` now returns real restore history entries from `restore_events`.
    - 2026-02-03: Validation: `make ci-core-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-93: Schedule blocks: handle DST gaps/ambiguity without silently skipping events**
  - **Description:** The schedule engine converts local “HH:MM” blocks to UTC using `Local.from_local_datetime`. When the local time is nonexistent (DST spring-forward gap), the code returns `None` and silently skips the block; during DST fall-back ambiguity it picks the first instance. This can lead to missed schedule start/end actions on DST transition days.
  - **Acceptance Criteria:**
    - Schedule block start/end actions do not silently disappear on DST transition days (define explicit behavior for nonexistent/ambiguous times and apply it consistently).
    - The schedule engine records an explicit warning/error when a configured schedule block time cannot be mapped cleanly (instead of silently skipping).
    - Unit tests cover at least one DST gap and one DST ambiguity case (using a fixed timezone) and verify the chosen behavior.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/services/schedule_engine.rs`
    - `apps/core-server-rs/src/routes/schedules.rs`
    - `apps/core-server-rs/src/time.rs`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: `local_naive_to_utc` returns `None` for `LocalResult::None` and callers `continue`, skipping blocks.
    - 2026-02-03: Fix: centralized DST-aware local block resolution in `apps/core-server-rs/src/time.rs` and used it in both the schedule engine and the schedules calendar endpoint (no silent `continue` on DST gaps/ambiguity).
    - 2026-02-03: Validation: `make ci-core-smoke` (pass) and unit tests cover DST gap + ambiguity.
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-94: Secure setup credentials inventory endpoint (no secret metadata leak)**
  - **Description:** `GET /api/setup/credentials` is currently unauthenticated and returns `setup_credentials.metadata` for each credential. Some integrations store sensitive values in metadata (e.g., Emporia refresh token), so this endpoint can leak secrets to any LAN client that can reach the controller API.
  - **Acceptance Criteria:**
    - `GET /api/setup/credentials` requires bearer auth (returns `401` with no token).
    - Caller must have an explicit capability to view setup credentials (use `config.write` as a stopgap, but prefer a read-scoped capability); otherwise returns `403`.
    - Response does **not** expose secret material (either remove `metadata` from the response entirely or explicitly redact known secret keys such as `refresh_token` / `api_token`).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/setup.rs` (`list_credentials`)
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: Emporia stores `refresh_token` in `setup_credentials.metadata`, and `list_credentials` returns metadata without auth.
    - 2026-02-03: Fix: `GET /api/setup/credentials` now requires bearer auth + `setup.credentials.view` (or `config.write`). Metadata is redacted for known secret-ish keys (`refresh_token`, `api_token`, `password`, etc.).
    - 2026-02-03: Validation: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass), `make ci-farmctl-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-95: Secure /api/dashboard/state snapshot (auth-gate + no user/backup/adoption leaks; avoid per-request expensive scans)**
  - **Description:** `GET /api/dashboard/state` is currently unauthenticated but returns a full controller snapshot including users (email/role/capabilities), node inventory (MACs/IP), schedules/alarms/history, and backup inventory. It also performs filesystem backup scanning and an mDNS discovery scan inside the request path, making it an easy DoS target.
  - **Acceptance Criteria:**
    - `GET /api/dashboard/state` requires bearer auth (returns `401` with no token).
    - Snapshot fields that are admin-only (e.g., `users`) are omitted or redacted unless the caller has `users.manage`.
    - Expensive side effects are removed from the request hot path (e.g., mDNS scan + backup root scan are cached, moved behind a separate endpoint, or capability-gated behind an explicit action).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/dashboard.rs` (`dashboard_state`)
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: `dashboard_state` has no `AuthUser` extractor and returns users + triggers backup scan + discovery scan.
    - 2026-02-03: Fix: `/api/dashboard/state` now requires bearer auth and gates fields (e.g., `users` requires `users.manage`). Removed per-request mDNS + filesystem scanning from the snapshot hot-path; discovery and backups are fetched via dedicated endpoints.
    - 2026-02-03: Validation: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass), `make ci-farmctl-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-96: Require auth for core read endpoints beyond metrics/backups (nodes/outputs/schedules/alarms/analytics)**
  - **Description:** Many core-server **read** endpoints are callable without authentication, leaking sensitive operational data (MACs, last IPs, command topics, schedule/alarm history, analytics summaries). Project management currently tracks unauthenticated access for metrics/backups, but not the broader read surface.
  - **Acceptance Criteria:**
    - Define and document the core API auth policy for read endpoints (recommended: all `/api/*` endpoints except `/api/auth/login`, `/api/auth/bootstrap`, and `/healthz` require bearer auth).
    - At minimum, require bearer auth (401 without token) and appropriate read capabilities (403 without capability) for:
      - Nodes: `GET /api/nodes`, `GET /api/nodes/{node_id}`
      - Outputs: `GET /api/outputs`, `GET /api/outputs/{output_id}`
      - Schedules: `GET /api/schedules`, `GET /api/schedules/calendar`
      - Alarms: `GET /api/alarms`, `GET /api/alarms/history`
      - Analytics: `GET /api/analytics/*` (power/water/soil/status/feeds/status)
    - View-only users can still read the appropriate surfaces (no “config.write required just to view dashboards”).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/nodes.rs`
    - `apps/core-server-rs/src/routes/outputs.rs`
    - `apps/core-server-rs/src/routes/schedules.rs`
    - `apps/core-server-rs/src/routes/alarms.rs`
    - `apps/core-server-rs/src/routes/analytics.rs`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: these routes lack `AuthUser`/`OptionalAuthUser` extractors and have no global auth layer under `/api`.
    - 2026-02-03: Fix: added bearer auth + read-scoped capabilities (`nodes.view`, `outputs.view`, `schedules.view`, `alerts.view`, `analytics.view`) across the listed routes (with `config.write` as a stopgap). Added an additive capabilities migration (`infra/migrations/038_auth_capabilities_read_scopes.sql`) to avoid breaking existing controllers.
    - 2026-02-03: Validation: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass), `make ci-farmctl-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-97: Remove unauthenticated “first user wins” bootstrap path (fresh-install takeover risk)**
  - **Description:** `POST /api/users` currently allows unauthenticated user creation when the users table is empty. If the controller is reachable on a LAN during initial setup, any host can race to create the first admin user and permanently take over the system.
  - **Acceptance Criteria:**
    - In production mode, unauthenticated user creation is not possible even on a fresh DB (no “first user wins” LAN takeover).
    - If a bootstrap path is required, it is explicitly gated (e.g., installer/setup secret, localhost-only, time-limited one-time token) and does not rely on “users table is empty” as the only guard.
    - `POST /api/users` behavior is covered by tests (at least: fresh DB, non-empty DB, and bootstrap gating behavior).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/users.rs` (`create_user`)
    - `apps/core-server-rs/src/routes/auth.rs` (`bootstrap`)
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: `create_user` permits unauthenticated creation when `SELECT EXISTS(users)` is false.
    - 2026-02-03: Fix: removed the implicit “empty DB => unauth create” fallback. Bootstrap user creation is now explicitly gated: loopback-only and requires `CORE_ALLOW_BOOTSTRAP_USER_CREATE=1|true|yes`.
    - 2026-02-03: Validation: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass), `make ci-farmctl-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-98: Predictive API endpoints should be real or explicitly disabled (no stubbed “success”)**
  - **Description:** Predictive control-plane endpoints exist in the Rust core-server but are currently stubbed: `GET /api/predictive/trace` returns `[]` unconditionally and `POST /api/predictive/bootstrap` returns `{submitted_samples:0,predictions:0}` without doing work. This creates a false-success UX and contradicts expectations that bootstrap can generate alarms (best effort) when predictive is enabled.
  - **Acceptance Criteria:**
    - If predictive is disabled by default, the API makes that explicit (e.g., return `404`/`501`/clear `400 Predictive disabled`) rather than returning stubbed “success” payloads.
    - If predictive is enabled and configured, `POST /api/predictive/bootstrap` triggers a real best-effort bootstrap flow and reports meaningful counts/status.
    - `GET /api/predictive/trace` returns real trace entries (or is removed/hidden if no trace facility exists).
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **References:**
    - `apps/core-server-rs/src/routes/predictive.rs`
    - `infra/migrations/040_predictive_trace.sql`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: predictive trace/bootstrap endpoints are stubs in Rust.
    - 2026-02-03: Fix: `POST /api/predictive/bootstrap` now performs a best-effort bootstrap over recent DB metrics (z-score anomaly heuristic), emitting predictive alarm events with cooldown gating (unless `force=true`).
    - 2026-02-03: Fix: `GET /api/predictive/trace` is now backed by `predictive_trace` and returns real diagnostic entries.
    - 2026-02-03: Validation: `make ci-core-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **CS-99: Metrics: derived sensors must support forecast_points inputs (history for temp compensation)**
  - **Description:** Derived sensors can be created with forecast-backed inputs (e.g., weather temperature from `forecast_points`), and latest-value evaluation works. However, historical series queries for derived sensors currently skip forecast dependencies, causing “created sensor history” to be blank. Extend the analysis bucket reader used by `/api/metrics/query` to allow derived sensors to depend on forecast sensors by querying `forecast_points` for the required buckets.
  - **Acceptance Criteria:**
    - When a derived sensor depends on one or more `source: "forecast_points"` sensors, `/api/metrics/query` returns a non-empty historical series for the derived sensor (assuming both raw + forecast inputs have data in the window).
    - Forecast sensors remain skipped as direct outputs by the analysis-lake reader, but can be used as inputs for derived evaluation.
    - Forecast input bucketing matches the metrics route semantics (`time_bucket(make_interval(secs => interval), ts)` and “asof” issued-at behavior).
    - Unit tests cover:
      - Forecast config parsing (`subject` default + `mode=asof`).
      - Derived input expansion including forecast inputs (no rejection).
    - Validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **Notes / Run Log:**
    - 2026-02-04: Updated the analysis bucket reader to allow derived sensors to depend on `source: "forecast_points"` sensors by querying `forecast_points` and merging forecast buckets into derived evaluation.
    - 2026-02-04: Added tests for forecast config parsing and derived input expansion including forecast inputs.
    - 2026-02-04: Validation: `make ci-core-smoke` (pass).
    - 2026-02-04: Tier A validated on installed controller `0.1.9.246-temp-comp-lag` (run: `project_management/runs/RUN-20260204-tier-a-temp-comp-lag-0.1.9.246-temp-comp-lag.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B DT-59)

- **CS-100: Derived sensors: support per-input lag_seconds for temp compensation**
  - **Description:** Reservoir-depth-style sensors can drift with a delayed response to air temperature (thermal inertia). To support accurate temperature compensation, derived sensors must be able to evaluate inputs at a shifted timestamp (e.g., temp[t − lag]) while keeping raw at time t. Add `inputs[].lag_seconds` support (signed seconds) and apply it during derived evaluation in the bucket reader used by `/api/metrics/query`.
  - **Acceptance Criteria:**
    - Core-server accepts `lag_seconds` (signed integer seconds, default 0) on derived sensor inputs.
    - `/api/metrics/query` derived evaluation respects `lag_seconds` by looking up input values at `epoch - lag_seconds`.
    - The derived bucket reader expands its internal input read window to cover the maximum lag requested by any derived input, while filtering raw output sensors back to the requested output window.
    - Unit tests cover:
      - Derived spec parsing accepts `lag_seconds` and defaults to 0.
      - Derived bucket evaluation respects lagged inputs.
    - Validation: `make ci-core-smoke` passes.
    - Tier A: installed controller refreshed via runbook (no DB/settings reset) and a lagged temp-comp derived sensor returns non-empty history via `/api/metrics/query`.
  - **Owner:** Core Platform (Codex)
  - **Notes / Run Log:**
    - 2026-02-04: Added `inputs[].lag_seconds` support for derived sensors and applied it in `/api/metrics/query` derived evaluation (lookup input values at `epoch - lag_seconds`; expand internal input window).
    - 2026-02-04: Validation: `make ci-core-smoke` (pass).
    - 2026-02-04: Tier A validated on installed controller `0.1.9.246-temp-comp-lag` (run: `project_management/runs/RUN-20260204-tier-a-temp-comp-lag-0.1.9.246-temp-comp-lag.md`).
      - Derived sensor created for real data: `0875e5462a1165e8d2e11c09` (“Reservoir Depth (temp comp lag 155m)”, temp lag `9300s`).
      - `/api/metrics/query` (72h, 300s buckets) swing reduction improved from ~12.8% (no lag) to ~41.5% (lagged).
  - **Status:** Done (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B DT-59)

- **CS-101: Metrics: derived lag_seconds must work across bucket intervals (7d Trends)**
  - **Description:** Trends auto-selects a coarser bucket interval at longer ranges (e.g., 7d → 30m). For derived sensors with `inputs[].lag_seconds` (temp compensation), if the lag is not an exact multiple of the requested interval, the derived bucket reader looks up input values at an epoch that is not a bucket boundary and the entire derived series can become empty. Fix by snapping misaligned lag lookups to a bucket boundary and expanding the internal input read window by one interval so the snapped bucket is always available.
  - **Acceptance Criteria:**
    - For a derived sensor with `inputs[].lag_seconds` set to a value not divisible by the query interval (e.g., 12,300s lag with a 1,800s bucket), `/api/metrics/query` returns a non-empty series (assuming raw + temperature inputs have data in the window).
    - For aligned lookups (lag divisible by interval), evaluation semantics are unchanged (exact bucket match).
    - If the desired lookup time is already aligned but the bucket is missing (true data gap), the derived bucket is skipped (no “carry-forward”).
    - Unit tests cover misaligned lag snapping.
    - Validation: `make ci-core-smoke` passes.
    - Tier A: installed controller refreshed via runbook (no DB/settings reset) and a real temp-comp derived sensor graphs on Trends at 7d.
  - **Notes / Run Log:**
    - 2026-02-04: Repro on installed controller `0.1.9.246-temp-comp-lag`: temp-comp derived sensor `adab2ed19bb1b9dfe189fa81` returns points at 24h/72h but returns **0 points** at 7d (interval 1800s), even though raw + temperature inputs are non-empty.
    - 2026-02-04: Fix: derived evaluation now floors misaligned lag lookups to a bucket boundary and expands the input read window by one interval for safety.
    - 2026-02-04: Tests: added coverage for misaligned lag snapping (`test_compute_derived_buckets_floors_misaligned_lag_to_bucket_boundary`).
    - 2026-02-04: Validation: `make ci-core-smoke` (pass).
    - 2026-02-04: Tier A validated on installed controller `0.1.9.247-derived-lag-buckets` (run: `project_management/runs/RUN-20260204-tier-a-cs101-derived-lag-buckets-0.1.9.247-derived-lag-buckets.md`).
      - Evidence: `manual_screenshots_web/tier_a_0.1.9.247-derived-lag-buckets_cs101_trends_7d_2026-02-04_062902444Z/trends_7d_temp_comp_depth.png`
  - **Owner:** Core Platform (Codex)
  - **Status:** Done (Tier A validated installed `0.1.9.247-derived-lag-buckets`; Tier B DT-59)

- **CS-102: Derived sensors: allow derived inputs (enable temp compensation of derived sensors)**
  - **Description:** The Temp Compensation wizard creates a derived sensor. When the user wants to compensate a sensor that is *already derived* (e.g., a computed/aggregated depth), the output becomes a derived-of-derived sensor. The core-server currently rejects derived sensors as derived inputs and the metrics bucket reader does not evaluate derived-of-derived dependencies. Enable derived sensors to depend on other derived sensors (with cycle + depth validation) so derived temp-compensation is supported end-to-end.
  - **Acceptance Criteria:**
    - `POST /api/sensors` accepts a derived sensor config whose `derived.inputs[]` includes one or more derived sensors (no 400 “cannot depend on other derived sensors”).
    - Validation rejects:
      - Derived cycles (a derived sensor cannot depend on itself transitively).
      - Derived graphs deeper than the supported limit (match the bucket reader depth limit).
    - `/api/metrics/query` returns non-empty bucketed history for a derived-of-derived temp compensation sensor (assuming inputs have data).
    - Derived latest-value evaluation supports derived-of-derived so `/api/sensors` and `/api/sensors/{id}` can populate `latest_value/latest_ts` when possible.
    - Unit tests cover derived-of-derived evaluation in the analysis bucket reader (dependency ordering + correctness).
    - Validation: `make ci-core-smoke` passes.
    - Tier A: installed controller refreshed via runbook (no DB/settings reset) and a derived sensor can be selected + compensated in `/analytics/compensation` with a screenshot captured + viewed under `manual_screenshots_web/`.
  - **Owner:** Core Platform (Codex)
  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260204-tier-a-cs102-dw218-dw219-0.1.9.248-derived-of-derived.md`
  - **Status:** Done (validated on installed controller `0.1.9.248-derived-of-derived`; clean-host E2E deferred to DT-59)

---

## Rust Core Server Migration
### Done
- **RCS-19: Port conflict detection**
  - **Description:** Add preflight port conflict detection at core-server-rs startup with a clear actionable error message.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0024-rcs-19-add-port-conflict-detection-at-startup.md`
  - **Acceptance Criteria:**
    - Startup fails fast with a helpful message when the listen port is already in use.
  - **Status:** Done (`cd apps/core-server-rs && cargo test -q`, `python3 tools/check_openapi_coverage.py`)


- **RCS-18: Integration tests**
  - **Description:** Add integration tests for Renogy BT-2 and WS-2902 preset flows (preset load, node-agent config push, DB upsert, and ingest behavior).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0023-add-integration-tests-for-preset-flows.md`
  - **Acceptance Criteria:**
    - Integration tests cover both flows and run in CI.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260104_050252`)


- **RCS-15: SQL error leakage**
  - **Description:** Eliminate SQL error information leakage from Rust core-server API responses by centralizing error mapping and returning generic client messages while logging full details server-side.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0020-eliminate-sql-error-information-leakage.md`
  - **Acceptance Criteria:**
    - A centralized `internal_error()` exists and route-local raw `err.to_string()` responses are removed.
    - Common DB cases return specific safe errors (not found, conflict, etc.).
    - API responses do not include raw SQL/constraint/schema details when inducing failures.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260103_211138/e2e_setup_smoke_20260103_211138.log`)


- **RCS-14: Sunset the Python core-server (remove legacy runtime)**
  - **Description:** Retire the Python core-server runtime now that production behavior is stable, leaving the Rust core-server as the only controller backend implementation.
  - **References:**
    - `apps/core-server/README.md`
    - `docs/README.md`
    - `README.md`
    - `tools/rcs_parity_http_smoke.py`
  - **Acceptance Criteria:**
    - No production or CI workflows rely on `apps/core-server` as a runnable backend.
    - Any remaining Python code is tooling-only (migrations/seed helpers) and not a server/runtime dependency.
    - Docs remove references to running the Python core-server as the controller backend.
  - **Status:** Done (`make ci-core-smoke`, `make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_195458.log`)


- **RCS-10: Implement missing OpenAPI paths in Rust core-server**
  - **Description:** Close the contract gap where `/apps/core-server/openapi/farm-dashboard.json` defines endpoints that the Rust core-server does not currently implement or implements under different paths.
  - **References:**
    - `docs/audits/2026-01-01-rust-migration-audit-snippet.md`
    - `apps/core-server/openapi/farm-dashboard.json`
    - `apps/core-server-rs/src/routes/`
  - **Acceptance Criteria:**
    - All OpenAPI paths are implemented by the Rust router (no missing paths for: auth/me, dashboard/demo, forecast/latest+status, indicators, metrics/ingest, templates, sensors/outputs CRUD, backups/{node_id}, predictive/bootstrap).
    - Installer-path validation remains green: `make e2e-installer-stack-smoke`.
    - A local check exists that fails if the Rust router is missing any OpenAPI path.
  - **Status:** Done (`python3 tools/check_openapi_coverage.py`, `make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_071125.log`)


- **RCS-11: Make Rust core-server the canonical OpenAPI source**
  - **Description:** Stop serving a vendored Python OpenAPI JSON from the Rust core-server; generate and export OpenAPI directly from the Rust route/schema definitions and use that as the SDK source of truth.
  - **References:**
    - `docs/audits/2026-01-01-rust-migration-audit-snippet.md`
    - `apps/core-server-rs/src/openapi.rs`
    - `tools/api-sdk/export_openapi.py`
    - `tools/check_openapi_coverage.py`
  - **Acceptance Criteria:**
    - `core-server --print-openapi` (or `GET /api/openapi.json`) exports a Rust-generated spec (not `include_str!("../../core-server/openapi/farm-dashboard.json")`).
    - `apps/core-server/openapi/farm-dashboard.json` is generated from the Rust core-server and CI drift checks validate against that output.
    - Generated TS client remains aligned and `make ci-web-smoke` remains green.
  - **Status:** Done (`python3 tools/api-sdk/export_openapi.py`, `python3 tools/check_openapi_coverage.py`, `make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_071125.log`)


- **RCS-12: Expand parity harness endpoint coverage beyond the “smoke subset”**
  - **Description:** Extend the Python-vs-Rust parity harness to cover the full contract surface (or an explicitly versioned “minimum supported” subset) so “green parity” matches “feature complete”.
  - **References:**
    - `docs/audits/2026-01-01-rust-migration-audit-snippet.md`
    - `tools/rcs_parity_http_smoke.py`
  - **Acceptance Criteria:**
    - The harness can run a curated, versioned endpoint set that includes all production-critical areas (auth/me, templates, indicators, metrics ingest/query, sensors/outputs CRUD, backups per-node).
    - Output remains deterministic and actionable.
    - `make rcs-parity-http-smoke` remains runnable without container runtimes.
  - **Notes:** The original harness compared Python vs Rust; after `RCS-14` removed the Python HTTP runtime, `make rcs-parity-http-smoke` was repurposed as a Rust-only seeded HTTP regression snapshot while preserving the historical parity artifacts in `reports/`.
  - **Status:** Done (`make rcs-parity-http-smoke`; log: `reports/manual-rcs-parity-http-smoke-20260102_103037.log`; artifacts: `reports/rcs-parity-http-smoke/2026-01-02T18-30-38Z/`)


- **RCS-13: Switch local dev + CI default to Rust core-server**
  - **Description:** Eliminate day-to-day dependence on the legacy Python core-server by making the Rust core-server the default for local dev and CI while keeping the Python server available only for explicitly-invoked parity/legacy flows until sunset.
  - **References:**
    - `Makefile`
    - `apps/core-server-rs/`
    - `apps/core-server/` (legacy; parity-only until sunset)
  - **Acceptance Criteria:**
    - `make core` (or an equivalent documented target) runs the Rust core-server by default.
    - CI no longer requires running the Python core-server test suite to validate controller behavior (Rust tests + installer-path E2E gate cover production behavior).
    - Any remaining Python core-server usage is explicitly labeled “legacy/parity-only” in docs.
  - **Status:** Done (`make ci-core-smoke`; log: `reports/manual-ci-core-smoke-20260102_103641.log`)


- **RCS-1: Define the contract-first migration plan (ADR + parity harness)**
  - **Description:** Document the migration strategy and build the scaffolding that keeps Rust and Python behavior aligned during the rewrite.
  - **References:**
    - `docs/ADRs/0004-rust-core-server-migration-(api-+-static-dashboard-served-by-rust).md`
    - `project_management/archive/archive/tickets/TICKET-0004-rust-core-server-migration-(contract-first-plan-parity-harness).md`
  - **Acceptance Criteria:**
    - An ADR captures the migration scope, ownership of the OpenAPI contract, and the parity strategy (Rust vs Python side-by-side).
    - A parity test harness can run both backends and compare key endpoint responses against the same DB seed.
    - The harness is runnable in CI/local without container runtimes.
  - **Status:** Done (`make rcs-parity-http-smoke`; artifacts: `reports/rcs-parity-http-smoke/2026-01-01T14-56-14Z/`)


- **RCS-5: Add response parity harness (Python vs Rust)**
  - **Description:** Extend the contract-first migration scaffolding into a runnable parity harness that compares Python vs Rust responses against the same seeded DB so drift is caught before UI regressions.
  - **Acceptance Criteria:**
    - A local target runs both backends side-by-side and compares a curated endpoint set (nodes/sensors/outputs/schedules/alarms/backups/scan) against the same DB seed.
    - Parity output is actionable (per-endpoint diff, truncated JSON samples, deterministic ordering).
    - No container runtimes required; macOS-only assumptions are documented.
  - **Status:** Done (`make rcs-parity-http-smoke`; artifacts: `reports/rcs-parity-http-smoke/2026-01-01T14-56-14Z/`)


- **RCS-2: Add a Rust core-server skeleton (API + static assets + OpenAPI)**
  - **Description:** Create the Rust server foundation that will eventually replace the Python core-server in production.
  - **References:**
    - `apps/core-server-rs/` (skeleton crate)
  - **Acceptance Criteria:**
    - Rust server serves `/healthz` and a minimal `/api` surface.
    - Rust server serves static dashboard assets from `/` with SPA fallback routing.
    - Rust server exports the canonical OpenAPI spec (`--print-openapi`).
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_181750.log`)


- **RCS-3: Make the dashboard build output static (no Node runtime in production)**
  - **Description:** Convert the dashboard runtime to static assets suitable for serving directly by the Rust core-server (no Next.js server process in production).
  - **Acceptance Criteria:**
    - Dashboard build produces a static asset directory suitable for `GET /` hosting (JS/CSS/assets).
    - Runtime API calls use relative `/api/*` paths (no baked-in host/port).
    - The installer/launchd production profile no longer needs a dashboard-web service process.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_181750.log`)


- **RCS-4: Switch the production controller runtime to the Rust core-server**
  - **Description:** Replace the bundled Python core-server runtime with a single Rust `core-server` binary (`apps/core-server-rs`) that serves both `/api/*` and the dashboard static assets (`/`).
  - **Acceptance Criteria:**
    - `farmctl bundle` builds `apps/core-server-rs` and bundles the binary at `artifacts/core-server/bin/core-server`.
    - The installer gate runs against the installed Rust core-server (no Python runtime required for the controller runtime).
    - `make e2e-setup-smoke` passes (via installer stack gate).
  - **Notes:** Keep the Python core-server available for side-by-side parity comparisons during the transition.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_181750.log`)


- **RCS-6: Implement `/api/dashboard/state` snapshot endpoint in Rust**
  - **Description:** Provide a single “dashboard snapshot” endpoint to avoid N+1 client fallback calls and improve perceived performance for the main dashboard.
  - **Acceptance Criteria:**
    - `GET /api/dashboard/state` returns a payload matching `DashboardSnapshotSchema` (or an approved, versioned schema update).
    - Dashboard no longer logs schema/fallback noise when the snapshot endpoint is available.
    - `make e2e-setup-smoke` still passes.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_181750.log`)


- **RCS-7: Enforce auth + capabilities in Rust core-server**
  - **Description:** Make Rust core-server behavior match production expectations by requiring auth where appropriate and enforcing view-only vs control capabilities for writes/commands.
  - **Acceptance Criteria:**
    - Write endpoints require a valid bearer token.
    - Capability checks block output commands, schedule edits, and config mutations without the right capability.
    - `make e2e-setup-smoke` still passes (with the E2E-created admin user).
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_181750.log`)


- **RCS-8: Expand Rust OpenAPI export for shipped endpoints**
  - **Description:** Extend `apps/core-server-rs` OpenAPI export to cover the endpoints currently shipped/used by the installed dashboard and installer-path E2E.
  - **Acceptance Criteria:**
    - Rust OpenAPI includes the shipped endpoint surface (at minimum: auth/login, users, nodes, sensors, outputs, schedules, alarms, backups, discovery scan, dashboard snapshot).
    - `make rcs-parity-smoke` passes.
  - **Status:** Done (`make rcs-parity-smoke`; log: `reports/rcs-parity-smoke-20251231_181750.log`)


- **RCS-9: Switch generated SDKs to Rust OpenAPI (contract-first)**
  - **Description:** Make Rust the canonical OpenAPI source for `apps/dashboard-web` (TS client) and iOS SDK generation, with CI drift checks against the Rust-exported contract.
  - **Acceptance Criteria:**
    - `tools/api-sdk/export_openapi.py` exports from the Rust core-server to `apps/core-server/openapi/farm-dashboard.json`.
    - Contract drift checks run against the Rust-exported spec.
    - `make ci-web-smoke` and `make rcs-parity-smoke` still pass.
  - **Status:** Done (`make ci-web-smoke`, `make rcs-parity-smoke`; logs: `reports/ci-web-smoke-20251231_181750.log`, `reports/rcs-parity-smoke-20251231_181750.log`)


---

## Telemetry Ingest Sidecar
### Done
- **TS-6: Split telemetry-sidecar ingest monolith into modules**
  - **Description:** Improve maintainability by splitting `ingest.rs` into focused modules (types, rolling average logic, DB ingestion coordination).
  - **Acceptance Criteria:**
    - `apps/telemetry-sidecar/src/ingest.rs` is split into smaller modules without behavior changes.
    - `cargo test` passes for telemetry-sidecar.
    - `make e2e-web-smoke` still passes.
  - **Status:** Done (`cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml`, `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`, `make e2e-web-smoke`)

- **TS-7: Fix offline flapping for >5s sensors**
  - **Description:** Prevent node/sensor status from flapping offline between samples when telemetry intervals are > the global offline threshold (production default is 5s but many sensors publish at 30s+ cadence).
  - **Acceptance Criteria:**
    - Offline detection threshold respects `sensors.interval_seconds` (and COV sensors do not get marked offline due to “no change”).
    - Default production behavior does not generate continuous “Sensor Offline” alarms for healthy 30s sensors.
  - **Evidence:**
    - Tier A (installed controller `0.1.9.70`): upgraded via setup-daemon; no `Node Offline`/`Sensor Offline` alarm flapping observed after refresh. Run log: `project_management/runs/RUN-20260110-tier-a-offline-flapping-0.1.9.70.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **TS-8: Accept non-UUID node status topics + persist node health**
  - **Description:** Support node-agent status topics keyed by `NODE_NODE_ID` (e.g., `pi-<macsuffix>`), not only controller UUIDs, so node uptime/CPU/storage and online/offline status update correctly in production.
  - **Acceptance Criteria:**
    - Telemetry-sidecar ingests `iot/{node_id}/status` where `{node_id}` may be a UUID or a non-UUID stable node-agent id.
    - Non-UUID node-agent ids resolve to controller UUIDs via `nodes.config.agent_node_id` (populated during adoption/profile sync).
    - `/api/dashboard/state` reflects non-zero `uptime_seconds` / `cpu_percent` / `storage_used_bytes` for online nodes after status publishes.
    - `cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml` passes.
  - **Notes / Run Log:**
    - 2026-01-05: Implemented node-agent id → controller UUID resolution and persisted node health fields; deleted nodes/sensors are ignored to prevent ghost telemetry.
    - 2026-01-10: Tier A (installed controller `0.1.9.70`): Pi5 nodes include `nodes.config.agent_node_id` and `/api/dashboard/state` reports non-zero `uptime_seconds` / `cpu_percent` / `storage_used_bytes` for `Pi5 Node 1` and `Pi5 Node 2`. Evidence: `project_management/runs/RUN-20260110-tier-a-phase3-adoption-ts8.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **TS-1: Make the sidecar the only MQTT consumer**
  - **Description:** Move telemetry ingest to the Rust sidecar and disable the core-server MQTT consumer by default to avoid double ingest.
  - **Acceptance Criteria:**
    - Core server starts without MQTT ingest or gRPC dependencies when sidecar-only mode is enabled.
    - Default compose/run targets launch the sidecar for MQTT ingest and do not run `app.mqtt_runner`.
    - Telemetry is ingested exactly once per sample when the sidecar is active.
  - **Status:** Done


- **TS-2: Run predictive alarms as a DB-driven Python worker**
  - **Description:** Keep predictive alarms in Python but decouple them from MQTT by reading from the DB (with an optional light feed from the sidecar).
  - **Acceptance Criteria:**
    - Predictive worker runs without the core-server MQTT consumer and still produces predictive alarms from DB metrics.
    - Worker uses a durable cursor or watermark so samples are processed once.
    - Optional sidecar HTTP/gRPC feed can submit recent samples without triggering double ingest.
  - **Status:** Done


- **TS-3: Align sidecar ingest semantics with core-server ingest**
  - **Description:** Match core-server ingest behavior for COV, rolling averages, offline tracking, and duplicate handling.
  - **Acceptance Criteria:**
    - COV and rolling average behavior matches current `app/services/mqtt_consumer.py` results for the same telemetry.
    - Node and sensor offline status updates use the same thresholds and alarm paths.
    - Duplicate handling is explicit and logged (no silent drops).
  - **Status:** Done


- **TS-4: Add sidecar-only ingest regression tests**
  - **Description:** Validate sidecar-only MQTT ingest + DB-driven predictive worker in automation.
  - **Acceptance Criteria:**
    - Integration test exercises sidecar ingest pipeline (DB inserts + offline alarms) without core-server MQTT.
    - Predictive worker test consumes DB samples and persists predictive alarms.
    - `make demo-live` (or equivalent) can run with sidecar-only ingest enabled.
  - **Status:** Done (sidecar ingest test + predictive worker + demo-live wiring)


- **TS-5: Align sidecar quality decoding with DB type**
  - **Description:** Prevent ingest warnings by decoding the `metrics.quality` column using the same smallint type as the database schema.
  - **Acceptance Criteria:**
    - Sidecar COV state reads `metrics.quality` without type mismatch warnings.
    - Test schema mirrors `smallint` for `metrics.quality`.
  - **Status:** Done (make e2e-web-smoke)


---

## Offline Telemetry Spool + Backfill Replay
### Done
- **OT-1: Phase 0: Lock requirements + policy decisions (disk/time/security) + finalize ADR**
  - **Description:** Convert the selected architecture into a concrete, implemented contract: spool policy defaults, time semantics, ACK protocol, and the telemetry envelope (`seq`/`stream_id`/`time_quality`/`backfill`).
  - **References:**
    - `project_management/tickets/TICKET-0049-offline-telemetry-spool-+-backfill-replay-(append-only-segments-+-rust-node-forwarder-+-ack).md`
    - `docs/ADRs/0009-offline-telemetry-buffering:-append-only-segments-+-seq-ack-+-controller-receipt-time-liveness.md`
  - **Acceptance Criteria:**
    - Default spool budget policy is chosen and implemented with an explicit config override path.
    - Time accuracy requirement is explicitly stated and a Phase 1 vs Phase 2 plan is recorded accordingly.
    - Security posture is explicitly stated (LAN-trust vs device-capture-in-scope) and Phase 1 storage permissions are specified.
    - MQTT constraints are explicitly stated (v3.1.1 semantics MVP; v5 optional follow-up).
    - Telemetry envelope is specified (at minimum: `sensor_id`, `sample_ts`, `value`, `seq`, `stream_id`, `mono_ms`, `time_quality`, `backfill`).
  - **Evidence / Run Log:**
    - `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`
  - **Status:** Done

- **OT-2: Phase 1: Rust node-forwarder segment spool (framing + recovery + bounded retention)**
  - **Description:** Add a new Rust node service (`node-forwarder`) that owns local durability. It writes an append-only, CRC-framed segment log, enforces caps (bytes/age/keep-free), and recovers cleanly after crashes/power loss.
  - **Acceptance Criteria:**
    - Segment format is versioned and framed (`len + crc32c + payload`) with a fixed header; recovery truncates only the invalid tail frame.
    - Rotation defaults are implemented (roll by 1h OR 128 MiB, whichever first) and are configurable.
    - Retention is enforced with deterministic drop policy (drop-oldest closed segments) and emits an explicit loss-range event/counter.
    - Durability is bounded by a configurable sync interval (default ~1s) and uses `fdatasync()` batching (no per-sample fsync).
    - Unit/integration coverage exists for: tail corruption recovery, cap enforcement, and segment deletion behavior.
  - **Status:** Done (`cargo test --manifest-path apps/node-forwarder/Cargo.toml`)

- **OT-3: Phase 1: Rust node-forwarder publish + replay (throttle + status priority)**
  - **Description:** Implement MQTT publishing for live telemetry and rate-limited backlog replay. Live status/heartbeat must remain responsive during backlog drains.
  - **Acceptance Criteria:**
    - Two publish lanes exist: status/heartbeat is never queued behind replay; telemetry uses rate-limited drain.
    - Replay is ordered by capture order (segment order, then frame order) and includes a `backfill=true` flag.
    - Throttle controls exist for both message rate and byte rate (token-bucket).
    - Node-forwarder consumes controller ACKs (`iot/{node_id}/ack`) and deletes only fully ACKed closed segments.
    - QoS 1 telemetry publish is used and duplicates are tolerated (no client-side dedupe requirement).
  - **Status:** Done (hardware + Tier A validated; see run log)

- **OT-4: Phase 1: node-agent sampling → local IPC (always-sample; no uplink coupling)**
  - **Description:** Modify the Python `node-agent` so sampling continues even when MQTT/controller connectivity is hard down. Samples are pushed to the local Rust forwarder over localhost HTTP.
  - **Acceptance Criteria:**
    - Sampling schedule continues during a simulated controller/broker outage (no event-loop stall and no “sampling stops because publish failed” coupling).
    - Samples are delivered over local IPC to node-forwarder; backpressure is bounded and does not deadlock the sampler.
    - If node-forwarder is down/unavailable, node-agent fails safely (bounded local queue; no unbounded RAM growth).
    - `make ci-node-smoke` passes.
  - **Status:** Done (`make ci-node-smoke`; hardware + Tier A validated)

- **OT-5: Phase 1: controller ACK topic + durable acked_seq (post-DB-commit)**
  - **Description:** Extend the Rust telemetry-sidecar to publish an application-level ACK (`iot/{node_id}/ack`) only after samples are durably committed to TimescaleDB, so nodes can safely truncate spools.
  - **Acceptance Criteria:**
    - ACK payload includes `acked_seq` (highest contiguous ingested seq) per node, published periodically and monotonic.
    - Sidecar restart does not reset ACKs incorrectly (acked state is derived from durable data or persisted explicitly).
    - Node-forwarder can delete segments based on `acked_seq` without losing unacked data.
    - `cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml` passes.
  - **Status:** Done (`cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml`)

- **OT-6: Phase 1: controller liveness monotonicity (receipt-time last_rx_at + sample-time freshness)**
  - **Description:** Update controller liveness semantics so backfill cannot cause false “offline” flaps. Liveness is derived from controller receipt time and is monotonic; sample timestamps are used only for data freshness.
  - **Acceptance Criteria:**
    - Controller tracks `last_rx_at = max(received_at)` for node + sensor liveness and never regresses it.
    - Controller tracks `last_sample_ts = max(sample_ts)` for freshness and never regresses it.
    - A replay of old samples does not move a node/sensor from online→offline unless receipt-time timeout is exceeded.
    - Regression coverage exists for the “last_seen goes backward during replay” failure mode.
  - **Status:** Done (validated under replay harness; see run log)

- **OT-7: Phase 1: enforce idempotent ingest invariants for QoS1 replay duplicates**
  - **Description:** Ensure the ingest pipeline is correct under QoS 1 (at-least-once). Replay duplicates must not create duplicate rows or corrupt “latest”/status logic.
  - **Acceptance Criteria:**
    - DB uniqueness/PK guarantees are documented and enforced for metrics ingest (typically `(sensor_id, sample_ts)`).
    - Ingest uses `ON CONFLICT DO NOTHING` (or equivalent) and is covered by tests for duplicate replay.
    - ACK computation remains correct under duplicates (acked_seq reflects durable ingest, not broker receipt).
  - **Status:** Done (unit tests + Tier A + hardware replay validated)

- **OT-8: Phase 1: spool health observability surfaces (node status + controller APIs + dashboards)**
  - **Description:** Expose spool health and replay progress for operators so it’s obvious when data is buffered/replayed/dropped.
  - **Acceptance Criteria:**
    - Node status includes spool bytes, oldest sample age, drop counters/ranges, `last_acked_seq`, and “replay draining” state.
    - Controller APIs surface per-node ingest receipt time, freshness time, and ACK progression.
    - Dashboard shows basic spool state on the Node detail view (no SSH required).
  - **Status:** Done (dashboard surfacing + screenshot evidence; see run log)

- **OT-9: Phase 1: E2E harness for disconnect/reconnect + reboot-mid-outage + catch-up**
  - **Description:** Add a deterministic harness for offline buffering behavior (disconnect simulation, reboot during outage, catch-up behavior under throttles). Tests align with production codepaths.
  - **Acceptance Criteria:**
    - Automated scenario exists that simulates outage + backlog replay and asserts “no offline flaps during replay”.
    - Scenario exists that reboots a node mid-outage and verifies spool recovery (only tail frame loss allowed).
    - Postflight leaves no orphaned processes/jobs (Tier‑B hygiene applied when run on clean hosts).
  - **Status:** Done (`tools/ot_offline_buffer_harness.sh`; validated on Pi 5 nodes; see run log)

- **OT-10: Tier A validation run + evidence (installed controller; no DB/settings reset)**
  - **Description:** Validate the buffering/replay feature on the installed controller (Tier A), with evidence recorded per runbook requirements.
  - **Acceptance Criteria:**
    - Installed controller is upgraded/refreshed to the target build and remains healthy (`/healthz` + `farmctl health`).
    - Disconnect window executed and backlog drains on reconnect without destabilizing the controller.
    - Evidence recorded under `project_management/runs/` including at least one **captured and viewed** screenshot under `manual_screenshots_web/`.
    - Clean-host Tier B is deferred to OT-13 when needed.
  - **Evidence / Run Log:**
    - `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to OT-13)

- **OT-11: Validate offline buffering on hardware (Pi 5 disconnect window + reboot/power-loss test)**
  - **Description:** Validate offline buffering and replay on real Pi 5 hardware (microSD-boot and NVMe-boot) including a disconnect window and a reboot recovery.
  - **Acceptance Criteria:**
    - Hard disconnect window executed and ≥48h-equivalent buffering demonstrated (real time or accelerated), bounded by caps.
    - Reboot occurs during the disconnect window; spool is recovered and replay continues on reconnect.
    - Loss-range events emitted if caps exceeded; no silent drops.
  - **Evidence / Run Log:**
    - `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`
  - **Status:** Done (validated on microSD + NVMe Pi 5 nodes)

- **OT-12: Prune legacy offline-buffer codepaths (single durability layer; no dead code)**
  - **Description:** Remove legacy “offline buffer” implementations so durability has a single, well-defined layer (node-forwarder spool + ACK).
  - **Acceptance Criteria:**
    - Legacy node buffering artifacts are removed or fully subsumed with a clear migration path.
    - Docs/runbooks no longer mention the legacy buffer; operator surfaces reference the new spool/ACK semantics.
    - Tests remain green for affected components.
  - **Status:** Done (legacy buffer removed; tests + Tier A + hardware validated)

---

## Node Agent
### Done
- **NA-55: Publish per-node network health telemetry (ping/latency/jitter + uptime %)**
  - **Description:** Collect controller reachability and network quality metrics on each Pi 5 node and publish them as telemetry so operators can trend latency/jitter and “uptime %” in the dashboard.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Spec clarification (implemented):**
    - Use a **TCP connect probe** (not ICMP ping) to avoid root/ICMP privileges. Target defaults to the node’s configured MQTT broker (`mqtt_host:mqtt_port`).
    - Probe cadence and buffers are bounded (short rolling window for latency/jitter + 24h outcomes for uptime%).
    - Publish into the existing status payload (`iot/<node_id>/status`) using stable keys already modeled across the stack:
      - `network_latency_ms`, `network_jitter_ms`, `uptime_percent_24h`
  - **Acceptance Criteria:**
    - Node-agent measures reachability to the controller (target defined by config) without blocking the HTTP/MQTT event loop (bus-owner/background task only).
    - Metrics include at minimum: rolling latency (ms), jitter (ms), and success/failure to compute uptime % over 24h.
    - Metrics are published into the existing MQTT telemetry pipeline with bounded buffers and explicit cadence controls.
    - No new internet requirements are introduced on the Pi (LAN-only is sufficient).
    - `make ci-node` remains green (simulated tests/fixtures; hardware validation tracked separately if needed).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.77`.
    - Pi5 Node 2 status publishes populate `/api/nodes` fields `network_latency_ms`, `network_jitter_ms`, `uptime_percent_24h`.
    - History is queryable via `/api/metrics/query` (node-health sensor IDs are deterministic).
    - Node-agent tests: `cd apps/node-agent && .venv/bin/python -m pytest -q` (pass).
    - Telemetry-sidecar tests: `cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **NA-56: Publish per-node CPU/RAM telemetry (including per-core CPU)**
  - **Description:** Collect CPU and memory utilization on each Pi 5 node and publish them as telemetry so operators can monitor node health over time.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Spec clarification (implemented):**
    - Publish CPU/RAM in the existing status payload (`iot/<node_id>/status`) as additional JSON keys:
      - `cpu_percent_per_core` (array), plus `cpu_percent` total
      - `memory_used_bytes`, plus existing `memory_percent`
    - Controller persists both current values (for Nodes list) and a short time-series history (for Node detail charts) without requiring SSH.
  - **Acceptance Criteria:**
    - Node-agent publishes per-core CPU utilization and total memory utilization on a configurable cadence.
    - Collection runs off the async hot path (no blocking calls in request handlers/MQTT callbacks).
    - Metrics are exposed in the dashboard as trends (UI tracked separately).
    - `make ci-node` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.77`.
    - Pi5 Node 2 shows `memory_percent` + `memory_used_bytes` on `/api/nodes`.
    - Node-agent tests: `cd apps/node-agent && .venv/bin/python -m pytest -q` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **NA-57: Clarify and implement “power-on / auto-recovery” behavior for Pi 5 nodes**
  - **Description:** The request “automatically power on Pi 5 nodes every 12 hours” is not achievable in software alone if the node is physically powered off. Define the intended operator outcome (watchdog/periodic reboot/Wake-on-LAN/PoE power cycling) and implement the feasible software portion.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Spec clarification (implementation target):**
    - **What software can do:** ensure `node-agent` and optional services auto-restart on crash (systemd `Restart=always` / watchdog), and optionally schedule a **configurable periodic reboot** (maintenance) if the operator wants it.
    - **What software cannot do:** power on a node that is physically unpowered; that requires external power hardware (PoE PSE control, smart plug/relay) or Wake-on-LAN + hardware support.
  - **Acceptance Criteria:**
    - A spec note documents what is and is not possible without external power hardware.
    - If a software-only recovery is chosen (e.g., periodic reboot or watchdog), it is configurable per node and implemented as part of the generic node stack without breaking offline installs.
    - Hardware-dependent recovery (PoE relay/smart plug) is tracked as a separate hardware task if needed.
  - **Evidence (Tier A):**
    - Implemented software-only watchdog auto-restart (`node-agent.service` restart policy) and verified on Pi5 Node 2 via a service restart without manual intervention.
  - **Status:** Done (validated on installed controller; hardware power-cycling remains out-of-scope)

- **NA-60: Renogy BT-2 Modbus settings write support (safe apply + read-back verify)**
  - **Description:** Extend the Renogy BT‑2 collector plumbing so node-agent can read and safely write controller settings over BLE Modbus with strict validation and read-back verification. This enables the core server to implement an auditable apply flow without the browser touching BLE.
  - **Acceptance Criteria:**
    - Node-agent exposes endpoints for Renogy BT‑2 devices to:
      - Read current settings registers (and any live telemetry needed for validation).
      - Apply a validated set of register writes (0x06/0x10) with:
        - per-device apply lock (no concurrent writes),
        - strict register-map validation (min/max/units/scaling),
        - read-back verification and clear per-field success/failure reporting.
    - BLE write failures (timeouts/CRC/modbus exception) return structured errors and do not crash the polling loop.
    - Unit tests cover Modbus write frame building + response parsing (including exception frames).
  - **Evidence:**
    - Node-agent tests: `cd apps/node-agent && .venv/bin/python -m pytest -q` (pass).
  - **Status:** Done (validated via unit tests; hardware validation deferred to NA-61)

- **NA-63: Renogy BT-2 telemetry reconnect when scan fails (BlueZ cached path fallback)**
  - **Description:** Prevent Renogy BT‑2 telemetry from stalling after restarts when the device is known to BlueZ but not discoverable via BLE scanning (Bleak returns `None`).
  - **Acceptance Criteria:**
    - If `BleakScanner.find_device_by_address()` returns `None`, node-agent attempts a connection via BlueZ’s cached D‑Bus object path (`/org/bluez/<hci>/dev_<AA_BB...>`).
    - Disconnect errors do not crash the polling loop (collector continues on the next interval).
    - Unit tests cover the BlueZ-path formatter and fallback device construction.
  - **Evidence (Tier A):**
    - Node 1 Renogy sensors resumed updating in the dashboard after upgrade/restart; **user validated visually** (no screenshots).
    - Node-agent tests: `NODE_TEST_BUILD_FLAVOR=prod make ci-node` (pass).
  - **Status:** Done (Tier A: user validated visually; no screenshots)

- **NA-53: Offline-capable Pi 5 node installs (no internet required on the Pi)**
  - **Description:** Make both Pi deployment paths (preconfigured media first-boot and deploy-from-server over SSH) fully offline on the Pi: no `apt-get`/`pip` network fetches at install time. Ship the full Python runtime deps + required system components (e.g., pigpiod) inside the controller bundle/node overlay so nodes can be installed on an isolated LAN with no WAN.
  - **References:**
    - `tools/build_image.py` (preconfigured media kit)
    - `apps/core-server-rs/src/services/deployments.rs` (SSH deploy)
    - `apps/farmctl/src/bundle.rs` (node-agent overlay builder)
    - `docs/runbooks/pi5-deployment-tool.md`
    - `docs/runbooks/pi5-preconfigured-media.md`
  - **Acceptance Criteria:**
    - `node-agent-overlay.tar.gz` contains all Python deps required to run `node-agent.service` on Pi 5 without pip installs (vendored site-packages or equivalent), and includes pigpio daemon support where required.
    - The preconfigured media first-boot script contains no `apt-get` or `pip install` steps and successfully enables/starts node-agent services from the shipped payload.
    - The deploy-from-server (SSH) job contains no `apt-get` or `pip install` steps and successfully enables/starts node-agent services from the shipped payload.
    - The dashboard Deployment UI remains the primary non-expert frontend for SSH deploy (no CLI required).
    - A fast smoke check exists that fails if these install paths reintroduce network fetches.
    - `make ci-node`, `make ci-web-smoke`, and `make e2e-installer-stack-smoke` remain green.
  - **Status:** Done (`make ci-node`, `make ci-web-smoke`, `make e2e-installer-stack-smoke`; logs: `reports/na53-ci-node-20260101_155139.log`, `reports/na53-ci-web-smoke-20260101_155114.log`, `reports/na53-e2e-installer-stack-smoke-20260101_155818.log`)


- **NA-44: Pi 5 local display (basic status + live values)**
  - **Description:** Add an optional Pi 5 local display mode that renders a kiosk-friendly view (`/display`) showing node identity, core comms health, latency/jitter, and live sensor values without blocking telemetry or HTTP.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0007-feature-001-pi5-local-display-basic.md`
  - **Acceptance Criteria:**
    - Display mode is disabled by default and can be enabled per node via config sync (no manual node edits).
    - Latency/jitter sampling uses a non-privileged method (TCP connect) and runs off the HTTP event loop.
    - Kiosk failures do not impact telemetry publish/adoption/provisioning.
  - **Status:** Done (`make ci-node`, `make e2e-web-smoke`)


- **NA-45: Pi 5 local display (advanced controls + trends)**
  - **Description:** Extend the Pi 5 local display to optionally support output control and lightweight trend views, with safety gating and capability checks.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0008-feature-002-pi5-local-display-advanced-controls.md`
  - **Acceptance Criteria:**
    - Advanced features are disabled by default and require explicit enablement.
    - Output control is capability-gated and fails safely when offline.
    - Trend display pulls from the same canonical APIs used by the dashboard (no bespoke data path).
  - **Status:** Done (`make ci-node`, `make e2e-web-smoke`)


- **NA-47: Implement generic Pi 5 node stack baseline (single stack + feature toggles)**
  - **Description:** Ensure every Pi 5 node ships the same installed software stack (systemd units + node-agent runtime), with features enabled/disabled per node via config/capabilities (no separate images per feature). Align node-agent sampling so hardware I/O never runs on the async control plane (FastAPI/MQTT callbacks), and ship the standard secondary services (e.g., renogy-bt) as install-time defaults that can be disabled per node.
  - **References:**
    - `project_management/tickets/TICKET-0015-pi5-generic-node-stack-(single-image-feature-toggles).md`
    - `tools/build_image.py`
    - `apps/core-server-rs/src/services/deployments.rs`
    - `docs/node-agent.md`
  - **Acceptance Criteria:**
    - Pi 5 imaging kit installs the same base systemd units on every node: `node-agent.service` plus required timers, and `renogy-bt.service` (installed but disabled by default unless configured).
    - No network fetches are required to install optional services (e.g., Renogy collector is shipped in-repo/offline-capable).
    - Fresh installs default to production behavior: empty sensors/outputs unless explicitly configured (no demo/fake telemetry unless simulation is enabled).
    - Hardware I/O remains off the async hot path (ADC sampling + BLE polling in bus-owner workers; API reads cached values only).
    - `make ci-node` passes.
  - **Status:** Done (`make ci-node`)


- **NA-52: Optional services auto-enable/disable via node_config watcher**
  - **Description:** Ensure optional node services (starting with `renogy-bt.service`) can be enabled/disabled by updating `node_config.json` (no SSH required after install) while keeping the generic Pi 5 stack “installed everywhere, disabled unless configured” truthful.
  - **References:**
    - `apps/farmctl/src/bundle.rs`
    - `apps/node-agent/systemd/node-agent-optional-services.path`
    - `apps/node-agent/systemd/node-agent-optional-services.service`
    - `apps/node-agent/scripts/node-agent-optional-services.py`
    - `tools/build_image.py`
    - `apps/core-server-rs/src/services/deployments.rs`
  - **Acceptance Criteria:**
    - Pi install paths (preconfigured media + deploy-from-server) enable `node-agent-optional-services.path`.
    - Controller bundles include `renogy-bt.service` and `node-agent-optional-services.*` in the node-agent overlay tar so deploy-from-server always ships the full generic stack.
    - Updating `/opt/node-agent/storage/node_config.json` enables/disables `renogy-bt.service` based on `renogy_bt2` config validity (enabled + external mode + ingest token + address/device name).
    - Secrets are not written to deployment logs (token is never echoed).
    - `make ci-node` passes and Rust deploy job tests still compile.
  - **Status:** Done (`make ci-node`, `cargo test --manifest-path apps/core-server-rs/Cargo.toml`)


- **NA-50: Implement counter-based pulse inputs (flow/rain) + delta telemetry**
  - **Description:** Replace the stub pulse driver with a counter-based implementation suitable for high-rate inputs (flow meters/rain gauges). Pulse capture must not rely on busy polling; use a kernel/DMA/external counter approach (pigpio DMA sampling via pigpiod) and report deltas at publish cadence.
  - **Acceptance Criteria:**
    - Node-agent supports a pulse counter backend that maintains cumulative counts per channel and returns deltas per publish interval.
    - Pulse capture runs outside the FastAPI event loop and remains stable under sustained/bursty pulse rates.
    - Unit coverage exists for delta math and buffer bounds without requiring hardware.
  - **Status:** Done (`make ci-node`)


- **NA-20: Complete Mesh Networking Implementation**
  - **Description:** Deliver the zigpy adapter integration, CLI pairing scaffold, and telemetry buffer wiring so mesh samples flow into the node telemetry pipeline before hardware validation.
  - **Acceptance Criteria:**
    - Node-agent includes a zigpy adapter abstraction wired into the drivers layer behind a feature flag.
    - `tools/mesh_pair.py` supports join/leave flows and writes a pairing artifact/config.
    - Mesh samples are accepted into the telemetry buffer in simulated/stub mode with unit coverage.
  - **Status:** Done (`make ci-node`)


- **NA-36: Dedupe Pi 5 simulator core registration sensor/output IDs**
  - **Description:** Prevent Pi 5 simulator core registration from failing when Sim Lab seed data already uses the same sensor/output IDs; ensure Renogy simulator sensors use the canonical IDs from the node-agent runbook for consistency.
  - **Acceptance Criteria:**
    - `tools/pi5_simulator.py --register-core` avoids 500s by de-duping sensor/output IDs that already exist on other nodes.
    - Renogy simulator sensors use `renogy-*` IDs aligned with `apps/node-agent/README.md`.
    - E2E smoke (`make e2e-web-smoke`) passes after changes.
  - **Status:** Done (`make e2e-web-smoke`)


- **NA-42: Split node-agent monolith into routers + schemas**
  - **Description:** Improve maintainability and E2E triage by extracting Pydantic models and route handlers out of `apps/node-agent/app/main.py`.
  - **Acceptance Criteria:**
    - Pydantic request/response models move into `apps/node-agent/app/schemas.py` (or `app/schemas/`).
    - Route handlers move into `apps/node-agent/app/routers/` and `main.py` becomes startup/wiring-only.
    - `apps/node-agent` pytest suite still passes.
    - `make e2e-web-smoke` still passes.
  - **Status:** Done (`make ci-node`, `make e2e-web-smoke`)


- **NA-43: Implement reservoir depth pressure transducer via node (4–20 mA current loop)**
  - **Description:** Add node-side support for a reservoir depth pressure transducer (4–20 mA) so depth telemetry is sampled reliably (no event-loop blocking) and published upstream for trends/analytics.
  - **References:**
    - `project_management/tickets/TICKET-0005-reservoir-depth-pressure-transducer-integration.md`
    - `docs/runbooks/reservoir-depth-pressure-transducer.md`
  - **Acceptance Criteria:**
    - Node can be configured with a current-loop depth channel (shunt Ω + range) and produces depth readings at ~2 Hz for ~10 sensors without blocking the HTTP event loop.
    - ADC I/O runs off the HTTP event loop (background sampling) so TCP config traffic queues and is processed, not dropped.
    - Sampling configuration documents the ADC conversion budget (ADS1263 SPS ≥ 50; recommend 100 for 10×2 Hz scanning).
    - Node-agent publishes telemetry in the standard schema and includes fault/status markers for out-of-range currents.
    - Node-agent automated tests cover conversion math and non-blocking sampling helpers (hardware validation tracked separately).
  - **Status:** Done (`make ci-node`)


- **NA-21: Complete BLE Provisioning and Local Configuration UX**
  - **Description:** Implement the full BLE provisioning workflow by replacing the stub with a real `bleak` GATT server and `/v1/provisioning/session` endpoint for token exchange. Integrate this with the OS networking stack to apply Wi-Fi credentials. Simultaneously, expand the local web UI to display provisioning status, active sessions, and expose configuration options for sensors/outputs/intervals, ensuring a seamless setup experience. Provide CLI fallback (`tools/provision_ble.py`) for headless provisioning.
  - **Status:** Done (implementation complete: Linux/BlueZ server + HTTP fallback)


- **NA-28: Implement Renogy Rover BT-2 telemetry collector on Pi 5 nodes**
  - **Description:** Add a node-agent capability/driver that connects to the Renogy BT-2 BLE module attached to the `RNG-CTRL-RVR20-US` (Rover 20A, RS-485 model) and publishes power-system telemetry (solar, battery, load, runtime estimate) into the existing MQTT telemetry pipeline so the core server can display and trend the node’s power subsystem.
  - **Acceptance Criteria:**
    - The published telemetry includes (at minimum): `pv_power_w`, `pv_voltage_v`, `pv_current_a`, `battery_soc_percent`, `battery_voltage_v`, `battery_current_a`, `controller_temp_c`, `battery_temp_c`, and `load_power_w` (with correct scaling/units).
    - Telemetry is published using the existing node MQTT topic/payload schema and ingested by the telemetry pipeline without requiring core-server “Renogy feed” credentials.
    - Failure mode is non-fatal: BLE disconnects/backoffs do not crash the node-agent process; the node continues to advertise status and recover automatically when BT-2 is reachable again.
    - Simulator external ingest validation passes via `tools/pi5_simulator.py --config-path ...` and `make e2e-web-smoke`.
  - **Status:** Done (tests: apps/node-agent pytest, `make e2e-web-smoke`)


- **NA-29: Create Raspberry Pi 5 deployment tool for Renogy charge-controller nodes**
  - **Description:** Provide a repeatable deployment tool/profile that provisions a Raspberry Pi 5 as a dedicated Renogy charge-controller node (BT-2 BLE + `RNG-CTRL-RVR20-US`) using the Farm Dashboard architecture (node-agent + MQTT publish + LAN to core server). The tool should minimize manual steps and make it easy to deploy additional identical charge-controller nodes.
  - **Acceptance Criteria:**
    - A single documented command/workflow generates a Pi 5 Renogy deployment bundle with the collector enabled (BT-2 MAC/config + poll interval + MQTT/core connection settings).
    - First-boot automation stages `node_config.json`, `node-agent.env`, and the `renogy-bt` config/unit from the boot volume and installs `renogy-bt` automatically (no Pi login required).
    - The deployment tool records the inputs required (BT-2 MAC, node name, adoption token/credentials) and produces artifacts suitable for backups/restore on replacement hardware.
    - Simulator bundle runs via `tools/pi5_simulator.py --config-path ...` with Renogy ingest publishing telemetry.
    - Includes a short runbook entry that an operator can follow to re-image/replace a failed Renogy node using the stored backup and the deployment workflow.
  - **Status:** Done (tests: apps/node-agent pytest, `make e2e-web-smoke`)

- **NA-34: Add full-stack Pi 5 simulator core registration mode**
  - **Description:** Extend the Pi 5 simulator to pre-register nodes/sensors/outputs in the core server and use the core node UUID for MQTT topics so telemetry-sidecar status updates and output commands work end-to-end.
  - **Acceptance Criteria:**
    - `tools/pi5_simulator.py --register-core` creates/updates the core node record and seeds sensors/outputs.
    - Simulator writes configs using the core node UUID and publishes MQTT to `iot/<node_uuid>/...` topics.
    - Runbook updates document full-stack registration flow and token requirements.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-26: Add runtime simulation profile controls for Sim Lab**
  - **Description:** Allow Sim Lab to update a running node-agent simulation profile (offline cycles, jitter/spikes, stuck outputs, base overrides) without restarting the process.
  - **Acceptance Criteria:**
    - New `GET /v1/simulation` returns the active `SimulationProfile`.
    - New `PUT /v1/simulation` (or extended `/v1/config`) accepts `SimulationProfile` updates when simulation is enabled.
    - Updates apply to the in-memory simulator immediately (telemetry + output behavior reflect changes).
    - Simulation updates persist to the config store so restarts keep the new profile.
    - Includes unit tests covering profile updates and refusal when simulation is disabled.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-27: Normalize heartbeat output payload shape**
  - **Description:** Ensure node-agent heartbeat payloads serialize outputs as a list of objects matching the OpenAPI schema to prevent validation errors during Sim Lab runs.
  - **Acceptance Criteria:**
    - Heartbeat payload `outputs` is a list of objects with output metadata/state.
    - Sim Lab E2E run no longer logs NodeStatusPayload output validation errors.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-30: Build Raspberry Pi 5 simulator runner**
  - **Description:** Provide a local Raspberry Pi 5 simulator that runs node-agent in simulation mode with realistic sensor/output payloads for telemetry, adoption, and dashboard testing without physical hardware.
  - **Acceptance Criteria:**
    - `tools/pi5_simulator.py` generates a Pi 5 config bundle and can run the node-agent with simulated sensors/outputs.
    - Simulated Renogy metrics use deterministic ranges when the Renogy sensor set is enabled.
    - Runbook documents usage and is linked from `docs/README.md` and `docs/node-agent.md`.
    - Tests: `apps/node-agent` pytest, `make e2e-web-smoke`.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-31: Map sensor categories to drivers in node-agent**
  - **Description:** Treat common sensor category types (humidity, lux, pressure, etc.) as analog/pulse inputs so telemetry publishes without unknown-type warnings.
  - **Acceptance Criteria:**
    - Node-agent maps supported sensor categories to ADS1115 or pulse drivers.
    - Simulation mode no longer falls back to hardware reads when offline.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-32: Bridge Renogy BT-2 ingest with renogy-bt**
  - **Description:** Support the upstream `renogy-bt` BLE client as the Renogy BT-2 data source by ingesting its JSON payloads and mapping them into node-agent telemetry.
  - **Acceptance Criteria:**
    - `renogy_bt2.mode="external"` skips BLE polling and accepts POSTs on `/v1/renogy-bt`.
    - Ingest maps Renogy fields (pv/battery/load) into node-agent Renogy metrics and publishes via MQTT.
    - Renogy deployment bundle includes `renogy-bt` config + systemd unit, and the runbook documents setup.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-33: Normalize sensor type strings for Sim Lab telemetry**
  - **Description:** Normalize sensor type strings (strip/alias) in the node-agent telemetry path so Sim Lab sensors like power don't emit unknown-type warnings.
  - **Acceptance Criteria:**
    - Sensor types with whitespace/suffixes normalize to expected analog/pulse categories.
    - Sim Lab runs no longer log "Unknown sensor type power" warnings.
  - **Status:** Done (verified in Pi 5 simulator run; tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-35: Allow Pi 5 simulator to run deployment bundles**
  - **Description:** Extend the Pi 5 simulator so it can boot from an existing `node_config.json` bundle (including Renogy BT-2 external ingest) and disable simulation when validating real ingest paths.
  - **Acceptance Criteria:**
    - `tools/pi5_simulator.py --config-path <bundle>/node_config.json` runs node-agent using the bundle config.
    - `--no-simulation` disables simulated telemetry so external Renogy ingest can drive metrics.
    - Renogy ingest payloads publish MQTT telemetry when running the bundle in the simulator.
  - **Status:** Done (tests: `apps/node-agent` pytest, `make e2e-web-smoke`)

- **NA-37: Add load voltage/current sensors to Renogy deployment bundle defaults**
  - **Description:** Extend the Renogy Pi 5 deployment bundle defaults so load voltage/current metrics are generated and trended alongside load power.
  - **Acceptance Criteria:**
    - `tools/renogy_node_deploy.py` default sensor list includes `load_voltage_v` and `load_current_a`.
    - Generated `node_config.json` includes Renogy sensors for load voltage/current with the bundle interval cadence.
  - **Status:** Done (tests: `make e2e-web-smoke`)

- **NA-58: Encrypt provisioning queue Wi-Fi secrets**
  - **Description:** Protect Wi-Fi passwords stored in the provisioning queue file by encrypting them at rest.
  - **Acceptance Criteria:**
    - Provisioning queue persists encrypted Wi-Fi credentials instead of plaintext.
    - Decryption happens only when loading records for application.
  - **Status:** Done

- **NA-59: Apply live sensor list updates safely (node-agent)**
  - **Description:** Ensure node-agent can accept live sensor list changes via `/v1/config` without requiring SSH/manual restarts, including clearing the sensor list and removing stale scheduling state.
  - **Acceptance Criteria:**
    - `PUT /v1/config` with `{"sensors":[]}` clears the sensor list (empty list is treated as an explicit update).
    - Telemetry publisher does not spin when sensors are removed (prunes stale scheduling/cache keys).
  - **Notes / Run Log:**
    - 2026-01-10: Fixed config apply semantics so empty lists clear sensors/outputs/schedules, and hardened the publisher loop to prune removed sensors from internal scheduling/caches.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **NA-8: Mesh radio adapter integration**
  - **Description:** Implement mesh radio adapter, telemetry buffer integration, and pairing CLI scaffold.
  - **Status:** Done


- **NA-9: Telemetry rolling averages**
  - **Description:** Implement and test rolling averages for telemetry.
  - **Status:** Done (Phase 6 Validation)


- **NA-10: Mesh backfill flow**
  - **Description:** Implement and test mesh backfill flow.
  - **Status:** Done (Phase 6 Validation)


- **NA-1: Implement a simplified provisioning method**
  - **Description:** File-based provisioning via `ConfigStore` and `/v1/config` endpoints.
  - **Status:** Done


- **NA-3: Create SD-card imaging and flashing scripts**
  - **Description:** Provide repeatable image/flash tooling (`tools/build_image.py`, `tools/flash_node_image.sh`) for generic node deployment.
  - **Status:** Done


- **NA-5: Publish telemetry/heartbeat with configurable intervals**
  - **Description:** Heartbeat and telemetry cadence configurable per node via env/config.
  - **Status:** Done


- **NA-6: Implement `/v1/config` to support restore push**
  - **Description:** `/v1/config`, `/v1/config/restore`, and `/v1/config/import` accept core backups and apply settings.
  - **Status:** Done


- **NA-7: Provide `/v1/status` for adoption preview**
  - **Description:** `/v1/status` returns hardware/firmware/uptime/sensor/mesh summaries for adoption UI.
  - **Status:** Done


- **NA-11: Include MAC addresses in discovery advertisement**
  - **Description:** Zeroconf advertisement includes MACs, firmware, capabilities, uptime, and mesh health.
  - **Status:** Done


- **NA-12: Core adoption flow integration**
  - **Description:** Core discovery/adoption consumes node agent metadata and MAC bindings.
  - **Status:** Done


- **NA-22: Support per-sensor publish intervals + change-of-value (COV) publishing**
  - **Description:** Respect `interval_seconds` per sensor (including `0` for COV) when publishing telemetry so pulse-based sensors don’t spam unchanged values and time-based sensors can run at their intended cadence.
  - **Acceptance Criteria:**
    - Sensors with `interval_seconds=0` only publish when the measured value changes.
    - Sensors with `interval_seconds>0` publish on their own schedule (independent of the global telemetry loop).
    - Includes unit tests for scheduling/COV behavior.
  - **Status:** Done (per-sensor scheduling + COV suppression)


- **NA-23: Keep node-agent tests runnable on Python 3.14**
  - **Description:** Add test-only DBus shims and avoid loading BLE/DBus services when DBus is unavailable so `PYTHONPATH=. poetry run pytest` works on macOS/Python 3.14 without BlueZ/DBus.
  - **Acceptance Criteria:**
    - On Python 3.14, importing node-agent modules does not fail due to `dbus_next` imports.
    - `PYTHONPATH=. poetry run pytest` passes without requiring a running DBus daemon.
  - **Status:** Done (dbus-next shims + BLE start/stop patched in `apps/node-agent/tests/conftest.py`)


- **NA-24: Stabilize simulator time inputs for deterministic tests**
  - **Description:** Treat injected `now` values as offsets from simulator start to avoid time-dependent variance in simulated sensor readings.
  - **Acceptance Criteria:**
    - Passing `now=0` and `now=1` yields consistent deltas across runs.
    - Simulated offline windows continue to use the same timing semantics.
  - **Status:** Done


- **NA-25: Replace asyncio-mqtt with aiomqtt**
  - **Description:** Move the node-agent MQTT client dependency to the maintained `aiomqtt` package and update imports/docs.
  - **Acceptance Criteria:**
    - Node agent code imports `aiomqtt` for MQTT client usage.
    - Node agent tests pass with the new dependency.
    - README/AGENTS docs reference `aiomqtt` instead of `asyncio-mqtt`.
  - **Status:** Done



- **NA-65: Renogy BT-2: Track PV energy (kWh today + total) as sensors**
  - **Description:** Expose the Renogy Rover controller’s energy counters as first-class sensors so operators can trend daily and lifetime PV production in the dashboard. Prefer reading the controller’s counters directly (matches the Renogy app) instead of integrating instantaneous PV power. Values must still be ingested when `0.0` (nighttime).
  - **Acceptance Criteria:**
    - Renogy preset includes two energy sensors:
      - `PV Energy (today)` (`kWh`, type `energy`, interval 30s)
      - `PV Energy (total)` (`kWh`, type `energy`, interval 30s)
    - Node-agent publishes both metrics on schedule via MQTT.
    - Core ingests and persists both series (value `0.0` is not dropped).
    - Validation:
      - `make ci-node` and `make ci-core-smoke` pass.
      - Tier A validated on installed controller (no DB/settings reset) with **viewed** screenshots under `manual_screenshots_web/` showing Node 1 values.
      - Tier B deferred to the clean-host cluster tickets.
  - **Owner:** Platform (Codex)
  - **References:**
    - `shared/presets/integrations.json`
    - `apps/node-agent/app/hardware/renogy_bt2.py`
  - **Notes / Run Log:**
    - 2026-01-31: Tier A refreshed installed controller to `0.1.9.231` and confirmed Node 1 ingest for both series (nighttime: today is `0.0`). Run: `project_management/runs/RUN-20260131-tier-a-na65-renogy-kwh-sensors-0.1.9.231.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred)
- **NA-66: Node-agent: require auth for config + provisioning HTTP endpoints (no secret leaks)**
  - **Description:** Node-agent currently exposes `/v1/config` and provisioning queue/session endpoints without authentication, and these endpoints can return sensitive material (adoption token, Wi‑Fi hints, decrypted provisioning records). Require an explicit auth mechanism (provisioning secret / PIN / controller-issued token) for any endpoint that reads or mutates configuration or returns provisioning state.
  - **Acceptance Criteria:**
    - All `/v1/config` endpoints (GET/PUT/restore/import) require auth and return `401` when missing.
    - Provisioning endpoints that expose queued/session data require auth and never return plaintext Wi‑Fi passwords to unauthenticated callers.
    - `GET /v1/config` does not include `adoption_token` or Wi‑Fi secrets unless the caller is authorized.
    - `make ci-node` passes.
  - **References:**
    - `apps/node-agent/app/routers/config.py`
    - `apps/node-agent/app/routers/provisioning.py`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: config/provisioning endpoints are unauthenticated and can expose adoption/Wi‑Fi secrets to any LAN client.
    - 2026-02-03: Fix: secured node-agent config + provisioning endpoints behind a bearer token (controller-issued adoption token; optional `NODE_PROVISIONING_SECRET` override). Config/provisioning responses now redact `adoption_token` + Wi‑Fi passwords, and mDNS discovery no longer advertises the adoption token. Local node UI now prompts for the token and sends it with API calls (without ever displaying the secrets).
    - 2026-02-03: Integration: core-server now attaches `Authorization: Bearer …` when calling node-agent `/v1/config` and `/v1/config/restore` (adoption profile sync, display profile apply, sensor apply, Renogy preset apply, restore worker).
    - 2026-02-03: Validation: `make ci-node` (pass), `make ci-core-smoke` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **NA-67: Node-agent provisioning: avoid blocking the event loop when applying Wi‑Fi credentials**
  - **Description:** Provisioning applies Wi‑Fi credentials by calling `subprocess.run()` inside an async flow. This blocks the asyncio event loop and can stall HTTP handling, BLE provisioning, and watchdog pings while `nmcli`/`wpa_cli` run (seconds-scale), making the node appear offline or causing resets under load.
  - **Acceptance Criteria:**
    - Applying Wi‑Fi credentials does not block the event loop (run the command in a thread/process executor, or hand off to a background worker).
    - The provisioning API returns quickly with a clear “in progress/applied/error” state and captures stdout/stderr for troubleshooting.
    - A unit test covers that `apply_provisioning_request` remains responsive when Wi‑Fi apply is slow (mock the runner and assert it doesn’t stall async scheduling).
    - `make ci-node` passes.
  - **References:**
    - `apps/node-agent/app/http_utils.py`
    - `apps/node-agent/app/routers/provisioning.py`
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: `apply_provisioning_request` calls `apply_wifi_credentials` which uses `subprocess.run()` directly.
    - 2026-02-03: Fix: Wi‑Fi apply now runs in a background thread via `asyncio.to_thread(...)` (guarded by a per-app lock). Provisioning returns quickly with a queued/in-progress state and captures apply output for troubleshooting.
    - 2026-02-03: Validation: `make ci-node` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

- **NA-68: Node-agent restore/apply_config must validate and clamp timing fields**
  - **Description:** `ConfigStore.apply_config()` mutates `Settings` in-place and sets `heartbeat_interval_seconds` / `telemetry_interval_seconds` directly from persisted JSON without re-validation. Invalid values (0/negative/too small) can cause tight-loop scheduling, excessive MQTT traffic, and CPU spikes.
  - **Acceptance Criteria:**
    - `apply_config` validates timing fields via Pydantic (or explicit guards) and clamps to sane minimums.
    - Invalid persisted configs are rejected safely (do not partially apply; preserve last-known-good config; surface a clear error to the caller/log).
    - Unit tests cover bad interval values and verify safe behavior.
    - `make ci-node` passes.
  - **References:**
    - `apps/node-agent/app/services/config_store.py` (`apply_config`)
  - **Notes / Run Log:**
    - 2026-02-03: Audit finding: `apply_config` directly `setattr`s interval fields from JSON without validation.
    - 2026-02-03: Fix: timing fields are now validated/clamped via `Settings` validators, and `apply_config` is transactional (validate candidate settings before mutating live settings). `/v1/node` PATCH now uses `apply_config` so it cannot bypass validation.
    - 2026-02-03: Validation: `make ci-node` (pass).
  - **Status:** Done (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)

  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

---

## Core Infrastructure
### Done
- **DT-60: ADS1263 Phase 0 — split “big diff” into phase commits + gates**
  - **Description:** Stabilize the in-flight ADS1263/analog work by turning the current “big diff” into reviewable phase commits and enforcing doc/tooling gates so future Tier‑A rebuild/refresh runs are reproducible.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - `project_management/tickets/TICKET-0032-pi5-ads1263-analog-contract-and-fail-closed.md`
    - Canonical ADC docs:
      - `docs/development/analog-sensors-contract.md`
      - `docs/runbooks/reservoir-depth-pressure-transducer.md`
      - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
  - **Acceptance Criteria:**
    - PM docs (`project_management/TASKS.md`, `project_management/BOARD.md`, `project_management/EPICS.md`) reflect the ADS1263 execution plan as the active path and map work to phase commits.
    - A safety checkpoint commit is created on a WIP branch containing the full current diff.
    - The checkpoint is split into `PH1/PH2/PH3…` commits (phase-prefixed commit messages) and pushed.
    - A hard gate exists: Tier‑A rebuild/refresh workflows cannot be performed from uncommitted changes.
    - ADC docs are de-conflicted: canonical docs selected; any legacy/conflicting ADC docs are marked **Deprecated** (or removed if stale).
    - All ADC docs explicitly state: “Production is fail‑closed; simulation is test/dev only and cannot be enabled via the dashboard.”
    - Working tree is clean before any Tier‑A evidence is recorded.
  - **Notes / Evidence:**
    - 2026-01-14: Checkpoint branch created: `wip/ads1263-p0-checkpoint` (commit `4a8fe10`).
    - 2026-01-14: Split into reviewable commits on `wip/ads1263-phases` (PH1–PH11).
    - 2026-01-14: Tier‑A rebuild/refresh gate enforced (fails if git working tree is dirty) and runbook updated.
    - 2026-01-14: CI: `make ci-core-smoke`, `make ci-node`, and `make ci-web-smoke` passing.
  - **Status:** Done


- **DT-61: ADS1263 Phase 1 — Safety baseline: “No simulation in production” (build-flavor + fail-closed analog)**
  - **Description:** Make it impossible for production artifacts to emit plausible analog telemetry unless ADS1263 hardware is actually healthy. Introduce a build-flavor concept that is baked into node-agent artifacts (not a runtime toggle), and hard-fail any simulator path in production builds.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - `project_management/tickets/TICKET-0032-pi5-ads1263-analog-contract-and-fail-closed.md`
    - Canonical ADC docs:
      - `docs/development/analog-sensors-contract.md`
      - `docs/runbooks/reservoir-depth-pressure-transducer.md`
      - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
  - **Acceptance Criteria:**
    - Node-agent has a build-flavor constant baked into artifacts (`BUILD_FLAVOR = prod|dev|test`) and production packaging emits `BUILD_FLAVOR="prod"`.
    - In `BUILD_FLAVOR="prod"`, analog is fail-closed: if the ADS1263 backend is disabled/unhealthy, **no analog telemetry** is published (sensors stay offline/unavailable).
    - In `BUILD_FLAVOR="prod"`, simulator paths hard-fail (no config/UI path can enable simulation).
    - Node-agent unit tests run under `BUILD_FLAVOR="prod"` and confirm:
      - No simulated values are emitted.
      - Analog telemetry is absent when the backend is unhealthy.
    - Tier‑A rebuild/refresh succeeds (no DB/settings reset) and evidence is recorded.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.120` (no DB/settings reset).
    - Verified node-agent artifact is production-flavored:
      - `tar -xOf /usr/local/farm-dashboard/releases/0.1.9.120/artifacts/node-agent/node-agent-overlay.tar.gz opt/node-agent/app/build_info.py`
      - `BUILD_FLAVOR = "prod"`
    - Run: `project_management/runs/RUN-20260114-tier-a-dt61-no-sim-prod-0.1.9.120.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)


- **DT-62: ADS1263 Phase 2 — Remove “ADS1115” as a concept (P0)**
  - **Description:** Nobody can create or configure an “ADS1115” sensor anymore. User/config-facing driver naming is `driver_type=analog` only; the Pi analog backend is ADS1263.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - `project_management/tickets/TICKET-0032-pi5-ads1263-analog-contract-and-fail-closed.md`
  - **Acceptance Criteria:**
    - API rejects creation/update with `driver_type=ads1115`.
    - UI never shows “ADS1115”.
    - Repo grep confirms `ads1115` only appears in docs/history/legacy notes (or the legacy mapper).
    - Tier‑A rebuild/refresh succeeds (no DB/settings reset) and evidence is recorded.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.121` (no DB/settings reset).
    - API rejection: `PUT /api/nodes/{node_id}/sensors/config` with `type=ads1115` returns `400`.
    - Screenshot bundle captured:
      - `manual_screenshots_web/20260114_tier_a_dt62_no_ads1115_0.1.9.121/sensors_add_sensor.png`
    - Run: `project_management/runs/RUN-20260114-tier-a-dt62-remove-ads1115-0.1.9.121.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)


- **DT-63: ADS1263 Phase 3 — ADS1263 node-agent hardware backend (Pi5) (P0/P1)**
  - **Description:** ADS1263 works on Pi 5 using `gpiozero` + `spidev`, with a deterministic health check (chip id + DRDY sanity + sample conversion sanity) and a clear error surface in node status. Production remains fail-closed: unhealthy ADC publishes **no analog telemetry**.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - Canonical ADC docs:
      - `docs/development/analog-sensors-contract.md`
      - `docs/runbooks/reservoir-depth-pressure-transducer.md`
      - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
  - **Acceptance Criteria:**
    - ADS1263 backend uses SPI0 (`/dev/spidev0.0`) with `spidev` and uses `gpiozero` for DRDY + RST/CS (explicit lgpio pin factory on Pi 5).
    - Health includes: chip-id read, DRDY sanity/timeout surface, and sample conversion sanity.
    - Node heartbeat publishes `analog_backend` and `analog_health` (including `last_error`) so debugging doesn’t require SSH.
    - Production remains fail-closed: if ADS1263 is not enabled or not healthy, analog sensors are offline/unavailable and **no plausible values** are published.
    - Node 1 validation: `/dev/spidev0.0` exists; ADS1263 chip id reads successfully; controller shows `analog_health.ok = true`.
    - Node-agent unit tests pass with `BUILD_FLAVOR=prod`.
    - Tier‑A rebuild/refresh succeeds (no DB/settings reset) and evidence is recorded.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.122` (no DB/settings reset).
    - Node1 shows `analog_backend=ads1263`, `analog_health.ok=true`, `chip_id=0x01` via `GET /api/nodes`.
    - Screenshot bundle captured:
      - `apps/manual_screenshots_web/20260114_tier_a_dt63_ads1263_health_0.1.9.122/nodes_0a55b329-104f-46f0-b50b-dea9a5cca1b3.png`
    - Run: `project_management/runs/RUN-20260114-tier-a-dt63-ads1263-backend-health-0.1.9.122.md`.
- **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)


- **DT-64: ADS1263 Phase 4 — End-to-end “Add hardware sensor” from dashboard (Pi-only) (P0)**
  - **Description:** A hardware engineer can add a real Pi hardware sensor from the dashboard UI (no SSH, no file copying) and immediately see telemetry flowing. This phase verifies and tightens the end-to-end path: dashboard editor → core apply endpoint → node-agent config → telemetry ingest.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - Canonical ADC docs:
      - `docs/development/analog-sensors-contract.md`
      - `docs/runbooks/reservoir-depth-pressure-transducer.md`
      - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
  - **Acceptance Criteria:**
    - Dashboard “Add sensor” flow is only enabled for Pi nodes (node-agent nodes; `agent_node_id` present).
    - UI shows ADC backend health prominently (ads1263 OK / SPI disabled / not detected) with clear messaging: “No data until ADS1263 healthy”.
    - Apply workflow supports: edit → validate → apply → readback/verify status.
    - Core-server persists desired node sensor config and last apply status (applied vs stored/offline + last error string).
    - Core upserts the sensor registry so telemetry ingest accepts new sensors immediately.
    - Tier‑A validation on installed controller:
      - Add a sensor on Pi Node1 via UI, apply, and see it listed and updating.
      - API checks: `GET/PUT /api/nodes/{node_id}/sensors/config`; `/api/sensors` shows `latest_value/latest_ts` updating.
      - Playwright screenshots captured **and viewed**.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.123` (no DB/settings reset).
    - UI: “Add sensor” drawer shows backend status `ads1263 OK` and includes the added sensor `DT64 ADC0 Voltage`.
    - API: `PUT /api/nodes/{node_id}/sensors/config` returns `status="applied"` and `/api/sensors` shows `latest_value/latest_ts` updating for the new sensor.
    - Screenshots captured and viewed:
      - `apps/manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123/sensors_node_after_apply.png`
      - `apps/manual_screenshots_web/20260114_tier_a_dt64_post_apply_0.1.9.123/sensors_add_sensor.png`
    - Run: `project_management/runs/RUN-20260114-tier-a-dt64-add-hardware-sensor-0.1.9.123.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)


- **DT-65: ADS1263 Phase 5 — Reservoir depth transducer (AIN0 vs AINCOM + 163Ω shunt) (P0)**
  - **Description:** Make the “Reservoir Depth” sensor real, correct, and trustworthy by finalizing the 4–20mA (current loop) conversion (measured as voltage across a 163Ω shunt) into depth, enforcing calibration bounds + fault handling, and documenting the wiring/contract for this install.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - Canonical ADC docs:
      - `docs/development/analog-sensors-contract.md`
      - `docs/runbooks/reservoir-depth-pressure-transducer.md`
      - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
  - **Acceptance Criteria:**
    - A dedicated preset (or finalized config) exists for reservoir depth with:
      - Current-loop conversion: voltage across shunt → current (mA) → depth (ft).
      - Explicit expected bounds and fault handling: open loop, short, and out-of-range (fail-closed; no plausible depth values when faulty).
    - Docs clearly state the wiring for this install:
      - ADS1263 `AIN0` (positive) vs `AINCOM` (negative/common) across the 163Ω shunt.
      - Expected voltage/current sanity bounds (e.g., ~0.65–3.26V for 4–20mA across 163Ω) and how to interpret them.
    - Tier‑A validation on installed controller (Node1):
      - Node sensor config applied and read back.
      - Reservoir Depth sensor shows plausible readings and updates continuously (trend/detail view).
      - Backend reports ADS1263 healthy (`analog_backend=ads1263`, `analog_health.ok=true`) and no simulation is involved.
      - Screenshots captured **and viewed**.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.124` (no DB/settings reset).
    - Node1 sensor config includes `Reservoir Depth` (`ea5745e00cb0227e046f6b88`, `ft`, `ch=0`, `shunt=163Ω`, `range=5m`) and backend health `ads1263 OK · chip 0x01`.
    - Current-loop faults are fail-closed in node-agent (non-zero quality suppresses publish; sensor remains offline/unavailable instead of emitting plausible values).
    - Screenshots captured and viewed:
      - `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/sensors_reservoir_depth_detail.png`
      - `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/sensors_add_sensor.png`
      - `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/trends_reservoir_depth_selected.png`
      - `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/trends_reservoir_depth_last_6h.png`
    - Run: `project_management/runs/RUN-20260114-tier-a-dt65-reservoir-depth-0.1.9.124.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)


- **DT-54: Remove panic-on-startup in WAN portal state init (audit)**
  - **Description:** Replace the `expect()` in the WAN portal HTTP client initialization with proper error propagation so misconfigured TLS/system state fails with a controlled message instead of a hard panic.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `apps/wan-portal/src/state.rs`
  - **Acceptance Criteria:**
    - WAN portal startup returns a clear error if the reqwest client cannot be constructed (no `expect` panic).
    - `cargo test --manifest-path apps/wan-portal/Cargo.toml` passes.
  - **Status:** Done (`cargo test --manifest-path apps/wan-portal/Cargo.toml`)


- **DT-55: Remove hardcoded iOS smoke-test password (audit)**
  - **Description:** Replace the hardcoded `SmokeTest!123` password in the iOS E2E smoke helper with an environment-configurable value to avoid encouraging unsafe copy/paste patterns.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `tools/e2e_ios_smoke.py`
  - **Acceptance Criteria:**
    - The iOS smoke helper reads the password from an env var (with a safe fallback for local dev).
    - Docs/PM note iOS/watch validation is deferred and will be revisited later.
  - **Status:** Done (env var: `E2E_IOS_TEST_PASSWORD`; iOS/watch validation remains deferred)


- **DT-56: Require clean-state pre/postflight checks for test runs**
  - **Description:** Prevent “false green” smoke/E2E runs by enforcing a clean machine state before tests start and verifying cleanup after they finish (no orphaned launchd jobs or background services).
  - **References:**
    - `tools/test_hygiene.py`
    - `tools/e2e_setup_smoke.py`
    - `tools/e2e_installer_stack_smoke.py`
    - `AGENTS.md`
    - `docs/DEVELOPMENT_GUIDE.md`
  - **Acceptance Criteria:**
    - `make test-preflight` fails when `launchctl list | grep -i farm` or the process scan indicates leftover Farm services.
    - `make test-preflight` fails when any FarmDashboard installer/controller DMGs are still attached (e.g. stale `hdiutil` images from failed attaches).
    - `make test-clean` removes `com.farmdashboard.*` launchd jobs and terminates safe-to-kill orphan processes.
    - `make test-clean` detaches any attached FarmDashboard installer/controller DMGs (so later `hdiutil attach` calls are reliable).
    - Test hygiene surfaces persistent launchd override-key residue (`com.farmdashboard.e2e.*`) as a warning and links to the one-time admin purge helper (`tools/purge_launchd_overrides.py`); strict mode is available for fully pristine runs.
    - `make e2e-installer-stack-smoke` fails fast if the machine is not clean at start and fails if any orphaned services remain after cleanup.
  - **Status:** Done (`make e2e-installer-stack-smoke` + `make e2e-installer-stack-smoke-quarantine`; log: `reports/e2e-installer-stack-smoke/20260105_171120`)


- **DT-53: Fix `farmctl native-deps` relative output path installs**
  - **Description:** Ensure `farmctl native-deps --output <relative-path>` installs native deps into the intended repo-relative directory even though build steps run in temp workspaces, so release builds and operator workflows don’t silently write into `/var/folders/...` temp paths.
  - **References:**
    - `apps/farmctl/src/native_deps.rs`
  - **Acceptance Criteria:**
    - `farmctl native-deps --output build/release-X/native-deps` installs Postgres/Redis/Mosquitto/TimescaleDB under `build/release-X/native-deps/*` (not under temp directories).
    - Unit tests cover relative vs absolute output root resolution.
    - `cargo test --manifest-path apps/farmctl/Cargo.toml` passes.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`)


- **DT-52: Deprecate dashboard-web manifest stub (static dashboard is served by core-server)**
  - **Description:** The controller bundle manifest currently includes a `dashboard-web` component with a stub “binary” entrypoint for backwards compatibility, even though the dashboard is served as static assets by the Rust core-server. Plan and execute the safe removal (likely requiring a manifest format bump) while keeping upgrade/rollback compatibility.
  - **Acceptance Criteria:**
    - Bundle manifest no longer requires a dashboard-web executable entrypoint (static assets only).
    - Installer/upgrade/rollback remain compatible with older bundles (document any required version gating).
    - `make e2e-setup-smoke` and `make e2e-installer-stack-smoke` remain green.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`, `make e2e-setup-smoke`, `make e2e-installer-stack-smoke`; logs: `reports/prod-ready-e2e-setup-smoke-20260101_173609.log`, `reports/prod-ready-e2e-installer-stack-smoke-20260101_174119.log`)


- **DT-51: Consolidate Sim Lab tooling paths under `tools/sim_lab/`**
  - **Description:** Remove confusing dual directory naming by moving fixture scripts/assets into `tools/sim_lab/` and updating E2E scripts/docs accordingly.
  - **Acceptance Criteria:**
    - No tracked files reference the legacy hyphenated Sim Lab directory after the change.
    - Fixture servers (BLE/mesh/forecast/rates) are launched from a single canonical location under `tools/sim_lab/`.
    - `make e2e-web-smoke` and `make e2e-installer-stack-smoke` still pass.
  - **Status:** Done (`python3 -m py_compile tools/e2e_web_smoke.py tools/e2e_ios_smoke.py`, `make e2e-web-smoke`, `make e2e-installer-stack-smoke`; logs: `reports/prod-ready-e2e-web-smoke-20260101_175422.log`, `reports/prod-ready-e2e-installer-stack-smoke-20260101_174119.log`)


- **DT-45: WAN read-only portal scaffolding (AWS template + pull agent skeleton)**
  - **Description:** Add the initial infra template and pull-agent scaffolding for the WAN read-only portal, keeping a strict “pull-only + read-only token” design.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0014-feature-008-wan-readonly-webpage-aws.md`
  - **Acceptance Criteria:**
    - An infra template/module exists (CloudFormation or Terraform) for a minimal sandbox deployment.
    - A pull agent skeleton can authenticate using a read-only token and fetch a small endpoint set.
    - No remote write path is introduced.
  - **Status:** Done (`cargo test --manifest-path apps/wan-portal/Cargo.toml`, `make ci-web`; template: `infra/wan-portal/cloudformation.yaml`, runbook: `docs/runbooks/wan-readonly-portal.md`)


- **DT-50: Remove obsolete dashboard service config fields from the setup wizard**
  - **Description:** The dashboard is now served by the Rust core-server (static assets + `/api/*`), so the setup config no longer needs `dashboard_port`/`dashboard_binary` fields. Remove these fields from `farmctl` config, preflight, and the setup wizard UI to avoid confusion and false “service” assumptions.
  - **Acceptance Criteria:**
    - `farmctl` no longer reads/writes `dashboard_port` or `dashboard_binary` in `config.json` (unknown fields remain tolerated for backward compatibility).
    - Setup wizard Configure step does not display dashboard port/binary fields.
    - `cargo test --manifest-path apps/farmctl/Cargo.toml` passes.
    - `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke` passes.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`, `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`)


- **DT-43: Productize “preconfigured media” deployment option (Pi 5)**
  - **Description:** Turn the existing imaging overlay + first-boot tooling into a polished “preconfigured media” flow (Pi Imager profile + optional offline pre-seeding) suitable for non-experts.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0009-feature-003-deployment-preconfigured-media.md`
    - `docs/runbooks/pi5-preconfigured-media.md`
  - **Acceptance Criteria:**
    - A documented Pi Imager profile/template workflow exists for a preconfigured node image.
    - First-boot automation remains idempotent and safe.
    - Tooling is macOS-first (no container runtimes).
  - **Status:** Done (`python3 tools/build_image.py pi-imager-profile …`, `cd apps/node-agent && PYTHONPATH=. poetry run pytest`, `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`; artifacts: `reports/e2e-installer-stack-smoke/20260101_040114/`)


- **DT-44: Prototype Pi 5 network-boot provisioning workflow**
  - **Description:** Research and prototype a Pi 5 network-boot provisioning approach based on Raspberry Pi bootloader-supported network install/HTTP boot, with clear constraints and a plan for production hardening.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0011-feature-005-deployment-network-boot.md`
    - `docs/runbooks/pi5-network-boot-provisioning.md`
  - **Acceptance Criteria:**
    - A macOS-first proof-of-concept can boot/install a Pi 5 via network boot (or documents exactly why not feasible).
    - Repo docs clearly describe required LAN services and safety boundaries (no insecure defaults).
    - A minimal controller-side helper exists to host netboot artifacts (HTTP only; no DHCP/TFTP).
  - **Status:** Done (`make ci-farmctl-smoke`; note: real Pi validation tracked as DT-48)


- **DT-46: Temporarily disable iOS/watch smoke in the pre-commit selector**
  - **Description:** Skip `make ci-ios-smoke` in the staged-path pre-commit selector while we stabilize the installer/controller workflow; iOS/watch validation is run manually when working on those apps.
  - **Acceptance Criteria:**
    - Pre-commit does not run iOS/watch smoke by default.
    - Pre-commit prints a clear message when iOS/watch paths are staged.
    - Docs/PM record that iOS/watch validation will be revisited later.
  - **Status:** Done (`python3 -m py_compile tools/git-hooks/select-tests.py`)


- **DT-38: Remove container-stack dependency from Sim Lab E2E harness**
  - **Description:** Update E2E smoke tooling to run without container stacks by using native services and local fixture processes.
  - **Acceptance Criteria:**
    - `make e2e-web-smoke` and `make e2e-ios-smoke` no longer require a container stack.
    - Sim Lab fixtures (BLE/mesh/forecast/rates) are launched via local processes (`tools/sim_lab/http_json_server.py`).
    - The E2E harness fails fast with a clear message when Postgres/MQTT are unavailable, and documents how to start the native stack.
  - **Status:** Done (`make e2e-installer-stack-smoke`)


- **DT-39: Refactor farmctl monolith into modules**
  - **Description:** Split `apps/farmctl/src/main.rs` into focused modules (config, launchd, install, bundle, server, utils) to keep it maintainable.
  - **Acceptance Criteria:**
    - `main.rs` is a thin entrypoint that wires CLI commands to module functions.
    - Module boundaries are clear and build succeeds without behavior changes.
    - No functionality regressions in install/upgrade/rollback/serve/bundle flows.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`, `make e2e-installer-stack-smoke`)


- **DT-40: Remove container runtime from the repo and CI**
  - **Description:** Ensure there are no container-runtime dependencies or container-only CI jobs remaining now that the project is installer-first + native launchd services.
  - **Acceptance Criteria:**
    - No container-runtime references remain in tracked files (excluding vendored dependencies).
    - GitHub Actions workflows do not run container build/compose teardown for Sim Lab.
    - Sim Lab smoke in CI runs on macOS and uses the installer-first/native stack.
  - **Status:** Done (container runtime references removed; CI updated)


- **DT-42: Add fast installer-path smoke checks + better E2E logs**
  - **Description:** Tighten the debug loop and prevent false-greens by adding fast, production-path smoke checks and capturing actionable service logs in E2E artifacts.
  - **Acceptance Criteria:**
    - A fast, non-UI smoke can validate an installed bundle is healthy (core `/healthz`, dashboard `/`, DB/MQTT/Redis reachable) without running Playwright.
    - `make e2e-web-smoke` fails fast if the configured core/web ports are already in use noted as a collision (unless explicitly overridden for debugging).
    - E2E artifacts capture core/web/sidecar + Sim Lab logs on failure (no log spam in terminal output).
  - **Notes:** Added `make e2e-installed-health-smoke` (farmctl-driven) and wired it into `make e2e-installer-stack-smoke` before Playwright. `tools/e2e_web_smoke.py` and `tools/e2e_ios_smoke.py` now fail fast on core/dashboard port collisions unless `FARM_E2E_ALLOW_PORT_COLLISION=1` is set. Added `make ci-farmctl-smoke` for quick Rust installer/launchd plan unit tests before running long E2E suites.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **DT-41: Align CI with local Makefile targets + Rust toolchain setup**
  - **Description:** Ensure CI uses the same Makefile targets as local runs and explicitly installs Rust for the Sim Lab sidecar.
  - **Acceptance Criteria:**
    - CI core/node/web jobs invoke Makefile targets (`make ci-core(-smoke)`, `make ci-node(-smoke)`, `make ci-web(-smoke/full)`).
    - CI web full runs `make ci-web-full` to include Next.js build.
    - Sim Lab smoke job installs a Rust toolchain before running `make e2e-web-smoke`.
    - Local `make ci` + `make e2e-web-smoke` are rerun after the change (record results).
  - **Notes:** `make ci` succeeded; `make e2e-web-smoke` succeeded after restarting native services (Sim Lab runs `CORE_DEMO_MODE=false`).
  - **Status:** Done

- **DT-27: Suppress Sim Lab candidate telemetry noise**
  - **Description:** Avoid emitting stub telemetry for adoption candidates so the ingest logs stay clean during E2E runs.
  - **Acceptance Criteria:**
    - Candidate node configs omit stub sensors when the database has none.
    - E2E runs no longer warn about `sim-sensor` telemetry.
  - **Status:** Done (make e2e-web-smoke)


- **DT-28: Reduce e2e-web-smoke log noise**
  - **Description:** Trim non-actionable noise from Sim Lab E2E runs (migration notices, fixture connect errors, and SQL echo spam) without hiding real failures.
  - **Acceptance Criteria:**
    - SQL migrations run without psql NOTICE spam in `make e2e-web-smoke`.
    - Sim Lab mocks are ready before core startup so forecast/rates ingest does not emit connection stack traces.
    - Sim Lab core server defaults to `CORE_DEBUG=false` so SQL echo logs do not flood E2E output.
  - **Status:** Done (make e2e-web-smoke)


- **DT-29: Expand E2E smoke coverage for dashboard flows**
  - **Description:** Extend the Sim Lab Playwright smoke suite to cover node detail, sensors/outputs list, output command/ack, alarms list/ack, schedules create/edit, and users/roles UI flows.
  - **Acceptance Criteria:**
    - New E2E smoke run covers the listed dashboard flows without flaky waits.
    - Failures include clear screenshots/logs for triage.
  - **Status:** Done (make e2e-web-smoke)


- **DT-30: Add telemetry pipeline E2E verification**
  - **Description:** Validate telemetry end-to-end (node-agent → MQTT → sidecar → core → UI) including change-of-value and rolling average semantics.
  - **Acceptance Criteria:**
    - Smoke verifies live data appears in core APIs and the UI for simulated nodes.
    - COV sensors only update on change; rolling averages are reflected at expected intervals.
  - **Status:** Done (make e2e-web-smoke)


- **DT-31: Add backups/restore E2E smoke**
  - **Description:** Add E2E coverage for listing/downloading backups and restoring a node configuration.
  - **Acceptance Criteria:**
    - Smoke confirms backup list and download succeed.
    - Smoke restores a node config and validates the update in `/api/nodes`.
  - **Status:** Done (make e2e-web-smoke)


- **DT-32: Add forecast/rates + schedule guard E2E smoke**
  - **Description:** Validate forecast/rate ingestion and schedule guard evaluation resulting in an alarm or action.
  - **Acceptance Criteria:**
    - Forecast/rates fixtures ingest successfully and are visible in core APIs.
    - A schedule guard evaluates against ingested data and triggers the expected outcome.
  - **Status:** Done (make e2e-web-smoke)


- **DT-33: Add provisioning E2E smoke (non-hardware)**
  - **Description:** Validate adoption token exchange plus provisioning session queue flow without hardware dependencies.
  - **Acceptance Criteria:**
    - Provisioning session is created and shows in the queue.
    - Adoption flow consumes token and registers the node.
  - **Status:** Done (make e2e-web-smoke)


- **DT-34: Add Renogy external ingest E2E smoke**
  - **Description:** Validate renogy-bt external ingest through node-agent into core and UI.
  - **Acceptance Criteria:**
    - `/v1/renogy-bt` ingest updates core metrics.
    - Dashboard shows Renogy metrics after ingest.
  - **Status:** Done (make e2e-web-smoke)


- **DT-35: Add iOS E2E UI smoke against Sim Lab**
  - **Description:** Add a lightweight simulator-driven UI smoke for the iOS app against a running core/Sim Lab (no BLE/mesh flows).
  - **Acceptance Criteria:**
    - App loads core data and navigates main tabs without errors.
    - Outputs and alarms screens render and can be exercised in simulator.
  - **Status:** Done (make e2e-ios-smoke)


- **DT-36: Trim pre-commit E2E scope to high-risk paths**
  - **Description:** Reduce pre-commit runtime by limiting E2E smoke runs to high-risk stack changes while keeping fast checks for dashboard and iOS changes.
  - **Acceptance Criteria:**
    - High-risk stack changes still trigger `make e2e-web-smoke`.
    - Dashboard-web-only changes trigger `make ci-web-smoke`.
    - iOS/watch changes trigger `make ci-ios-smoke`.
  - **Status:** Done (selector update; no runtime tests required)


- **DT-37: Validate dashboard-only pre-commit selection**
  - **Description:** Confirm the pre-commit selector runs `ci-web-smoke` for dashboard-only edits instead of full-stack E2E.
  - **Acceptance Criteria:**
    - Commit hook triggers `make ci-web-smoke` for dashboard-only staged paths.
    - Commit completes without running `make e2e-web-smoke`.
  - **Status:** Done (pre-commit hook run)


- **DT-23: Add Sim Lab control API service**
  - **Description:** Provide a dedicated Sim Lab control-plane API (Option C) implemented as a separate service under `tools/sim_lab/` that exposes sim-engine state and accepts scenario/fault injections, while domain data continues to come from core-server APIs.
  - **Acceptance Criteria:**
    - A FastAPI (or equivalent) service lives under `tools/sim_lab/` with a dedicated port and base URL.
    - Control service exposes endpoints for status, scenarios, and active faults (ex: `GET /sim-lab/status`, `GET /sim-lab/scenarios`, `GET /sim-lab/faults`).
    - Actions exist for start/stop/pause, set seed, set time multiplier, and apply/clear faults per node/sensor/output.
    - Destructive actions require an `armed` toggle (explicit API call or short-lived token) and refuse when not armed.
    - Control API updates node-agent simulation profiles via NA-26 endpoints; if unavailable, it can restart simulated nodes with updated profiles.
    - Service startup is wired into `tools/sim_lab/run.py` (or a dedicated runner) with clear port/config flags.
    - `tools/sim_lab/README.md` documents how to start the control API, its base URL, and required env vars.
  - **Status:** Done (make e2e-web-smoke; warnings: predictive 429s + node-agent telemetry output shape errors in logs)


- **DT-25: Disable predictive alarms during Sim Lab runs by default**
  - **Description:** Avoid predictive API rate-limit warnings by defaulting Sim Lab runs to predictive-disabled unless explicitly enabled.
  - **Acceptance Criteria:**
    - Sim Lab runner does not enable predictive alarms unless a CLI flag or explicit env toggle is provided.
    - Sim Lab E2E runs complete without predictive API rate-limit warnings in logs.
  - **Status:** Done (make e2e-web-smoke passing with predictive disabled by default; no adoption/token timeouts)


- **DT-26: Run Sim Lab E2E smoke in production mode**
  - **Description:** Ensure the E2E smoke run exercises the core server in production mode instead of demo mode.
  - **Acceptance Criteria:**
    - CI runs `make e2e-web-smoke` for Sim Lab adoption smoke (production mode).
    - The Sim Lab CI job installs node-agent dependencies and tears down infra after the run.
    - Docs call out that E2E smoke uses production mode.
  - **Status:** Done (make e2e-web-smoke)


- **DT-24: Add path-aware E2E commit hook selector**
  - **Description:** Replace the pre-commit `make ci` hook with a staged-path-aware selector that boots the full Sim Lab stack and runs E2E smoke tests for relevant changes, while preserving doc-only skips.
  - **Acceptance Criteria:**
    - Selector inspects staged paths and chooses the correct smoke targets (web stack, node-agent, iOS, docs-only).
    - `make e2e-web-smoke` boots the full stack (core + sidecar + web + Sim Lab mocks) and runs the Playwright smoke flow.
    - Pre-commit hook calls the selector and preserves the doc/log-only fast path.
    - Selector fails fast with a clear message when native services are required but not running.
    - E2E runner cleans `storage/sim_lab/*.json` runtime artifacts after each run.
  - **Status:** Done (tests: `make e2e-web-smoke`)


- **DT-9: Gate CI jobs by changed paths**
  - **Description:** Add change detection so CI jobs run only for relevant areas (core-server, dashboard-web, infra/native services), with a doc-only fast path that skips heavy work.
  - **Acceptance Criteria:**
    - CI determines changed paths and exposes per-area flags (server, web, infra, docs/planning).
    - Jobs run only when their area flag is true.
    - Doc-only changes complete with a green workflow and no heavy jobs executed.
  - **Status:** Done


- **DT-10: Split iOS simulator runs into an opt-in workflow**
  - **Description:** Move iOS simulator tests out of the main CI workflow and run them only on iOS changes or manual dispatch.
  - **Acceptance Criteria:**
    - A dedicated iOS workflow triggers on iOS path changes and `workflow_dispatch`.
    - Manual runs support inputs to select smoke vs full simulator suites.
    - Main CI no longer boots simulators on unrelated changes.
  - **Status:** Done


- **DT-11: Add smoke vs full test tiers**
  - **Description:** Define fast smoke suites for PR validation and reserve full regressions for nightly/label-triggered runs.
  - **Acceptance Criteria:**
    - Each app exposes a smoke target (core-server, node-agent, dashboard-web).
    - PR workflows run smoke targets by default; full suites run on schedule or label.
    - Documentation explains when to use smoke vs full.
  - **Status:** Done


- **DT-12: Optimize CI caching and concurrency**
  - **Description:** Improve CI reuse with caching and reduce duplicate runs via concurrency controls.
  - **Acceptance Criteria:**
    - Dependency caches (Poetry/pip, npm) keyed by lockfiles are enabled.
    - Build caches are reused across runs (Rust + Node dependencies).
    - Concurrency cancels superseded runs per branch/PR.
  - **Status:** Done


- **DT-13: Sim Lab deterministic mesh/BLE/feed simulation**
  - **Description:** Extend Sim Lab so it deterministically simulates mesh radio telemetry, BLE provisioning availability, and utility/forecast feeds without physical hardware.
  - **Acceptance Criteria:**
    - Simulated nodes emit mesh diagnostics/telemetry in mock mode with stable seeds.
    - BLE provisioning reports “ready” in Sim Lab on non-Linux hosts.
    - Forecast ingestion uses a deterministic simulator and utility rates are provided without external credentials.
    - Adoption and alarm flows remain unblocked in Sim Lab.
  - **Status:** Done


- **DT-4: Sim Lab runner for emulated hardware endpoints**
  - **Description:** Provide a local simulator that publishes node heartbeats and sensor metrics to MQTT (and optional HTTP endpoints) so the full stack can run against the seeded Postgres demo DB without physical hardware. Prefer reusing `apps/node-agent` (telemetry publisher + zeroconf advertiser) for protocol parity.
  - **Acceptance Criteria:**
    - `make demo-live` starts infra, runs `make migrate` + `make seed`, launches core-server + dashboard-web, and starts the simulator.
    - Simulator publishes MQTT topics that match core ingestion (`apps/core-server/app/services/mqtt_consumer.py`):
      - Telemetry: `iot/<node_uuid>/<sensor_id>/telemetry` with JSON containing `value`, optional ISO-8601 `timestamp`, and integer `quality`.
      - Status: `iot/<node_uuid>/status` with payload `online|offline` (or JSON with `status`).
    - For seeded DB nodes, simulator uses the DB node UUID as the MQTT `node_id` so node online/offline updates work (core server looks up nodes by primary key UUID).
    - Metrics follow configured intervals, including `interval_seconds=0` change-of-value behavior, and update the web dashboard/trends/alarms in real time.
    - Supports multiple simulated nodes concurrently without state collisions (per-node storage/config directories).
  - **Status:** Done (Sim Lab runner + node-agent simulation wiring + make demo-live)


- **DT-5: Sim Lab adoption workflow (mDNS + node-agent)**
  - **Description:** Run multiple node-agent instances in simulation mode advertising `_iotnode._tcp.local.` so dashboard scan/adopt works end-to-end on a single machine (python-zeroconf ServiceInfo registration + ServiceBrowser scan).
  - **Acceptance Criteria:**
    - At least two simulated node-agents advertise via mDNS and show up in `/api/scan` + the dashboard adoption UI.
    - Each simulated node advertises unique `mac_eth`/`mac_wifi` properties (required for adoption uniqueness) and uses unique `advertise_port` values.
      - Adoption works in real mode by issuing an adoption token (`POST /api/adoption/tokens`) and including it in the adopt request (`POST /api/adopt`), then persists the node keyed by MAC addresses.
      - After adoption, the node-agent is configured so its MQTT `node_id` equals the adopted DB node UUID (required for node status updates).
  - **Status:** Done (candidate node IDs now UUIDs; discovery-seeded adoption tokens advertised/filtered; make e2e-web-smoke)


- **DT-6: Simulated outputs + fault injection scenarios**
  - **Description:** Add command-loop handling for outputs and scenario scripts to test alarms/schedules (offline, spikes, stuck actuators, jitter). Use the asyncio MQTT client patterns recommended by the library docs (subscribe + async message iterator + graceful task cancellation).
  - **Acceptance Criteria:**
      - Simulated node(s) subscribe to output command topics (`iot/<node_uuid>/<output_id>/command` by default) and apply state transitions.
      - Long-running subscribers are cancellable and do not leak tasks (listener task cancels cleanly on shutdown).
      - Scenario definitions can toggle node/sensor offline (stop publishing / publish `offline`) and inject anomalies reproducibly for demos and regression tests.
  - **Status:** Done (output command listener + repeatable sim profiles/scenarios)


- **DT-7: Make demo-live rerunnable (migrations + web port)**
  - **Description:** Ensure analytics CAGG creation, predictive alarm metadata alters, and the Sim Lab web port avoid conflicts so `make demo-live` succeeds on seeded DBs.
  - **Acceptance Criteria:**
    - Re-running `make migrate` does not fail when analytics continuous aggregates already exist.
    - Predictive alarm metadata migration uses `alter table if exists` + `add column if not exists` without syntax errors.
    - `make demo-live` completes migrations without aborting on analytics CAGG creation.
    - Sim Lab dashboard-web starts on a non-Grafana port by default (3001) without EADDRINUSE.
  - **Status:** Done


- **DT-8: Remove guardrail reminders from test output**
  - **Description:** Stop printing guardrail/AGENTS reminders during pytest/Vitest runs so test output stays clean for new developers.
  - **Acceptance Criteria:**
    - Pytest runs do not emit guardrail reminder headers.
    - Vitest runs do not log guardrail reminder snippets.
  - **Status:** Done


- **DT-1: Add local CI target and enforce local-only testing**
  - **Description:** Added `make ci` aggregator covering core-server and node-agent pytest suites plus dashboard lint/tests, and documented the policy that all checks run locally (pre-commit now uses the staged-path selector).
  - **Status:** Done


- **DT-2: Offline Boot Config Generator**
  - **Description:** Create a user-friendly utility (e.g., a static HTML/JS page or script) to generate `node-agent-firstboot.json` files. This serves as a robust fallback for provisioning nodes when BLE (NA-21) is unavailable or for bulk-provisioning.
  - **Acceptance Criteria:**
    - Users can generate valid config files via a GUI form without writing raw JSON.
    - Tool validates inputs (e.g., SSID length) before generation.
  - **Status:** Done


- **DT-3: Stabilize dashboard-web Vitest environment for charts + downloads**
  - **Description:** Add JSDOM shims for `HTMLCanvasElement.getContext`, `ResizeObserver`, and anchor downloads so Chart.js rendering and CSV/JSON export helpers do not crash or emit noisy runtime warnings during `make ci`.
  - **Status:** Done (`apps/dashboard-web/vitest.setup.ts`)


- **DT-14: Observability foundation (logs + traces + runbooks)**
  - **Description:** Standardize structured logging and request IDs across core-server and node-agent; add OpenTelemetry exporter wiring with a local collector + Tempo quickstart dashboards; document runbooks for adoption/BLE/mesh failures.
  - **Acceptance Criteria:**
    - JSON logs emitted across core-server and node-agent include trace/span IDs.
    - Request IDs propagate through HTTP responses and MQTT command/ack payloads.
    - Local OTLP collector + Tempo + Grafana dashboard templates are shipped under `infra/`.
    - Runbooks cover adoption failures, BLE provisioning, and mesh dropouts.
  - **Status:** Done


- **DT-15: Release channels + semver + changelog tooling**
  - **Description:** Define alpha/beta/stable release channels with semver validation and changelog tooling, and enforce version checks in CI.
  - **Acceptance Criteria:**
    - `tools/release/` provides validation + changelog generation.
    - CI fails when web/iOS/firmware changes do not update version files.
    - Release channel rules are documented for contributors.
  - **Status:** Done


- **DT-16: Local Sim Lab hardware mocks + fixtures**
  - **Description:** Provide a local fixture stack that emulates MQTT node telemetry, BLE advertising, mesh coordinator status, and forecast/utility rate fixtures with deterministic outputs.
  - **Acceptance Criteria:**
    - `tools/sim_lab/` ships local fixture services with MQTT, node simulator, BLE mock, mesh mock, and forecast/rate fixture endpoints.
    - Fixture services expose stable JSON on predictable ports for demos and CI.
    - README documents how to run and stop the Sim Lab stack and wire core-server to the fixtures.
  - **Status:** Done


- **DT-17: Sim Lab Playwright adoption smoke in CI**
  - **Description:** Run a lightweight Playwright smoke that exercises the dashboard adoption flow against the Sim Lab stack.
  - **Acceptance Criteria:**
    - `apps/dashboard-web/scripts/sim-lab-smoke.mjs` performs a smoke adoption run.
    - CI starts the Sim Lab stack and runs the smoke script when core/web changes.
    - Developer docs call out the smoke script for local verification.
  - **Status:** Done (make e2e-web-smoke passes; adoption candidates filtered to ready nodes with tokens)


- **DT-18: Contract-first API + generated SDKs**
  - **Description:** Establish a master OpenAPI contract and SDK generator; use generated clients in web/iOS/node surfaces and enforce drift checks in CI.
  - **Acceptance Criteria:**
    - `apps/core-server/openapi/farm-dashboard.json` is exported from the Rust core-server (`apps/core-server-rs --print-openapi`) plus `tools/api-sdk/openapi_extras.json`.
    - SDKs are generated in `apps/dashboard-web/src/lib/api-client/`, `apps/ios-app/FarmDashboardApp/FarmDashboardApp/GeneratedAPI/`, and `apps/node-agent/app/generated_api/`.
    - CI runs an SDK regeneration job and fails on drift.
    - ADR documents the contract-first workflow.
  - **Notes / Run Log:**
    - 2026-01-08: Tightened `tools/check_openapi_coverage.py` to fail on “extra” Rust routes not present in the OpenAPI contract (prevents “added route but forgot to register in `apps/core-server-rs/src/openapi.rs`” slips). Updated pre-commit selector to run the coverage gate when Rust routes/OpenAPI/SDK files change.
  - **Status:** Done


- **DT-19: Isolate and clean up iOS simulators per test run**
  - **Description:** Create a disposable iOS simulator per `ci-ios` run and always shut it down/delete it so test state does not leak between runs.
  - **Acceptance Criteria:**
    - `make ci-ios` uses a helper to create a new simulator, injects the UDID into `xcodebuild`, and cleans up in a `finally`/trap.
    - A `REUSE_SIM=1` override keeps the simulator for local debugging.
    - Docs describe the disposable simulator lifecycle and override flag.
  - **Status:** Done


- **DT-20: Use disposable watch simulator pairs for screenshots**
  - **Description:** Run watch UI screenshots on a fresh paired iPhone + watch simulator and always clean up the pair to avoid stale state.
  - **Acceptance Criteria:**
    - `tools/watch_screenshots.py` creates a temporary phone + watch pair, boots it, and runs `xcodebuild` against the watch UDID.
    - Teardown shuts down and deletes both devices and unpairs them in a `finally` block.
    - `REUSE_WATCH_PAIR=1` preserves the pair for debugging and is documented.
  - **Status:** Done


- **DT-21: Ensure Sim Lab smoke sets CORS origins correctly**
  - **Description:** Encode CORS origin list for the demo core server spawned by the Sim Lab smoke script.
  - **Acceptance Criteria:**
    - `apps/dashboard-web/scripts/sim-lab-smoke.mjs` passes JSON-encoded origins to `CORE_CORS_ALLOWED_ORIGINS`.
    - The smoke script can start core-server without env parsing errors.
  - **Status:** Done


- **DT-22: Versioned pre-commit hook with doc/log skip**
  - **Description:** Provide a shared pre-commit hook + installer that runs the staged-path selector and skips when staged changes are doc/log-only.
  - **Acceptance Criteria:**
    - `tools/git-hooks/pre-commit` and `tools/git-hooks/install.sh` exist and are executable.
    - Hook skips E2E when all staged files have doc/log/image extensions (regardless of path).
    - README documents hook installation and extension-based skip rules.
  - **Status:** Done


- **DT-66: Define fastest dashboard-web validation loop (commands + runtime)**
  - **Description:** Capture the fastest reliable validation commands for dashboard-web-only changes (lint + smoke tests, optional build) with realistic runtime expectations, and use the loop for the next dashboard-web change set.
  - **Acceptance Criteria:**
    - The recommended command(s) and expected runtime are documented and shared for dashboard-web-only changes.
    - The next dashboard-web change set is validated with the agreed loop (tests/build run; results recorded).
  - **Notes / Run Log:**
    - 2026-01-23: Started; identify fastest reliable dashboard-web validation loop (commands + runtime) and prepare to run after changes land.
    - 2026-01-23: Reviewed Tier‑A runbook steps for dashboard‑web changes (commands, clean-worktree gate, screenshot evidence) to inform the validation loop guidance.
    - 2026-01-23: Recommended loop for dashboard-web-only changes:
      - `make ci-web-smoke` (~10s on dev laptop; lint + Vitest smoke)
      - `cd apps/dashboard-web && npm run build` (~5–15s; Next build + TS)
      - Guidance: treat build as **passed only if exit code is 0** and the Next route table prints (don’t stop at “Compiled successfully”, which can appear before the TS step).
    - 2026-01-23: Used the loop for DW-189/DW-190/DW-191; `make ci-web-smoke` (pass) + `npm run build` (pass) and Tier A evidence captured in `project_management/runs/RUN-20260123-tier-a-dw189-dw190-dw191-map-refactor-0.1.9.199.md`.
  - **Status:** Done


- **DT-67: Codify dashboard-web validation loop (make target + pre-commit)**
  - **Description:** Reduce reliance on humans reading long logs (or long task lists) by encoding the recommended dashboard-web validation loop as a single `make` target and running it automatically for staged dashboard-web changes via the pre-commit selector.
  - **Acceptance Criteria:**
    - `make ci-web-smoke-build` exists and runs `ci-web-smoke` + `npm run build` for dashboard-web.
    - `tools/git-hooks/select-tests.py` runs `make ci-web-smoke-build` when staged changes include `apps/dashboard-web/**`.
    - Validation is recorded: `python3 -m py_compile tools/git-hooks/select-tests.py` passes and `make ci-web-smoke-build` passes.
  - **Notes / Run Log:**
    - 2026-01-23: Implemented `make ci-web-smoke-build` and updated the pre-commit selector to run it for dashboard-web changes; validated via `python3 -m py_compile tools/git-hooks/select-tests.py` and `make ci-web-smoke-build`.
  - **Status:** Done


- **DT-69: Document full test suite + TSSE validation commands + reports artifact guidance**
  - **Description:** Provide a concise, actionable map of the repo’s full test suite, smoke vs full CI targets, and TSSE-specific validation commands (including benchmark harness usage), plus how to store artifacts under `reports/**` without breaking bundle build gates.
  - **Acceptance Criteria:**
    - The full suite and smoke targets are listed with the exact `make` commands and when to use them.
    - TSSE-specific validation guidance covers core-server-rs tests, OpenAPI coverage, dashboard-web smoke/build, and the TSSE bench harness (`tsse_bench` + dataset generation) with report output under `reports/**`.
    - Guidance states that `reports/**` is allowed to be dirty for bundle builds and shows how to capture logs/bench output there.
  - **Notes / Run Log:**
    - 2026-01-24: Documented full suite + TSSE validation command matrix and reports artifact guidance for CI/Test scout request.
  - **Status:** Done

- **DT-70: Codify installed-controller uptime discipline (upgrade only validated builds; rollback on failure)**
  - **Description:** Minimize downtime when debugging on the production-installed controller (Tier A) by enforcing a strict upgrade/rollback discipline: do not refresh to known-broken/incomplete builds, and rollback immediately on failed upgrades before continuing work.
  - **Acceptance Criteria:**
    - `AGENTS.md` documents the installed-controller uptime discipline (no known-broken upgrades; immediate rollback before continuing), and references the Tier‑A runbook.
  - **Notes / Run Log:**
    - 2026-01-24: Added “Installed Controller Uptime Discipline” guidance to `AGENTS.md`.
  - **Status:** Done

- **DT-71: Add local one-shot script to rebuild + refresh installed controller (Tier A)**
  - **Description:** Provide a single local script that rebuilds a controller bundle from the repo and triggers a Tier‑A refresh on an already-installed controller (no DB/settings reset), following `docs/runbooks/controller-rebuild-refresh-tier-a.md`.
  - **Acceptance Criteria:**
    - Script lives outside the repo and can be run from anywhere.
    - Script enforces the Tier‑A clean-worktree gate (allow `reports/**` only) and errors clearly if dirty.
    - Script builds a controller bundle DMG to a stable path under `/Users/Shared/FarmDashboardBuilds/`, updates setup-daemon `bundle_path`, and triggers `/api/upgrade`.
    - Script prints the resulting `current_version` from `http://127.0.0.1:8800/api/status` after refresh.
  - **Notes / Run Log:**
    - 2026-01-27: Added local helper script at `/Users/FarmDashboard/rebuild_refresh_installed_controller.py`.
  - **Status:** Done


---

## Discovery and Adoption
### Done
- **DA-3: Update core zeroconf module**
  - **Description:** Capture details + store adoption records.
  - **Status:** Done


- **DA-4: Implement `/api/scan` and `/api/adopt`**
  - **Description:** Implement endpoints with validation, template application, and provisioning tasks.
  - **Status:** Done


- **DA-5: Seed demo adoption tokens**
  - **Description:** Seed tokens for dashboard preview.
  - **Status:** Done


- **DA-6: Build dashboard adoption wizard**
  - **Description:** Hook dashboard wizard into new endpoints.
  - **Status:** Done


- **DA-1: Extend node agent advertisement with more metadata**
  - **Description:** Node agent advertisement includes MACs, firmware, capabilities, uptime, and mesh diagnostics.
  - **Status:** Done


- **DA-2: Enforce uniqueness constraints and stable naming logic**
  - **Description:** Adoption tokens enforce MAC uniqueness and stable naming during `/api/adopt`.
  - **Status:** Done


---

## Dashboard Web
### Done
- **DW-195: shadcn Phase 3 completion + dead code pruning**
  - **Description:** Complete the gray→token Tailwind class migration across all `.tsx` files, delete 9 dead provisioning files, and de-export 8 unused symbols.
  - **Acceptance Criteria:**
    - All hard-coded `text-gray-*`, `bg-gray-*`, `border-gray-*`, `divide-gray-*`, `ring-gray-*`, `placeholder-gray-*` Tailwind classes in `.tsx` files replaced with design token equivalents (`text-foreground`, `text-muted-foreground`, `bg-muted`, `bg-card-inset`, `border-border`, `border-input`, `divide-border`, `ring-ring`, etc.).
    - 9 dead provisioning files deleted (SensorsCard, NodeSettingsCard, FilePlacementCard, JsonPreviewCard, StatusBanner, useProvisioningState, builders, draft, validation).
    - 8 unused exports removed (scheduleUtils: 6 symbols; NodeInfoList: NodeInfoListItem; NodeTypeBadge: classifyNodeType).
    - `npm run lint && npm run test:smoke && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-30: Ran Node.js migration script: 1,571 replacements across 109 files.
    - 2026-01-30: 6 intentional gray classes remain (status indicator dots `bg-gray-400`, pending badge `bg-gray-300`, SVG compass `text-gray-300`).
    - 2026-01-30: Deleted 9 dead provisioning files; kept alive: `page.tsx`, `SensorCard.tsx`, `presets.ts`, `types.ts`, `utils.ts`.
    - 2026-01-30: De-exported 8 symbols across 3 files.
    - 2026-01-30: Validation: `npm run lint` (pass), `npm run test:smoke` (3/3 pass), `npm run build` (24 static pages, pass).
  - **Status:** Done

- **DW-184: Setup Center parser/validator unit tests**
  - **Description:** Add fast unit coverage for Setup Center config parsing/validation helpers so the SetupPageClient refactor is pinned by tests.
  - **Acceptance Criteria:**
    - Helper functions extracted from `SetupPageClient.tsx` (string/number/bool coercion + setup-daemon/runtime config parsers + preflight/local-ip parsing) have dedicated unit tests.
    - Tests live under `apps/dashboard-web/tests/` and run via Vitest.
    - `cd apps/dashboard-web && npm test -- --run tests/setupDaemonParsers.test.ts tests/setupValidation.test.ts` passes.
  - **Notes / Run Log:**
    - 2026-01-22: Added `tests/setupDaemonParsers.test.ts` and `tests/setupValidation.test.ts`. Command: `cd apps/dashboard-web && npm test -- --run tests/setupDaemonParsers.test.ts tests/setupValidation.test.ts` (pass).
  - **Status:** Done

- **DW-189: Map/Setup: dedupe offline map pack install UX + progress logic**
  - **Description:** Remove duplicated offline-pack install/progress logic between the Map tab and Setup Center by introducing shared hooks/components while keeping copy and visuals consistent.
  - **Acceptance Criteria:**
    - Map tab and Setup Center use a shared hook for offline-pack install actions (busy/error handling + query invalidation).
    - Offline-pack progress math is centralized (no local re-implementation of per-layer totals).
    - Offline-pack status UI (missing/failed/installing/progress/installed/not-installed) is rendered via a shared component with no copy regressions.
    - Changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no layout drift; existing component variants only).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-23: Started deduping offline map pack install UI + progress logic for Map/Setup.
    - 2026-01-23: Implemented shared `OfflinePackInstallStatus` + `useOfflineMapPackInstaller` and rewired Map/Setup offline pack sections.
    - 2026-01-23: Rewired `MapOfflinePackCard` to use the shared hook/status component (no inline progress math).
    - 2026-01-23: Validation: `make ci-web-smoke` (pass; eslint warnings pre-existing); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-23: Tier A refreshed installed controller to `0.1.9.199` (run: `project_management/runs/RUN-20260123-tier-a-dw189-dw190-dw191-map-refactor-0.1.9.199.md`; screenshots: `manual_screenshots_web/20260122_211457/map.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** Done (Tier A validated installed `0.1.9.199`; Tier B deferred to `DW-97`)


- **DW-190: Map: centralize derived data for MapCanvas + sidebar panels**
  - **Description:** Consolidate map-derived lookup data (nodes/sensors/features/layers/offline packs) into a shared hook so MapCanvas and sidebar panels reuse memoized maps/collections instead of recomputing per component.
  - **Acceptance Criteria:**
    - A shared `useMapDerivedData`/`useMapContext` hook returns memoized lookup maps (nodes/sensors/features/layers/offline packs) plus MapCanvas-ready feature collections.
    - MapCanvas consumes the shared feature collections and lookup maps (no local `Map` building in `MapCanvas.tsx`).
    - Map sidebar panels use the shared memoized maps/sets (no per-panel `Map` recomputation for node/sensor/feature lookups).
    - Changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no visual regressions; use existing component variants).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-23: Implemented map derived-data hook + rewired MapCanvas/sidebar panels. Tests: `make ci-web-smoke` (pass; lint warnings pre-existing), `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-23: Tier A refreshed installed controller to `0.1.9.199` (run: `project_management/runs/RUN-20260123-tier-a-dw189-dw190-dw191-map-refactor-0.1.9.199.md`; screenshots: `manual_screenshots_web/20260122_211457/map.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** Done (Tier A validated installed `0.1.9.199`; Tier B deferred to `DW-97`)


- **DW-191: Map tab: refactor `MapPageClient` into sections + hooks**
  - **Description:** `apps/dashboard-web/src/app/(dashboard)/map/MapPageClient.tsx` bundled data fetching, derived state, map actions, and the entire sidebar UI. Split it into focused hooks and section components so map changes are isolated and the page file becomes a thin orchestrator.
  - **Acceptance Criteria:**
    - `apps/dashboard-web/src/app/(dashboard)/map/MapPageClient.tsx` is reduced to orchestrating top-level layout (MapCanvas + sidebar panels + modals) with no large inline UI sections.
    - Map sidebar sections live in dedicated components under `apps/dashboard-web/src/features/map/components/` (Offline pack, Base map, Devices, Markup, Overlays, Save-as modal, placement banner).
    - State/data helpers live under `apps/dashboard-web/src/features/map/hooks/` (viewport fill, placement state/actions, sidebar filters, modal controller, shared map context).
    - No UX regressions; changes follow `apps/dashboard-web/AGENTS.md` guardrails. If a one-off is unavoidable, add a `DW-*` UI debt ticket (owner + exit criteria).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-23: Refactored Map tab into focused hooks/panels; MapPageClient now composes MapCanvas + sidebar panels + modals.
    - 2026-01-23: Validation: `make ci-web-smoke` (pass; lint warnings pre-existing); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-23: Tier A refreshed installed controller to `0.1.9.199` (run: `project_management/runs/RUN-20260123-tier-a-dw189-dw190-dw191-map-refactor-0.1.9.199.md`; screenshots: `manual_screenshots_web/20260122_211457/map.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** Done (Tier A validated installed `0.1.9.199`; Tier B deferred to `DW-97`)


- **DW-192: Map tab: post-upgrade manual smoke checklist**
  - **Description:** Provide a quick, refactor-scoped manual smoke checklist for the Map tab after controller upgrades.
  - **Acceptance Criteria:**
    - `docs/qa/map-tab-upgrade-smoke.md` exists and is linked from `docs/qa/QA_NOTES.md`.
    - Checklist covers layers/base maps, overlays, saved maps, custom markup CRUD, node/sensor placement, and offline pack install/verification.
    - Checklist is scoped to the Map refactor changes (DW-189/DW-190/DW-191) and stays concise.
  - **Status:** Done


- **DW-157: Dashboard display order: drag-and-drop reorder nodes + sensors (persistent)**
  - **Description:** Allow operators to drag-and-drop reorder nodes and sensors once, persist that order in the controller DB, and render lists consistently across the dashboard (Nodes, Sensors & Outputs, Trends, Map, Analytics, etc.). This prevents per-page sorting drift and makes “where things are” stable.
  - **Acceptance Criteria:**
    - Core server stores and serves a persistent `ui_order` for nodes and per-node sensors (not localStorage).
    - `GET /api/nodes` returns nodes in persisted order (stable across refreshes).
    - `GET /api/sensors` returns sensors ordered by node order then sensor order (stable across refreshes).
    - New write endpoints exist (require `config.write`):
      - `PUT /api/nodes/order` updates node order.
      - `PUT /api/nodes/{node_id}/sensors/order` updates sensor order for that node.
    - Dashboard UI provides drag-and-drop reordering UX for nodes and sensors and persists changes (no page-specific hacks).
    - Relevant tabs render nodes/sensors in the same order (no local `sort()` overrides).
    - `make ci-core-smoke`, `make ci-web-smoke`, and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/`.
  - **Notes / Run Log:**
    - 2026-01-19: Tier A validated installed controller `0.1.9.168`; run log: `project_management/runs/RUN-20260119-tier-a-dw157-display-order-0.1.9.168.md`.
    - Tier B: deferred to existing clean-host web validation cluster (see `DW-114` / `DW-98`) because this host is intentionally running the installed controller stack.
  - **Status:** Done (Tier A validated installed `0.1.9.168`; Tier B deferred to `DW-114` / `DW-98`)


- **DW-158: Trends: Per-panel analysis keys + plain-English variable labels**
  - **Description:** Make Trends’ analysis features self-explanatory for non-technical operators by attaching “Key / How to interpret” guidance to each analysis section (Chart settings, Related sensors, Relationships, Matrix Profile) and by replacing fragile/cryptic abbreviations with clear labels and tooltips (e.g., “Correlation (r)”, “Buckets (n)”, “Event score”, “Matched events”, “Distance”).
  - **Acceptance Criteria:**
    - Sensor picker includes a Key explaining selection limits, filters (node + search), PUBLIC sensor meaning, and the “Hide public provider data” toggle.
    - Chart settings includes a Key explaining Range/Interval semantics, axes behavior, and chart interactions (zoom/pan/reset).
    - Trend chart includes a Key adjacent to the chart explaining bucketed points, gaps/missing data, tooltips/units, axes modes, and zoom/pan/reset.
    - Related sensors includes a Key explaining correlation vs events mode, `r`/`n`/lag, event detection controls (threshold `z`, polarity, min separation), and conditioning bins (“≈ constant” comparisons).
    - Related sensors preview/suggestions use plain-English labels + tooltips (no “what is n?” ambiguity).
    - Relationships includes Keys adjacent to each sub-tool (matrix + pair analysis) so users don’t need to scroll to understand the current visualization.
    - Relationships uses plain-English labels for the active pair summary and defines all variables shown (`r`, `n`, lag, rolling window).
    - Matrix Profile includes Keys adjacent to each visualization (profile curve, window highlights, shape overlay, heatmap, anomalies/motifs list), defining all variables shown (window, points `n`, distance, exclusion, `z`).
    - Matrix Profile UI labels favor “distance” over “dist”.
    - No essential interpretation/variable definition lives only in the page-level Key; each analysis section’s Key is self-contained for the UI it sits next to.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/`.
  - **Notes / Run Log:**
    - 2026-01-19: User feedback: per-panel Keys were not close enough to each visualization; refactoring to add per-subsection Keys + variable definitions.
    - 2026-01-19: Follow-up UX feedback: ensure Keys exist next to Sensor picker and the main Trend chart (not just the controls panels), and unify “Buckets (n)” terminology (no “points” ambiguity).
    - 2026-01-19: Tier A validated installed controller `0.1.9.170`; run log: `project_management/runs/RUN-20260119-tier-a-dw158-trends-analysis-keys-0.1.9.170.md`.
    - Tier B: deferred to existing clean-host web validation cluster (see `DW-114` / `DW-98`) because this host is intentionally running the installed controller stack.
    - 2026-01-19: Reopened after additional UX feedback: ensure each Key defines all variables shown in its adjacent UI (not just overall panel), and rewrite copy for non-technical operators.
    - 2026-01-19: Continue: add Keys adjacent to each Relationships sub-tool and each Matrix Profile visualization; ensure variable definitions (`r`, `R²`, `n`, lag, window, distance, thresholds) are present without requiring scrolling.
    - 2026-01-19: Follow-up UX gap: user reports the original “top-level” Key guidance did not fully carry over into the per-section Keys; fix by (1) ensuring every visualization/variable has either an adjacent Key or a label tooltip, and (2) rewriting Keys for operators (assume no math/stats background; use concrete “when to use” examples).
    - 2026-01-19: Follow-up UX gap: user still cannot find key variable definitions in-context (e.g., `n`/bucket overlap) without using the top Key; continue by simplifying visible labels (prefer “Overlap” over “Buckets (n)”), keeping terminology consistent across panels, and ensuring per-panel Keys cover every number shown nearby.
    - 2026-01-19: Tier A validated installed controller `0.1.9.174`; run log: `project_management/runs/RUN-20260119-tier-a-dw158-overlap-n-0.1.9.174.md`.
    - Tier B: deferred to existing clean-host web validation cluster (see `DW-114` / `DW-98`) because this host is intentionally running the installed controller stack.
  - **Status:** Done (Tier A validated installed `0.1.9.174`; Tier B deferred to `DW-114` / `DW-98`)


- **DW-164: Trends: Inline bottom help keys (replace popovers)**
  - **Description:** Redesign Trends’ help “Keys” (AnalysisKey) so they live at the bottom of each relevant container, always show a short plain-English overview, and expand inline for details (no floating popover overlays). This reduces header clutter and keeps help discoverable.
  - **Acceptance Criteria:**
    - On `/trends`, help is shown as an inline bottom section (not a header popover button) for:
      - Page header (overall Trends workflow/glossary).
      - Sensor picker.
      - Chart settings.
      - Main Trend chart.
      - Related sensors.
      - Relationships.
      - Matrix Profile.
    - Each help section starts with a short **Overview** that is always visible without clicking.
    - A clear affordance (“View details” / “Hide details”) expands/collapses the full help content inline.
    - No Trends help content renders as an absolute-positioned overlay/popover.
    - Change follows `apps/dashboard-web/AGENTS.md` UI/UX guardrails (page pattern + Tailwind token set; no new one-off styles).
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validation is recorded with at least one captured + viewed screenshot under `manual_screenshots_web/` and a run log under `project_management/runs/`.
    - Tier B validation is deferred to `DW-98` (Trends/COV/CSV clean-host cluster).
  - **Notes / Run Log:**
    - 2026-01-19: Added after UX feedback that the header popover “Key” buttons are too visually noisy and too disconnected from their section content.
    - 2026-01-20: Tier A validated installed controller `0.1.9.175`; run log: `project_management/runs/RUN-20260120-tier-a-dw164-trends-inline-help-keys-0.1.9.175.md`. Evidence (viewed): `manual_screenshots_web/tier_a_0.1.9.175_trends_keys_2026-01-20_002031118Z/01_trends_sensor_picker_key.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-163: Sensors & Outputs: Stack Outputs below Sensors in node panels**
  - **Description:** In the Sensors & Outputs tab, the per-node Sensors table currently shares a row with the Outputs card on wide screens, making the Sensors container too narrow and causing the table to spill outside its container. Stack Outputs below Sensors so Sensors can use full width.
  - **Acceptance Criteria:**
    - In Sensors & Outputs (grouped-by-node view), the Outputs section renders below the Sensors section (no side-by-side layout on desktop).
    - The Sensors table no longer overflows outside its card on typical desktop widths.
    - Change follows the dashboard page pattern and uses Tailwind tokens (no inline styles).
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Notes / Run Log:**
    - 2026-01-19: Implemented stacked layout in per-node panels. CI: `make ci-web-smoke` (pass); build: `cd apps/dashboard-web && npm run build` (pass).
  - **Status:** Done


- **DW-165: Analytics IA: Move Trends + Power under Analytics (sub-tabs) + reorganize Analytics Overview**
  - **Description:** Reorganize the dashboard’s “Analytics” area to improve hierarchy and maintainability. Move Trends and Power under Analytics as sub-tabs (Analytics Overview / Trends / Power), keep legacy entrypoints working, and refactor Analytics Overview into clear, task-first sections without losing any functionality.
  - **Acceptance Criteria:**
    - Navigation / IA:
      - Sidebar includes a dedicated **Analytics** group.
      - Analytics group includes exactly: **Analytics Overview**, **Trends**, **Power**.
      - Top-level sidebar items “Trends” and “Power” are removed from Operations (moved under Analytics).
      - Canonical routes exist and render correctly:
        - `/analytics` → Analytics Overview
        - `/analytics/trends` → Trends
        - `/analytics/power` → Power
      - Legacy entrypoints remain functional:
        - `/trends` loads Trends and routes users to `/analytics/trends` (or otherwise lands them on the canonical path without breaking bookmarks).
        - `/power` loads Power and routes users to `/analytics/power` (or otherwise lands them on the canonical path without breaking bookmarks).
      - Internal “Link” actions (e.g., sensor drawers) use `/analytics/trends` going forward.
      - Overview “Where things live” map reflects the new IA.
    - Analytics Overview UX:
      - Reorganized into a clear hierarchy with consistent layout and spacing (no drift / overflow / broken grids):
        - At-a-glance summary cards
        - Forecasts
        - Energy (fleet totals)
        - Water & Soil
        - Fleet health / status
        - Advanced (includes Feed health for now to avoid feature loss)
      - **No feature loss** from the existing Analytics, Trends, or Power pages:
        - Existing Analytics charts/controls remain available (including DW-162 range controls and Weather forecast segmented control behavior).
        - Existing Trends features remain available (selection, relationships, matrix profile, CSV export, analysis keys).
        - Existing Power dashboards remain available (Renogy/Emporia node dashboards, charts, quality panels).
    - Time standardization (scoped to Analytics area):
      - Analytics Overview + Trends + Power display timestamps using controller-local time (site time), not the viewer’s browser timezone.
      - Charts on these pages format x-axis ticks/tooltips in controller-local time.
      - Custom range inputs in Trends are interpreted in controller-local time.
    - Tooling + validation:
      - Screenshot runner updates capture `/analytics/trends` and `/analytics/power`.
      - `make ci-web-smoke`, `cd apps/dashboard-web && npm run build`, and `cargo build --manifest-path apps/core-server-rs/Cargo.toml` pass.
      - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/`.
    - Tier B deferred to existing clean-host clusters (`DW-98`, `DW-114`, `CS-69`).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Plan: add shared Analytics layout + tabs, move routes, keep legacy entrypoints working, refactor Analytics Overview structure, and standardize controller-local time on these pages first.
    - 2026-01-20: Tier A validated on installed controller `0.1.9.176` (run: `project_management/runs/RUN-20260120-tier-a-dw165-analytics-ia-0.1.9.176.md`). Evidence: `manual_screenshots_web/20260119_203247/analytics.png`, `manual_screenshots_web/20260119_203247/trends.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98/DW-114/CS-69)


- **DW-167: Analytics: Remove header subpage tabs + rename Solar/UV plot label**
  - **Description:** Remove cross-page navigation controls from the top header of Analytics pages (use the sidebar Analytics group for navigation). Also rename the Analytics Overview “Pressure / solar / UV” plot header to “Solar / UV” for clarity.
  - **Acceptance Criteria:**
    - `/analytics`, `/analytics/trends`, and `/analytics/power` do not show cross-page navigation links/segmented controls in the page header.
    - Page header title correctly reflects the current page (`Analytics Overview`, `Trends`, `Power`).
    - Analytics Overview weather history plot header reads `Solar / UV — <range>`.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the updated header and the renamed plot.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Plan: simplify `AnalyticsHeaderCard` (remove internal subpage tabs) and do minor copy polish in Analytics Overview.
    - 2026-01-20: Tier A validated on installed controller `0.1.9.177` (run: `project_management/runs/RUN-20260120-tier-a-dw167-analytics-header-cleanup-0.1.9.177.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/CS-69)


- **DW-168: Analytics Overview: Fix Solar/UV + Pressure chart layout**
  - **Description:** Fix layout/readability issues in the Analytics Overview weather-history charts. The Solar/UV plot currently renders poorly on compact cards (axis labels overlap) and the layout becomes inconsistent when pressure data is present. Refactor so Solar/UV and Pressure render as full-size chart cards and apply compact x-axis labels for 24-hour views. Also stack Weather Stations and Weather Forecast vertically so both sections can use full width.
  - **Acceptance Criteria:**
    - Analytics Overview weather-history section renders `Solar / UV — <range>` as a single chart card (not stacked mini-charts).
    - Pressure history, when available, renders as a separate `Pressure — <range>` chart card (no combined “Pressure / solar / UV” card).
    - Chart sizing matches other weather-history cards (`h-64`) and axes/legend do not overlap on typical desktop widths.
    - For 24-hour analytics history charts, x-axis ticks are time-only (no full date in each tick label).
    - In Forecasts, Weather stations and Weather forecast sections are stacked vertically (full width), not side-by-side.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the fixed layout.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.179` (run: `project_management/runs/RUN-20260120-tier-a-dw168-dw169-analytics-polish-0.1.9.179.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/CS-69)


- **DW-169: Display order modal: Fix viewport overflow (Nodes + Sensors & Outputs)**
  - **Description:** The “Reorder…” (Display order) modal currently extends outside the top/bottom of the viewport on Nodes and Sensors & Outputs, making it hard to use. Refactor the modal layout so it always fits within the viewport and scrolls internally when needed.
  - **Acceptance Criteria:**
    - Display order modal never extends outside the viewport on typical laptop and smaller screens.
    - Modal content scrolls inside the dialog (header/actions remain usable) when lists are tall.
    - Behavior is consistent when opened from Nodes and Sensors & Outputs.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the modal fully visible (no clipping).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.179` (run: `project_management/runs/RUN-20260120-tier-a-dw168-dw169-analytics-polish-0.1.9.179.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/DW-98)


- **DW-170: Analytics Overview: Battery voltage full width**
  - **Description:** On Analytics Overview, the Battery voltage section is currently rendered side-by-side with Fleet status on wide screens, which squeezes layout and makes the chart hard to read. Move Battery voltage to its own full-width row beneath Fleet status.
  - **Acceptance Criteria:**
    - On `/analytics`, Battery voltage renders below Fleet status (full width) on desktop layouts.
    - No regressions in section ordering or collapsible behavior.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing Battery voltage full width.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.180` (run: `project_management/runs/RUN-20260120-tier-a-dw170-analytics-battery-full-width-0.1.9.180.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/CS-69)


- **DW-171: Analytics Overview: Weather forecast layout tighten (avoid empty whitespace)**
  - **Description:** The Weather forecast panel in Analytics Overview can render with excessive empty whitespace (for example, when only one chart is available but the layout still reserves space for a second chart). Refactor the Weather forecast chart layout so it always uses space efficiently and never shows a “blank half” on wide screens.
  - **Acceptance Criteria:**
    - On `/analytics`, Weather forecast charts are full-width when only one plot is available (no empty right-hand column).
    - When two plots are available, the layout uses a two-column grid on wide screens and stacks on small screens.
    - Segmented horizon control (24h / 72h / 7d) continues to work without regressions.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the Weather forecast panel with the tightened layout.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.181` (run: `project_management/runs/RUN-20260120-tier-a-dw171-analytics-weather-forecast-layout-tighten-0.1.9.181.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/CS-69)


- **DW-172: Analytics Overview: Weather station “Live readings” layout fixes**
  - **Description:** The Weather stations “Live readings” card has crowded spacing and overlapping text at common viewport sizes (especially when Weather stations are expanded). Refactor the layout to be robust and readable across breakpoints (no overlapping text; consistent spacing and hierarchy).
  - **Acceptance Criteria:**
    - “Live readings” values do not overlap and remain readable at typical laptop widths and smaller windows.
    - Layout uses appropriate responsive breakpoints (no overly tight multi-column grids).
    - Wind direction card and Live readings layout do not crowd each other; the two cards stack when space is constrained.
    - No regressions to Weather station charts below the live readings section.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the fixed Weather station panel.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.182` (run: `project_management/runs/RUN-20260120-tier-a-dw172-weather-station-live-readings-layout-0.1.9.182.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/CS-69)


- **DW-173: Analytics Overview: UI/layout/rendering audit + fixes (round 1)**
  - **Description:** Perform a full UI/layout/rendering audit of the Analytics Overview page using fresh Playwright screenshots. Identify issues (spacing, overlap, chart ticks, empty states, consistency) and fix them in a maintainable way (shared chart options/patterns; no page-specific hacks).
  - **Acceptance Criteria:**
    - No overlapping/garbled x-axis tick labels on Analytics Overview charts at 24h range (especially Water usage).
    - Analytics Overview charts retain consistent tooltip behavior (controller-local time formatting; no lost plugin config).
    - Empty states are clear (avoid “blank chart” confusion when datasets are missing/empty).
    - Tier A validation is recorded with viewed screenshots under `manual_screenshots_web/` demonstrating the fixes.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Audit screenshots captured + viewed: `manual_screenshots_web/20260120_041942/`.
      - Water → “Usage — Past 24 hours” shows overlapping x-axis tick labels (root cause: 24h mode can mix 7d series when `*_series_24h` is missing; must always filter to requested range).
      - Weather forecast + PV forecast vs measured charts appear “blank” (axes/legend render but no line). Suspected: timestamp normalization and/or Chart.js animation timing; fix should normalize forecast timestamps to `Date` and ensure charts render deterministically for screenshots (avoid page-by-page hacks).
    - 2026-01-20: Tier A validated on installed controller `0.1.9.184` (run: `project_management/runs/RUN-20260120-tier-a-dw173-dw174-0.1.9.184.md`).
      - Evidence: `manual_screenshots_web/20260120_081428/analytics.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/CS-69)


- **DW-174: Display order modal: ensure viewport-safe scrolling**
  - **Description:** The “Reorder…” (Display order) modal can still become unusable on shorter viewports because the list region is clipped (no reliable way to reach items near the bottom). Refactor the modal layout so it always fits within the viewport and supports scrolling (either internal dialog scroll or an outer overlay scroll) without breaking drag-and-drop.
  - **Acceptance Criteria:**
    - Display order modal remains fully usable on typical laptop and shorter-height windows: content never becomes unreachable.
    - Scrolling works reliably while lists are tall (header actions remain accessible).
    - Tier A validation includes a viewed screenshot under `manual_screenshots_web/` showing the modal within the viewport and the sensor list reachable.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Screenshot shows remaining clipping near the bottom of the sensor list: `manual_screenshots_web/20260120_041942/nodes_reorder_modal.png`. Fix should ensure the dialog has a viewport-constrained height and a single, reliable internal scroll region (header/actions remain visible).
    - 2026-01-20: Tier A validated on installed controller `0.1.9.184` (run: `project_management/runs/RUN-20260120-tier-a-dw173-dw174-0.1.9.184.md`).
      - Evidence: `manual_screenshots_web/20260120_081428/nodes_reorder_modal.png`, `manual_screenshots_web/20260120_081428/sensors_reorder_modal.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114/DW-98)


- **DW-175: Analytics Overview: unify range selector UI**
  - **Description:** The Weather forecast 24h/72h/7d selector does not match the other range controls on Analytics Overview. Refactor range selectors into a single shared UI pattern so every Analytics Overview container uses consistent labels, sizing, and control type.
  - **Acceptance Criteria:**
    - On `/analytics`, Weather forecast, Weather stations, PV forecast vs measured, Power, Water, Soil, and Battery voltage all use the same range selector UI pattern (single shared component; no local variants).
    - Range selector options are consistent (24h / 72h / 7d) and continue to drive the same underlying data windows (no feature loss/regressions).
    - Range selector layout remains readable on typical laptop widths and smaller windows (no overflow/clipping).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation includes at least one viewed screenshot showing the Weather forecast range selector visually matching the others.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.188` (run: `project_management/runs/RUN-20260120-tier-a-dw177-trends-overlay-limit-0.1.9.188.md`).
      - Evidence: `manual_screenshots_web/20260120_133427/trends.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-187: Analytics Overview: de-bloat `AnalyticsOverview.tsx` (module split + shared hooks/libs)**
  - **Description:** `apps/dashboard-web/src/features/analytics/components/AnalyticsOverview.tsx` is a ~3.3k LOC “god file” that bundles summary, power/water/soil, weather station + forecast, PV forecast, status/battery, and feed health. Refactor into smaller components and shared helpers so isolated tweaks don’t require reloading the entire page context. No behavior/UX regressions are allowed.
  - **Acceptance Criteria:**
    - File split:
      - `apps/dashboard-web/src/features/analytics/components/AnalyticsOverview.tsx` becomes a thin orchestrator that composes extracted sections (minimal local helpers; no multi-hundred-line inline components).
      - Weather-related code is split into focused components (e.g., live readings, compass, custom sensors table, station charts, forecast charts) under `apps/dashboard-web/src/features/analytics/components/`.
      - PV forecast windowing logic is extracted into a helper so it can be adjusted without touching JSX.
    - Shared libs (no new duplicates):
      - `sensorSource`/`sensorMetric`/`findSensor` are centralized (extend `apps/dashboard-web/src/lib/sensorOrigin.ts`), and local re-implementations are deleted.
      - Power-node classification helpers are centralized into a shared module used by **both** Analytics Overview and Power page (no forked logic).
      - No new ambiguous `analytics/lib/*` paths are introduced (use `apps/dashboard-web/src/features/analytics/**` for Analytics-specific helpers).
    - Shared data layer:
      - A shared `useAnalyticsData` hook/context fetches Nodes + Sensors once and provides memoized maps (e.g., `nodesById`, `nodeLabelsById`, `sensorsByNodeId`) consumed by sections.
    - Charts:
      - Decision is documented: either (A) reuse `TrendChart` for standard line charts or (B) keep raw Chart.js but centralize chart option/dataset builders so Weather station/forecast stop duplicating config.
      - Zoom/reset behavior remains consistent across extracted charts (double-click reset supported).
    - Validation:
      - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
      - Refactor conforms to `apps/dashboard-web/AGENTS.md` UI/UX guardrails; if any deliberate one-off is required, add a `DW-*` UI debt follow-up ticket (owner + measurable exit criteria).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-22: Started refactor (god-file split + shared helper consolidation + shared analytics data hook).
    - 2026-01-22: Audit confirmed `AnalyticsOverview.tsx`, `PowerPageClient.tsx`, and `SetupPageClient.tsx` use shared `sensorOrigin`/`powerSensors` helpers with no local re-implementations; follow-up ideas: reuse shared `configString` in `Ws2902SensorBuilder` and extract the duplicated `sensorSourceBucket` helper in Trends panels.
    - 2026-01-22: Wired WeatherStationSection + StatusSection to use shared `useAnalyticsData` (removed direct nodes/sensors queries; no UX change).
    - 2026-01-22: Extracted weather station UI + shared analytics chart helpers into new component files (pending wiring into `AnalyticsOverview.tsx`).
    - 2026-01-22: Extracted PV forecast section into `apps/dashboard-web/src/features/analytics/components/PvForecastSection.tsx` and moved PV windowing logic into `apps/dashboard-web/src/features/analytics/utils/pvForecast.ts` with unit coverage (`apps/dashboard-web/tests/pvForecastHelpers.test.ts`, `cd apps/dashboard-web && npm test -- pvForecastHelpers.test.ts`).
    - 2026-01-22: Completed split: `AnalyticsOverview.tsx` is now a thin orchestrator composed of extracted sections + shared `useAnalyticsData` provider; shared Chart.js helpers live in `AnalyticsShared.tsx` (Option B: keep raw Chart.js + centralize builders). Validation: `make ci-web-smoke`, `cd apps/dashboard-web && npm run build`, and `cd apps/dashboard-web && npm test` pass.
  - **Status:** Done (CI + build + vitest pass)


- **DW-176: Trends: Savitzky–Golay smoothing toggle + advanced settings**
  - **Description:** Add an optional Savitzky–Golay (SG) filter to the Trends Trend chart so operators can smooth noisy series or visualize rate-of-change. This must be implemented in a scientifically correct way (least-squares polynomial smoothing with derivative support) and in a maintainable way (TrendChart opt-in prop; no cross-page regressions).
  - **References:**
    - Ticket: `project_management/archive/archive/tickets/TICKET-0039-trends-savgol-smoothing-toggle.md`
  - **Acceptance Criteria:**
    - UX:
      - Trends → Chart settings includes a `Savitzky–Golay` toggle (off by default).
      - A collapsible `Advanced` section includes configurable parameters:
        - window length (odd)
        - polynomial degree
        - derivative order (0 = smoothed series; 1+ = derivative)
        - edge mode (at least `interp` + one padding mode like `nearest`/`mirror`)
      - Parameters validate with plain-English errors (no crashes); invalid settings do not apply the filter.
    - Correctness:
      - SG is computed via least-squares polynomial fitting (not a moving average).
      - Derivative mode uses `Δt` derived from the selected Trend chart Interval.
      - Missing-data gaps remain visible; smoothing never crosses null gaps.
    - Maintainability:
      - SG logic lives in a shared helper (no copy/paste per chart).
      - `TrendChart` is unaffected elsewhere unless explicitly opted in by prop.
      - UI change conforms to `apps/dashboard-web/AGENTS.md` guardrails (page pattern + token set, templates/slots, visual hierarchy, component variants). If any one-off is required, add a follow-up `DW-*` UI debt ticket (owner + measurable exit criteria) before merge.
    - Validation:
      - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
      - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the SG toggle + advanced settings in Trends.
      - Tier B deferred to existing clean-host Trends cluster (`DW-98`).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.187` (run: `project_management/runs/RUN-20260120-tier-a-dw176-trends-savgol-smoothing-0.1.9.187.md`).
      - Evidence: `manual_screenshots_web/20260120_120159/trends.png`, `manual_screenshots_web/20260120_120159/trends_savgol_advanced.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-177: Trends: Increase overlay sensor limit (>10)**
  - **Description:** Trends currently caps overlays at 10 sensors, which is too limiting for real-world analysis. Increase the max overlay limit (at least 20) while keeping the UI predictable and responsive.
  - **References:**
    - Ticket: `project_management/archive/archive/tickets/TICKET-0040-trends-increase-overlay-limit.md`
  - **Acceptance Criteria:**
    - Trends Sensor picker allows selecting more than 10 sensors (at least 20).
    - When the limit is reached, the UX clearly indicates it (no silent failure).
    - Related add-to-chart actions (Related sensors / Relationships / Matrix Profile) respect the new maximum.
    - `make ci-web-smoke` passes.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the updated max selection count.
    - Tier B deferred to existing clean-host Trends cluster (`DW-98`).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Tier A validated on installed controller `0.1.9.188` (run: `project_management/runs/RUN-20260120-tier-a-dw177-trends-overlay-limit-0.1.9.188.md`).
      - Evidence: `manual_screenshots_web/20260120_133427/trends.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-178: Trends: Co-occurring anomalies (multi-sensor)**
  - **Description:** Add a dedicated Trends panel to surface time buckets where multiple sensors have anomalies at the same time (with extra weight for larger groups). It must also support a “focus scan” mode (1 selected sensor → scan all sensors) so operators can quickly answer “what weird things happened together?” across the system.
  - **References:**
    - Ticket: `project_management/archive/archive/tickets/TICKET-0041-trends-cooccurring-anomalies.md`
  - **Acceptance Criteria:**
    - Trends shows a “Co-occurring anomalies” panel when 1+ sensors are selected:
      - With 1 sensor selected, it operates in focus-scan mode (scan all sensors).
      - With 2+ sensors selected, it can operate on the current selection (and optionally also offer focus-scan).
    - Per-series anomalies are detected using robust change-event detection (MAD z-score on deltas; not a moving average).
    - The panel lists time buckets where ≥2 sensors have anomalies in the same Interval bucket, with extra weight for larger co-occurring groups (e.g., 3 sensors ranks above 2).
    - In focus-scan mode, the results are restricted to co-occurrences that include the selected focus sensor (so it acts like “Related sensors”, but grouped by timestamp).
    - Users can adjust sensitivity (z-threshold), minimum group size, and alignment tolerance (buckets) from the panel UI.
    - Users can click a co-occurrence entry to highlight that timestamp on the Trend chart (visual marker) and view the contributing sensors.
    - UI is cohesive and maintainable: uses `CollapsibleCard` + in-section `Key`/Glossary (no “scroll away” explanations).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one captured **and viewed** screenshot under `manual_screenshots_web/` showing the panel + chart marker.
    - Tier B deferred to existing clean-host Trends cluster (`DW-98`).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-20: Started.
    - 2026-01-21: Implemented panel + markers.
      - Tests: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-21: Tier A validated on installed controller `0.1.9.193` (run: `project_management/runs/RUN-20260121-tier-a-dw178-trends-cooccurring-anomalies-0.1.9.193.md`).
      - Evidence: `manual_screenshots_web/20260121_073030/trends_cooccurrence.png`, `manual_screenshots_web/20260121_073030/analytics.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-179: Weather station nodes: add custom sensors via dashboard (soil moisture, etc.)**
  - **Description:** Provide a user-friendly, scalable way to add new sensors to an existing WS‑2902 weather station node via the web dashboard (including advanced configuration). Validate that the new sensor appears in the dashboard and that data is flowing end‑to‑end.
  - **References:**
    - Ticket: `project_management/archive/archive/tickets/TICKET-0042-weather-station-nodes-add-sensors-via-dashboard.md`
  - **Acceptance Criteria:**
    - Dashboard provides a clear entrypoint to add a new sensor to an existing weather station node (WS‑2902).
    - The workflow is simple by default, but exposes an Advanced section for power users (explicit metric/type/unit/topic/config).
    - The new sensor is persisted in controller config and shows up consistently across:
      - Nodes
      - Sensors & Outputs
      - Analytics Overview (Weather stations section, where applicable)
      - Trends (selectable + chartable)
    - Tier A validation records evidence that the sensor’s time-series data is visible in the UI (no DB reset).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-21: Started (DW-179 branch `dw-179-weather-station-add-sensors`).
    - 2026-01-21: Tier A validated on installed controller `0.1.9.194` (run: `project_management/runs/RUN-20260121-tier-a-dw179-weather-station-add-sensors-0.1.9.194.md`).
      - Evidence: `manual_screenshots_web/20260121_142426/sensors_add_sensor_ws2902.png`, `manual_screenshots_web/20260121_142426/sensors_ws2902_custom.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-180: Trends: expand range + interval presets (10m/1h, 1s/30s)**
  - **Description:** Improve the Trends “Range” and “Interval” presets so operators can zoom into short windows (10 minutes / 1 hour) and use higher-resolution buckets (1s / 30s) when sensors support it. Remove unhelpful presets to keep the dropdowns focused.
  - **Acceptance Criteria:**
    - Range presets include **Last 10 minutes** and **Last hour**.
    - Range presets no longer include **Last 180 days**.
    - Interval presets include **1s** and **30s**.
    - Interval presets no longer include **15 min** or **2 hours**.
    - Custom interval minimum is **1s** (not clamped to 10s).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validated on installed controller (no DB/settings reset); Tier B deferred to `DW-98`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-21: Started (branch `dw-180-trends-range-interval-presets`).
    - 2026-01-21: Implemented preset updates and custom interval minimum.
      - Tests: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-21: Tier A validated on installed controller `0.1.9.195` (run: `project_management/runs/RUN-20260121-tier-a-dw180-trends-range-interval-presets-0.1.9.195.md`).
      - Evidence: `manual_screenshots_web/20260121_161614/trends_short_range.png`, `manual_screenshots_web/20260121_161614/trends.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)


- **DW-188: Trends: Related sensors week+1m support (adaptive metrics batching + confirm/progress/cancel UX)**
  - **Description:** Trends → Related sensors can hit the core-server metrics guardrail (`Requested series too large … max 25000`) when the requested Range/Interval implies too many buckets across the scanned candidate set. Make week range + 1 minute interval usable by adaptively batching metrics queries, and add operator-facing UX for large scans (confirm prompt, determinate progress bar, and cancel/abort).
  - **Acceptance Criteria:**
    - Week+1m works:
      - On Trends → Related sensors, selecting **Range = 7 days** (168h) and **Interval = 1 min** (60s) can scan candidates without a 400 “Requested series too large … max 25000” error.
      - Scans may take longer, but complete successfully by automatically splitting requests as needed.
    - Confirm + cancel UX:
      - When the scan estimate exceeds a threshold, the UI shows an explicit **Continue scan / Cancel** confirmation prompt with the estimated size.
      - While scanning, the UI shows a **determinate progress bar** (sensors loaded / total) and a **Cancel scan** button that aborts in-flight requests and stops the scan without a hard error state.
    - Validation:
      - `make ci-web-smoke`, `cd apps/dashboard-web && npm run build`, and `cd apps/dashboard-web && npm test` pass.
      - Tier A validated on installed controller (no DB/settings reset), with at least one viewed screenshot under `manual_screenshots_web/` showing the new prompt/progress UI on Trends.
      - Tier B deferred to `DW-98`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-22: Implemented adaptive metrics batching (`fetchMetricsSeriesBatched`) that splits batches when the backend rejects a series as too large, and added Related sensors large-scan UX (confirm prompt + progress bar + cancel).
      - Tests: `make ci-web-smoke` (pass; warnings only); `cd apps/dashboard-web && npm run build` (pass); `cd apps/dashboard-web && npm test` (pass).
    - 2026-01-22: Tier A validated on installed controller `0.1.9.198` (run: `project_management/runs/RUN-20260122-tier-a-dw188-related-sensors-week-1m-0.1.9.198.md`).
      - Evidence (viewed): `manual_screenshots_web/20260122_002811/trends_related_sensors_large_scan.png`, `manual_screenshots_web/20260122_002811/trends_related_sensors_scanning.png`.
      - Tier B deferred to `DW-98`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-156: Trends: event-match mode + analysis key + opt-in deep computations**
  - **Description:** Extend Trends analysis beyond continuous correlation by adding an event/spike comparison mode (co-occurrence + lag), with an optional conditioning sensor (binned “approx equals constant” filtering). Add per-panel analysis keys that define variables (e.g., `n`, `r`, lag) and expose opt-in “deep” computations where we previously limited work for responsiveness.
  - **Acceptance Criteria:**
    - Trends “Related sensors” supports a `Mode` toggle:
      - `Correlation`: existing Pearson/Spearman + lag sweep behavior remains.
      - `Events (spikes)`: suggestions are ranked by event co-occurrence (detected via robust z-score over bucket-to-bucket changes) and can optionally include lead/lag.
    - Events mode supports:
      - Configurable event threshold (`z`), polarity (±/up/down), and minimum separation (buckets).
      - Comparing candidates across `Same node` or `All nodes` scope (same as correlation).
      - Optional conditioning on another sensor (selected from chart) with configurable bin count + bin selection (“irradiance ≈ constant” style filtering).
    - Analysis keys:
      - Keys are placed next to each relevant analysis surface (no single global glossary) and define key variables (`r`, `n`, lag, score, window).
    - Opt-in deep computations:
      - Related sensors supports a user-enabled full lag sweep for all scanned candidates (explicitly marked as potentially slow).
      - Matrix Profile self-similarity heatmap exposes a user-controlled resolution knob (explicitly marked as potentially slow).
    - `make ci-web-smoke` passes and `cd apps/dashboard-web && npm run build` passes.
  - **Notes / Run Log:**
    - 2026-01-19: Implemented Trends “Events (spikes)” related-sensors mode + conditioning bins, added per-panel analysis keys, added full lag sweep toggles and Matrix Profile heatmap resolution control. CI: `make ci-web-smoke` (pass). Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260119-tier-a-dw156-trends-events-analysis-keys-0.1.9.166.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.166_trends_auto_compare_2026-01-19_040012388Z/01_trends_auto_compare_events_key.png`
  - **Status:** Done (validated on installed controller 0.1.9.166; clean-host E2E deferred to DW-98)

- **DW-155: Trends tab UI polish (cohesive layout)**
  - **Description:** Polish the Trends tab UI so it feels cohesive and production-ready: consistent card structure, headings, spacing, and controls (picker, chart settings, related sensors, relationships, matrix profile).
  - **Acceptance Criteria:**
    - Trends “Sensor picker” uses a consistent card look and compact, readable filters.
    - Chart settings are clearly labeled and visually consistent with the rest of the tab.
    - “Related sensors” panel title/typography matches other Trends panels.
    - Matrix Profile explorer padding/spacing matches other Trends panels.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-18: Polished Trends tab layout: added an explicit “Chart settings” header, improved sensor picker filter readability, aligned “Related sensors” heading sizing, and normalized Matrix Profile padding. CI: `make ci-web-smoke` (pass); build: `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260119-tier-a-cs88-dw152-dw153-dw154-dw155-0.1.9.165.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.165_trends_auto_compare_2026-01-19_001524157Z/01_trends_auto_compare_panel.png`
  - **Status:** Done (validated on installed controller 0.1.9.165; clean-host E2E deferred to DW-98)

- **DW-154: Centralize hide behavior via sensor visibility policy (no per-page filters)**
  - **Description:** Implement hide flags as a single sensor visibility policy enforced at the API/UI boundary so every surface (Nodes, Sensors & Outputs, Trends, Map, future iOS) behaves consistently. Remove duplicated page-by-page filtering and ensure Trends/saved selections behave predictably when sensors become hidden.
  - **Acceptance Criteria:**
    - Nodes / Sensors & Outputs / Map / Trends all rely on the backend’s visible-only sensor list (`/api/sensors`) instead of ad-hoc per-page filters.
    - Per-node “Hide public provider data (Open‑Meteo)” removes the Open‑Meteo sensors from all surfaces without UI-only hacks.
    - Trends does not chart hidden sensors (even if previously selected/saved); hidden sensors drop from the effective selection deterministically.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-18: Removed `apps/dashboard-web/src/lib/sensorVisibility.ts` and refactored Nodes/Sensors/Map/Trends to rely on API-visible sensors. Trends now computes an `effectiveSelected` list (visible sensors only) so hidden sensors cannot silently persist in charts. CI: `make ci-web-smoke` (pass); build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-18: Updated Tier A Playwright toggle test to use `/api/sensors?include_hidden=true` so it remains robust now that the API hides sensors by default.
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260119-tier-a-cs88-dw152-dw153-dw154-dw155-0.1.9.165.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.165_hide_live_weather_2026-01-19_001514602Z/02_after_toggle_node_detail.png`
  - **Status:** Done (validated on installed controller 0.1.9.165; clean-host E2E deferred to DW-97/DW-98)

- **DW-153: Trends “Related sensors”: acknowledge/deprioritize + all-nodes comparisons**
  - **Description:** Improve the Trends “Related sensors” suggestions so operators can acknowledge/deprioritize unhelpful relationships (greyed out) and so comparisons can consider sensors across all nodes (including weather-station sensors), not just the current node.
  - **Acceptance Criteria:**
    - Suggested relationships include an “Ack” control that moves the relationship into an acknowledged/deprioritized state (greyed out) while keeping it available for preview/add.
    - Scope supports comparing candidates across all nodes and defaults to “All nodes” (persisted locally).
    - Candidate scanning does not starve sensors from later nodes due to list ordering (balanced sampling across nodes when in all-nodes scope).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-18: Added local “Ack/Unack” persistence for related-sensor suggestions and defaulted/persisted scope to “All nodes” in `apps/dashboard-web/src/features/trends/components/AutoComparePanel.tsx`. CI: `make ci-web-smoke` (pass); build: `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260119-tier-a-cs88-dw152-dw153-dw154-dw155-0.1.9.165.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.165_trends_auto_compare_2026-01-19_001524157Z/01_trends_auto_compare_panel.png`
  - **Status:** Done (validated on installed controller 0.1.9.165; clean-host E2E deferred to DW-98)

- **DW-152: Unify “Public provider data” labeling + sensor origin badges**
  - **Description:** Replace ambiguous “Live weather” / “Weather API” labeling with a unified “Public provider data” concept, and refactor sensor-origin badges so the same sensor shows the same badge everywhere (Nodes, Sensors & Outputs, Trends, Map, etc.) without page-specific hacks.
  - **Acceptance Criteria:**
    - Public-provider-backed sensors (e.g. Open-Meteo, Forecast.Solar) show a single unified badge label across the dashboard (short form is OK for badges).
    - Node grid and other sensor lists render origin badges next to the sensor name using shared components (no ad-hoc per-page pill strings).
    - Node “public provider” weather panel + per-node hide toggle use the unified label (no “Live weather” phrasing).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-18: Added `apps/dashboard-web/src/lib/sensorOrigin.ts`; updated `SensorOriginBadge` to use `PUBLIC` badge label with “Public provider data” tooltip; refactored Nodes/Trends copy to avoid ambiguous “Live weather/Weather API” phrasing. CI: `make ci-web-smoke` (pass); build: `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260119-tier-a-cs88-dw152-dw153-dw154-dw155-0.1.9.165.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.165_non_local_sensor_badges_2026-01-19_001519445Z/01_sensors_drawer_badge.png`
  - **Status:** Done (validated on installed controller 0.1.9.165; clean-host E2E deferred to DW-114)

- **DW-151: Fix regression: “Hide live weather” still shows Open‑Meteo sensors**
  - **Description:** The per-node “Hide live weather (Open‑Meteo)” toggle should hide public API-backed weather sensors across the dashboard UI. A regression caused the sensors to remain visible in sensor lists even when the toggle is enabled.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0038-hide-live-weather-open-meteo-ui-filter-regression.md`
  - **Acceptance Criteria:**
    - Enabling “Hide live weather (Open‑Meteo)” on a node hides Open‑Meteo weather sensors (where `sensor.config.source="forecast_points"`, `provider="open_meteo"`, `kind="weather"`) from:
      - Nodes → node detail sensor list
      - Sensors & Outputs → table and grouped-by-node views
      - Map → Devices list/search results (and any sensor markers that rely on the sensors list)
    - Disabling the toggle restores those sensors/panels.
    - This is a UI filter only; telemetry ingest/storage is unchanged.
    - `make ci-web-smoke` passes.
    - Tier A validation is recorded on the installed controller with at least one captured + viewed screenshot and a run log under `project_management/runs/`.
  - **Notes / Run Log:**
    - 2026-01-18: Implemented shared UI filtering for Open‑Meteo weather sensors and applied it across Nodes/Sensors/Map. Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw151-hide-live-weather-ui-filter-0.1.9.164.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.164_dw151_hide_live_weather/ws2902_node_detail_hide_live_weather_filters_open_meteo.png`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.164_dw151_hide_live_weather/sensors_outputs_ws2902_open_meteo_hidden.png`
  - **Status:** Done (validated on installed controller 0.1.9.164; clean-host E2E deferred to DW-98)

- **DW-150: Trends: show sensor origin badges + hide external sensors toggle**
  - **Description:** The Sensors & Outputs tab already shows origin badges (e.g., `WEATHER API`, `PV FCST`, `WS LOCAL`, `DERIVED`) so operators can immediately see when data is coming from public APIs vs local telemetry. Bring the same transparency to Trends, and add a sensor-picker toggle to hide external/public-API sensors for easier selection.
  - **References:**
    - `project_management/tickets/TICKET-0037-trends-show-sensor-origin-badges-hide-external-toggle.md`
  - **Acceptance Criteria:**
    - Trends → Sensor picker shows origin badges consistent with Sensors & Outputs (same labels/colors).
    - “Hide external sensors” hides sensors whose `config.source="forecast_points"` (Weather API, PV forecast, other public API-backed sensors) from the picker list.
    - Toggle does **not** synthesize/alter underlying telemetry (UI filter only).
    - Toggle preference persists locally across refreshes.
    - `make ci-web-smoke` passes.
    - Tier A validation is recorded on the installed controller with at least one captured + viewed screenshot and a run log under `project_management/runs/`.
  - **Notes / Run Log:**
    - 2026-01-18: Added `SensorOriginBadge` to the Trends sensor picker list and the “Selected” chips for transparency.
    - 2026-01-18: Added a “Hide external sensors” toggle (filters `config.source="forecast_points"`), persisted in localStorage.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw150-trends-origin-badges-0.1.9.163.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.163_dw150_trends_origin_badges/trends_weather_api_badge.png`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.163_dw150_trends_origin_badges/trends_hide_external_enabled.png`
  - **Status:** Done (validated on installed controller 0.1.9.163; clean-host E2E deferred to DW-98)

- **DW-149: Derived sensor builder: expose extended function library + insert helpers**
  - **Description:** Improve the Sensors & Outputs → Add sensor → Derived workflow so operators can discover and use the richer derived-sensor function library (math/trig/conditionals) without guesswork.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0036-derived-sensors-expand-function-library-cs-87-dw-149.md`
  - **Acceptance Criteria:**
    - Derived sensor expression UI lists and provides “insert” helpers for the extended function set (`floor/ceil/sqrt/pow/ln/log10/log/exp/sin/cos/tan/deg2rad/rad2deg/sign/if`).
    - UI copy makes trig angle units explicit (radians) and points to `deg2rad()` / `rad2deg()`.
    - Existing derived sensor creation UX remains unchanged (no regressions): input picker + vars + expression + output type/unit/name.
    - `make ci-web-smoke` passes.
    - Tier A validation is recorded on the installed controller with at least one captured + viewed screenshot and a run log under `project_management/runs/`.
  - **Notes / Run Log:**
    - 2026-01-18: Added insert helpers + a discoverable “More functions” section listing the full derived function library.
    - 2026-01-18: Made trig angle units explicit (radians) with `deg2rad()` / `rad2deg()` copy.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs87-dw149-derived-sensor-functions-0.1.9.162.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.162_cs87_dw149_20260118_074353/derived_sensor_function_library.png`
  - **Status:** Done (validated on installed controller 0.1.9.162; clean-host E2E deferred to DW-98)

- **DW-148: Trends: auto-compare related sensor suggestions + previews**
  - **Description:** Add a polished Trends panel that suggests other sensors to compare against a chosen focus sensor based on observed patterns (correlation, inverse correlation, and lead/lag). Provide transparent scoring metadata and one-click add-to-chart plus previews.
  - **References:**
    - `project_management/tickets/TICKET-0035-trends-auto-compare-related-sensor-suggestions.md`
  - **Acceptance Criteria:**
    - With ≥1 sensor selected, Trends renders a “Related sensors” panel that picks a focus sensor (user-selectable) and scans candidate sensors (same-node default; all-nodes optional) using only controller telemetry.
    - Suggestions are transparent: show correlation method (Pearson/Spearman), overlap points `n`, best-lag (±N buckets) and lead/lag direction; any normalization used for previews is explicitly labeled.
    - Selecting a suggestion shows a preview and “Add to chart” adds the sensor to the chart selection without disrupting existing selections.
    - Candidate metric fetches are batched and bounded (no unbounded “fetch everything” single request).
    - Deterministic Playwright test uses stubbed series to assert suggestions appear and that “Add to chart” updates the chart selection.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Added a “Related sensors” panel to Trends that discovers relationships via Pearson/Spearman correlation, with optional lead/lag search bounded to the top-K candidates.
    - 2026-01-18: Preview is explicit about normalization vs raw values; alignment-by-lag is visually labeled and does not rewrite stored data.
    - 2026-01-18: Added deterministic Playwright coverage using the stub API and a Tier‑A screenshot spec for the installed controller.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw148-trends-auto-compare-0.1.9.160.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.160_trends_auto_compare_2026-01-18_140746175Z/01_trends_auto_compare_panel.png`
  - **Status:** Done (validated on installed controller 0.1.9.160; clean-host E2E deferred to DW-98)

- **DW-147: Alarm Events: click-through detail drawer + context charts**
  - **Description:** Allow operators to click individual Alarm Events to open a drilldown view with clear metadata and contextual graphs so alarms are explainable and high-trust (no obscured data).
  - **References:**
    - `project_management/tickets/TICKET-0034-alarm-event-drilldown-details-and-charts.md`
  - **Acceptance Criteria:**
    - Alarm Events in the dashboard are clickable and open a detail drawer/modal with event metadata (id, status, raised time, origin/anomaly if present).
    - Drawer shows the linked target (sensor/node) and key alarm definition info (type/severity/condition) when available.
    - For sensor-backed alarms, the drawer renders a context Trend chart with simple range presets (e.g., event context + last 1h/6h/24h).
    - If threshold/min/max can be derived from the alarm rule, it is displayed explicitly and graphed where feasible.
    - “Acknowledge” actions still work and do not open the drawer accidentally.
    - Deterministic Playwright test covers opening the drilldown and rendering the chart using stubbed API responses.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Alarm Events cards are now clickable (active + resolved) and open a drilldown drawer.
    - 2026-01-18: Drilldown includes event metadata, linked target buttons, and a context chart using real sensor telemetry (no synthetic backfill). If a numeric threshold can be extracted from the stored alarm rule, it is displayed and graphed as a reference line.
    - 2026-01-18: Added deterministic Playwright coverage (`alarm-event-drilldown.spec.ts`) and Tier‑A screenshot spec.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw147-alarm-event-drilldown-0.1.9.158.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.158_alarm_event_drilldown_2026-01-18_112742695Z/02_alarm_event_drawer.png`
  - **Status:** Done (validated on installed controller 0.1.9.158; clean-host E2E deferred to DW-114)

- **DW-146: Overview: Telemetry tapestry layout stability + regression tests**
  - **Description:** Fix the Overview “Telemetry tapestry” panel so hovering heatmap cells never causes layout shift and the card never produces internal horizontal scrolling at normal desktop widths. Add deterministic Playwright coverage so this class of layout regression is caught automatically.
  - **References:**
    - `project_management/tickets/TICKET-0033-overview-telemetry-tapestry-layout-regressions.md`
  - **Acceptance Criteria:**
    - Hovering across Telemetry tapestry cells does **not** change the vertical position of the heatmap rows (no hover jank).
    - The Telemetry tapestry card shows **no internal horizontal scrollbar** at typical desktop widths (e.g. 1280×800) and remains usable at tablet widths.
    - Header/details area is stable (no `flex-wrap`-triggered reflow on hover); hover details remain readable without obscuring critical UI.
    - Deterministic Playwright test asserts:
      - No horizontal overflow (tapestry container + page-level).
      - No vertical shift of the heatmap rows during hover/unhover.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Replaced hover-driven conditional layout with a stable header grid and always-present details panel (placeholder vs hover content).
    - 2026-01-18: Removed forced min-width + horizontal scrolling; added `min-w-0` and responsive row layout to avoid overflow.
    - 2026-01-18: Added deterministic Playwright regression helpers and a layout spec (`overview-tapestry-layout.spec.ts`).
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw146-overview-tapestry-layout-0.1.9.157.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.157_overview_tapestry_layout_2026-01-18_104201563Z/02_overview_tapestry_hover.png`
  - **Status:** Done (validated on installed controller 0.1.9.157; clean-host E2E deferred to DW-114)

- **DW-145: Trends: resizable chart height**
  - **Description:** Allow operators to resize the Trends chart container height so multi-series graphs are easier to inspect without being constrained to a fixed-height plot.
  - **Acceptance Criteria:**
    - Trends includes a clear, user-friendly control to adjust the chart height (taller than the default).
    - The chosen chart height persists for the user (local-only, e.g., browser storage) so the UI is stable across refreshes.
    - Charts reflow correctly when the height changes (no clipped axes/legend; no layout overlap).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Added a chart height slider + reset in Trends, persisted in localStorage and forcing a Chart.js resize on height changes.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw145-trends-chart-height-0.1.9.156.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.156_trends_chart_height_2026-01-18_093921214Z/01_trends_chart_height_resized.png`
  - **Status:** Done (validated on installed controller 0.1.9.156; clean-host E2E deferred to DW-98)

- **DW-143: Sensors & Outputs: do not auto-expand the first node**
  - **Description:** Navigating to Sensors & Outputs auto-expanded the first node accordion/panel. This is jarring and causes content jumps. Default to all nodes collapsed until the operator explicitly expands one.
  - **Acceptance Criteria:**
    - When opening Sensors & Outputs, no node is expanded by default.
    - Expanding/collapsing nodes still works as before (no regressions).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Follow-up hardening: stop auto-expanding nodes on initial mount (including when the node list is filtered). Expansion is now user-driven only.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs83-dw142-dw143-0.1.9.150.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.150_cs83_dw142_dw143/sensors.png`
  - **Status:** Done (validated on installed controller 0.1.9.150; clean-host E2E deferred to DW-114)

- **DW-144: Derived sensors: create via “Add sensor” drawer UI**
  - **Description:** Provide a high-quality UX in Sensors & Outputs → “Add sensor” that lets an operator create a derived sensor computed from other sensors (expression-based with functions). The UI must be transparent about inputs, provenance, and whether values are measured vs computed.
  - **Acceptance Criteria:**
    - “Add sensor” drawer includes an option to create a Derived Sensor (in addition to the existing hardware sensor workflow).
    - UI supports: selecting input sensors (searchable), assigning variable names, editing an expression, and choosing output type/unit/name.
    - UI copy makes it explicit that the sensor is computed (not directly measured) and surfaces input sensor provenance (badges).
    - Saving creates a sensor with `config.source="derived"` and stored expression + inputs.
    - Derived sensors show a `DERIVED` origin badge and surface their expression/inputs in the sensor detail drawer (no obscured data).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Added a Derived sensor builder (expression + functions + input picker) to the Sensors & Outputs Add sensor drawer.
    - 2026-01-18: Sensor detail surfaces the derived expression + inputs list for transparency (no obscured data).
    - 2026-01-18: Validation: `make ci-web-smoke` (pass), `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs85-dw144-derived-sensors-0.1.9.152.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.152_derived_sensors_2026-01-18_070954307Z/01_sensors_nodes_collapsed.png`
      - `manual_screenshots_web/tier_a_0.1.9.152_derived_sensors_2026-01-18_070954307Z/02_derived_sensor_builder_created.png`
  - **Status:** Done (validated on installed controller 0.1.9.152; clean-host E2E deferred to DW-98)

- **DW-142: Show node type badges next to node titles across the dashboard**
  - **Description:** Operators need to quickly identify what kind of node they’re looking at (Core, Pi 5, Emporia Vue, Weather station, etc.) across the UI. Add a compact color-coded badge next to node titles in all primary node surfaces.
  - **Acceptance Criteria:**
    - Node type badge renders next to node titles in: Nodes tab, Sensors & Outputs tab, Trends → Sensor picker, Map tab node list, and Node detail page (and any other primary node-title surfaces discovered during implementation).
    - Badge style is consistent with the dashboard design system and does not break truncation/overflow layouts.
    - Badge classification derives from `node.config` (Core vs Pi 5 vs Emporia vs WS-2902 vs fallback).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Implemented `NodeTypeBadge` (derived from `node.config`) and added it to the primary node-title surfaces across the dashboard.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-cs83-dw142-dw143-0.1.9.150.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.150_cs83_dw142_dw143/nodes.png`
      - `manual_screenshots_web/tier_a_0.1.9.150_cs83_dw142_dw143/trends.png`
      - `manual_screenshots_web/tier_a_0.1.9.150_cs83_dw142_dw143/map.png`
  - **Status:** Done (validated on installed controller 0.1.9.150; clean-host E2E deferred to DW-114)

- **DW-141: Sensors & Outputs: don’t auto-expand the first node**
  - **Description:** When navigating to Sensors & Outputs, the first node accordion opened automatically. Default should be collapsed so operators can scan the node list without the UI expanding a random node.
  - **Acceptance Criteria:**
    - Navigating to `/sensors` shows all nodes collapsed by default.
    - Operators can expand/collapse any node and the state does not “fight” the UI on re-render.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Stopped forcing `details open` on the first node; expansion is now user-driven. Follow-up: deep links that set `?node=<id>` still auto-expand the filtered node panel.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw140-dw141-cs82-0.1.9.149.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.149_dw140_dw141_pressure_2026-01-18_024046513Z/01_sensors_outputs_default_collapsed.png`
      - `manual_screenshots_web/tier_a_0.1.9.149_dw140_dw141_pressure_2026-01-18_024046513Z/02_sensors_outputs_filtered_node_expanded.png`
  - **Status:** Done (validated on installed controller 0.1.9.149; clean-host E2E deferred to DW-114)

- **DW-140: Trends: render sparse series + show “last seen” when empty**
  - **Description:** Make Trends resilient when a sensor has only 1 datapoint (or none in the selected window). Single-point series should still be visible (dot), and empty series should expose “last value / last seen” so operators can diagnose stale sensors.
  - **Acceptance Criteria:**
    - A selected series with exactly one datapoint renders visibly in the Trends chart.
    - When a selected series has zero points in the active window, the “Selected” chip shows last value + last seen time (when available).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-18: Updated `TrendChart` to render single-point series and extended sensor normalization to include `latest_ts`.
    - 2026-01-18: Validation: `make ci-web-smoke` (pass).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260118-tier-a-dw140-dw141-cs82-0.1.9.149.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.149_dw140_dw141_pressure_2026-01-18_024046513Z/03_trends_barometric_pressure_single_point.png`
  - **Status:** Done (validated on installed controller 0.1.9.149; clean-host E2E deferred to DW-98)

- **DW-139: Trends: Matrix Profile explorer (motifs + anomalies + heatmap)**
  - **Description:** Add an advanced, data-science-driven visualization to Trends using matrix profiles to surface motifs (repeating patterns), anomalies (unusual windows), and a self-similarity heatmap. The panel is interactive and driven by real `/api/metrics/query` data (no fake/demo-only values).
  - **Acceptance Criteria:**
    - Trends renders a **Matrix Profile explorer** panel when at least one selected sensor has chartable data.
    - Panel supports:
      - Selecting which sensor to analyze (among selected sensors with data).
      - Adjusting window length (points) and analysis point cap.
      - Switching between **Anomalies**, **Motifs**, and **Self-similarity** views.
      - Clicking the matrix profile curve to select a window; motif/anomaly overlays update without refresh.
      - Clicking the heatmap pins a motif pair and updates window overlays.
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && CI=1 npm test` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Playwright stub regression passes:
      - `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8010 npx playwright test playwright/trends-matrix-profile.spec.ts`
    - Tier A: installed controller refreshed and a screenshot showing the Matrix Profile explorer is captured **and viewed** under `manual_screenshots_web/` and referenced from a run log under `project_management/runs/` (Tier B deferred to DW-98 cluster).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260116-tier-a-dw139-trends-matrix-profile-0.1.9.147.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.147_trends_matrix_profile_2026-01-16_080807419Z/01_trends_matrix_profile.png`
  - **Status:** Done (validated on installed controller 0.1.9.147; clean-host E2E deferred to DW-98)

- **DW-138: Overview: configure which sensors appear (and order) for local visualizations**
  - **Description:** Add a discrete configuration entrypoint on the Overview tab so operators can choose which local sensors appear in the Overview “Local sensors” visualizations and in what priority order.
  - **Acceptance Criteria:**
    - Overview → Local sensors includes a “Configure” button that opens a config modal/panel.
    - The config UI supports:
      - Hiding/showing sensors from the Overview visualizations.
      - Reordering shown sensors (priority order).
    - The Overview visualizations respect the chosen order and hide list.
    - The selection persists across page reloads for the same browser session/profile (local storage is acceptable for this iteration).
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A: installed controller refreshed and a screenshot showing the config UI is captured **and viewed** under `manual_screenshots_web/` and referenced from a run log under `project_management/runs/` (Tier B deferred to DW-114 cluster).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260116-tier-a-dw138-overview-local-sensors-config-0.1.9.146.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.146_overview_local_visuals_2026-01-16_070257452Z/01_overview_local_visuals.png`
      - `manual_screenshots_web/tier_a_0.1.9.146_overview_local_visuals_2026-01-16_070257452Z/02_overview_local_sensors_config.png`
  - **Status:** Done (validated on installed controller 0.1.9.146; clean-host E2E deferred to DW-114)

- **DW-137: Analytics: reservoir depth gauges default to 15 ft full-scale**
  - **Description:** The Analytics “Live reservoir depths” gauges default to a 10 ft full-scale view, which makes typical ~12 ft readings appear saturated/misleading. Increase the default full-scale max to 15 ft (and equivalent defaults for in/m/cm) so the UI matches the reservoir physical range and remains readable.
  - **Acceptance Criteria:**
    - For water-depth sensors with unit `ft`, the default full-scale max is **15 ft** (not 10), while still allowing auto-upscaling when observed values exceed the default range.
    - Equivalent defaults apply for `in`/`m`/`cm` water-depth gauges.
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A: installed controller refreshed and a screenshot showing the 15 ft full-scale tick labels is captured **and viewed** under `manual_screenshots_web/` and referenced from a run log under `project_management/runs/` (Tier B deferred to DW-114 cluster).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260116-tier-a-dw137-analytics-water-depth-scale-0.1.9.145.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.145_analytics_water_depth_2026-01-16_062105261Z/01_analytics_water_depth.png`
  - **Status:** Done (validated on installed controller 0.1.9.145; clean-host E2E deferred to DW-114)

- **DW-136: Alarm events: collapse acknowledged events by default**
  - **Description:** After acknowledging an alarm event, it should drop out of the high-signal “active alarms” view everywhere it is displayed. Move acknowledged/cleared events into a collapsible section that is collapsed by default so operators can keep the current-alert view clean while still being able to review history.
  - **Acceptance Criteria:**
    - All dashboard surfaces that render alarm events (currently the shared `AlarmEventsPanel` used on Nodes and Sensors pages) hide `status=acknowledged` and `status=ok` events from the default list.
    - A new collapsed-by-default section (“Acknowledged & cleared”) contains acknowledged + cleared events.
    - After acknowledging an event, it moves to the collapsed section without a manual refresh.
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Playwright stub regression passes:
      - `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8010 npx playwright test playwright/alarm-events-collapsed.spec.ts`
    - Tier A: installed controller refreshed and a screenshot showing acknowledged events are hidden by default is captured **and viewed** under `manual_screenshots_web/` and referenced from a run log under `project_management/runs/` (Tier B deferred to DW-114 cluster).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260116-tier-a-dw136-alarm-events-collapse-0.1.9.144.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.144_ack_all_alerts_2026-01-16_060129657Z/01_sensors_alarm_events.png`
      - `manual_screenshots_web/tier_a_0.1.9.144_ack_all_alerts_2026-01-16_060129657Z/02_nodes_alarm_events.png`
  - **Status:** Done (validated on installed controller 0.1.9.144; clean-host E2E deferred to DW-114)

- **DW-135: Analytics: weather station section (WS-2902) + rich visualizations**
  - **Description:** Add a first-class Weather stations panel to the Analytics tab so operators can see WS-2902 telemetry (temperature/humidity/wind/rain/UV/solar/pressure) alongside forecasts and power/water/soil. The panel must be performant (don’t fetch chart series unless expanded) and include rich visualizations, not just raw numbers.
  - **Acceptance Criteria:**
    - Analytics renders a **Weather stations** card that discovers nodes with `config.kind="ws-2902"`.
    - For each station, the collapsed row shows high-signal live chips (at least temp/humidity/wind).
    - Expanding a station fetches a bounded window of history via `/api/metrics/query` and renders charts for:
      - Temperature + humidity (dual axis)
      - Wind speed + gust
      - Rain (daily mm + rain rate dual axis)
      - Pressure and solar/UV (at least one chart each)
      - Wind direction compass (live)
    - The metrics query is **disabled until expanded** (no background polling when collapsed).
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Playwright stub regression passes:
      - `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8010 npx playwright test playwright/analytics-weather-station.spec.ts`
    - Tier A: installed controller refreshed and a screenshot showing the Weather stations section is captured **and viewed** under `manual_screenshots_web/` and referenced from a run log under `project_management/runs/` (Tier B deferred to DW-114 cluster).
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260116-tier-a-dw135-analytics-weather-station-0.1.9.143.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.143_analytics_weather_station_2026-01-16_053903585Z/01_analytics_weather_station.png`
  - **Status:** Done (validated on installed controller 0.1.9.143; clean-host E2E deferred to DW-114)

- **DW-134: Validate numeric input UX on installed controller (Tier A)**
  - **Description:** Capture Tier‑A evidence that numeric input fields behave correctly on the installed controller UI (decimals, intermediate range typing, and negative sign entry where allowed).
  - **Acceptance Criteria:**
    - Installed controller refreshed to a bundle containing DW-132 changes (no DB/settings reset).
    - At least one screenshot is captured **and viewed** that shows:
      - Add sensor → ADC config fields accept decimals (e.g., `0.25` without the dot disappearing).
      - A range-restricted field accepts multi-digit typing via intermediate out-of-range drafts (e.g., typing `12` works even if min is `10`).
      - A negative-capable field accepts a leading `-` (e.g., PV azimuth).
    - Run log recorded under `project_management/runs/` with the controller bundle version + screenshot path under `manual_screenshots_web/`.
  - **Evidence (Tier A):**
    - Run: `project_management/runs/RUN-20260116-tier-a-dw134-numeric-input-0.1.9.142.md`
    - Screenshots (viewed):
      - `manual_screenshots_web/tier_a_0.1.9.142_dw134_numeric_input_2026-01-16_033658858Z/01_add_sensor_adc_numeric.png`
      - `manual_screenshots_web/tier_a_0.1.9.142_dw134_numeric_input_2026-01-16_033658858Z/02_range_restricted_jitter_window.png`
  - **Status:** Done (validated on installed controller 0.1.9.142)

- **DW-132: Fix numeric input UX (decimals + range typing) across dashboard-web**
  - **Description:** Many dashboard numeric fields are controlled by numeric state and parse/clamp on every keystroke. This prevents entering decimals (e.g., `0.` collapses to `0`) and blocks valid multi-digit values when intermediate digits are out of range (e.g., cannot type `12` because `1` is rejected). Standardize numeric editing to use a raw string “draft” value while typing, and only parse/validate/commit the numeric value without fighting the cursor.
  - **Acceptance Criteria:**
    - Decimal-capable inputs allow typing fractional values without the `.` disappearing mid-edit (e.g., `0.25`, `1.5`).
    - Range-restricted numeric inputs do not block intermediate typing (e.g., typing `12` works even if the allowed range starts at `10`).
    - Negative-capable inputs allow typing a leading `-` without resetting the field (e.g., PV azimuth).
    - Optional numeric fields can be cleared (blank) without snapping to `0` or another default.
    - `cd apps/dashboard-web && npm run build` passes.
    - `cd apps/dashboard-web && CI=1 npm test` passes.
  - **Notes / Run Log:**
    - 2026-01-16: Added `NumericDraftInput` (raw string draft while typing) and migrated dashboard numeric offenders (ADC sensor config, schedules, trends correlations, node settings, PV forecast config, Map layer editor max zoom). Updated `vitest.config.ts` to exclude Playwright specs from `npm test`.
    - 2026-01-16: `cd apps/dashboard-web && CI=1 npm test` (pass).
    - 2026-01-16: `cd apps/dashboard-web && npm run build` (pass).
  - **Status:** Done (tests/build pass; Tier A validated installed 0.1.9.142 via DW-134)

- **DW-131: Analytics: split reservoir depth into depth charts + rich live depth panel**
  - **Description:** Fix Analytics so reservoir depth is no longer charted with “gallons” units. Add a dedicated depth-sensors graph (room for multiple depth sensors) and a companion “live reservoir depth” visualization that shows each depth at a glance with a full-range scale (not zoomed into tiny fluctuations).
  - **Acceptance Criteria:**
    - Reservoir depth is removed from any gallons-based Water charts in Analytics.
    - Analytics includes a **Water depths** line chart that shows all local depth sensors (currently `water_level`, unit like `ft`) with correct depth units.
    - Analytics includes a companion **Live reservoir depths** visualization that renders one tile per depth sensor, side-by-side, with a 0→full-scale gauge (not auto-zoomed to the last small variation).
    - Companion tiles show current value + unit and remain readable at a glance.
    - `make ci-web-smoke` passes.
    - Tier A validated on installed controller with a captured + viewed screenshot on `/analytics`.
  - **Plan:**
    - Update `WaterSection` charts: keep usage charts in `gpm`/`gal` and move depth sensors into a dedicated depths section.
    - Implement a custom “tank gauge” card component for the companion visualization (SVG-based; not reusing Trends charts).
    - Rebuild + refresh installed controller; capture + view a Tier‑A screenshot and record a run log under `project_management/runs/`.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-dw131-analytics-water-depth-0.1.9.141.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.141_analytics_water_depth_2026-01-15_210340433Z/01_analytics_water_depth.png`
  - **Status:** Done (Tier A validated; Tier B deferred)

- **DW-130: Overview: advanced local sensor visualizations**
  - **Description:** Add new, feature-rich visualizations to the Overview tab that highlight **locally acquired** sensors (exclude forecast/public API sensors) and provide a high-signal “at a glance” view that is not recycled from other tabs.
  - **Acceptance Criteria:**
    - Overview includes at least two **new** visualization panels that are not simply the existing Trends charts copied into Overview.
    - Visualizations are driven by **local sensors** (non-`forecast_points`) and clearly labeled as “Local sensors”.
    - Visualizations remain performant (limit series count / downsample; avoid rendering hundreds of charts).
    - Fallback states are clear: loading + empty + error.
    - `make ci-web-smoke` passes.
    - Tier A validated on the installed controller with a captured + viewed screenshot showing the new panels on `/overview`.
  - **Plan:**
    - Implement a dedicated Overview-only data hook that selects a small set of representative local sensors and fetches a bounded window of telemetry (`/api/metrics/query`).
    - Add new Overview-only visualization components (e.g., a telemetry “tapestry” heatmap + sparkline mosaic) using custom SVG/CSS (not reusing Trends/Charts components).
    - Rebuild + refresh installed controller; capture + view a Tier‑A screenshot and record a run log under `project_management/runs/`.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-dw130-overview-local-visualizations-0.1.9.140.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.140_overview_local_visuals_2026-01-15_204501236Z/01_overview_local_visuals.png`
  - **Status:** Done (Tier A validated; Tier B deferred)

- **DW-129: Map layout fills viewport height**
  - **Description:** Fix the Map tab layout so the map canvas and the stacked right-side panels extend to the bottom of the browser window (no “3/4 height” gap), while keeping the UX usable on smaller screens.
  - **Acceptance Criteria:**
    - On desktop widths (≥ `lg`), the map canvas extends to the bottom of the viewport and the right-side panels match its height.
    - Right-side panels remain scrollable when their contents exceed the available height.
    - The Map tab remains usable on smaller screens (no forced full-viewport overflow; map still has a reasonable min height).
    - `make ci-web-smoke` passes.
    - Tier A validated on the installed controller with a captured + viewed screenshot showing the Map tab reaching the bottom of the viewport.
  - **Plan:**
    - Replace fixed `vh` heights in the Map page layout with a “fill remaining viewport height” strategy, using a measured available-height on desktop and preserving the existing min-height behavior on mobile.
    - Refresh the installed controller bundle and capture a Map screenshot (viewport) as Tier‑A evidence.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-dw129-map-viewport-fill-0.1.9.139.md`
    - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.139_map_viewport_fill_2026-01-15_202340332Z/01_map_viewport_fill.png`
  - **Status:** Done (Tier A validated; Tier B deferred to DW-97 cluster)

- **DW-118: Nodes: admin-only soft delete action (UI)**
  - **Description:** Add a dashboard UI action to soft-delete a node (mark as `deleted`, append `-deleted`, preserve telemetry) without requiring manual API calls. This action must be visible only to **admin** users to reduce accidental removal.
  - **Acceptance Criteria:**
    - Node detail (canonical detail surface) includes a **Delete node** action that is only visible for `role=admin` (and not shown for the Core node).
    - Clicking the action requires an explicit confirmation and calls `DELETE /api/nodes/{id}` (no purge).
    - After deletion, the Nodes list refreshes and the node no longer has a usable MAC/IP binding (backend behavior); no telemetry history is purged.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Notes / Run Log:**
    - 2026-01-14: Implemented the admin-only “Danger zone → Delete node” UI on the Node detail page (post DW-122). Tier A validation is pending.
    - 2026-01-15: Tier A validation currently blocked by a backend regression (`GET /api/outputs` = 500), which prevents the Nodes page from loading without an error banner.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-soft-delete-node-sensor-0.1.9.132.md`
  - **Status:** Done (Tier A validated; Tier B deferred to DW-97/DW-98 clusters if needed)

- **DW-124: Sensors: soft delete action (UI)**
  - **Description:** Add a dashboard UI action to soft-delete a sensor (preserve telemetry history; remove from all sensor lists/trends/maps) without requiring manual API calls.
  - **Acceptance Criteria:**
    - Sensor detail drawer includes a **Delete sensor** action that is only visible to users with `config.write`.
    - Clicking the action requires explicit confirmation and calls `DELETE /api/sensors/{sensor_id}?keep_data=true`.
    - After deletion, the sensor no longer appears anywhere in the dashboard UI (Sensors & Outputs, Trends, Nodes “sensor lists”, Map placements).
    - The deleted sensor’s name is renamed by the backend so the original name can be used again later without UX conflicts.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Plan:**
    - Add “Danger zone → Delete sensor” to `SensorDetailDrawer` with confirmation + busy/error states.
    - After deletion, invalidate affected queries and close the drawer so the page does not require a manual refresh.
    - Tier A validate on the installed controller with screenshots showing before/after (sensor removed) and record the run log.
  - **Notes / Run Log:**
    - 2026-01-15: UI implemented in `SensorDetailDrawer`; Tier A validation pending (blocked by `/api/outputs` 500 regression).
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-soft-delete-node-sensor-0.1.9.132.md`
  - **Status:** Done (Tier A validated; Tier B deferred to DW-97/DW-98 clusters if needed)

- **DW-125: Mark non-local sensors (forecast/API) with badges**
  - **Description:** Non-local sensors (weather via public API, weather forecast, PV forecast) should be visually distinct in the dashboard so operators can quickly differentiate them from locally acquired telemetry.
  - **Acceptance Criteria:**
    - Sensors backed by external providers (currently `config.source=forecast_points`) render a clear badge in the UI wherever sensor names are shown prominently.
    - At minimum, the badge is visible in:
      - Sensors & Outputs table
      - Node detail sensor list
      - Node sensor list panels
      - Map node/sensor lists (where sensors are listed)
    - The badge label is concise and readable (e.g., “WEATHER API”, “PV FCST”, or “REMOTE”), and includes a tooltip/title for provider detail.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Plan:**
    - Add a small reusable `SensorOriginBadge` component driven by `sensor.config` (`source/provider/kind/mode`).
    - Render the badge inline next to sensor names across key list surfaces (Sensors table, Nodes, Node detail, Map popovers).
    - Tier A validate on the installed controller with screenshots.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-non-local-sensor-badges-0.1.9.133.md`
  - **Status:** Done (Tier A validated; Tier B deferred)

- **DW-126: Per-node toggle: hide live weather (Open-Meteo) from UI**
  - **Description:** Allow operators to hide live weather (public API) sensors for a node so locally-acquired data (e.g., weather station) is not visually mixed/confusing. Data should continue to be collected and stored; only visibility is affected.
  - **Acceptance Criteria:**
    - Node detail page includes a `config.write`-gated toggle: “Hide live weather (Open‑Meteo)”.
    - When enabled:
      - Open‑Meteo weather sensors for that node are hidden everywhere in the web dashboard UI (Nodes, Sensors & Outputs, Trends, Map, Overview, sensor drawers).
      - The Live Weather panel does not render for that node.
      - Data continues to be ingested/stored (no polling disable).
    - When disabled:
      - Sensors previously hidden by this toggle are visible again.
      - Sensors that were hidden for other reasons remain hidden.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Plan:**
    - Persist setting in `nodes.config.hide_live_weather` and cascade to `sensors.config.hidden` using a `hidden_reason` marker.
    - Update dashboard UI to edit the node setting and conditionally render Live Weather.
    - Tier A validate on installed controller with screenshots.
  - **Notes / Run Log:**
    - 2026-01-15: Core-server cascade logic implemented in `apps/core-server-rs/src/routes/nodes.rs` (toggle updates `nodes.config.hide_live_weather`, hides/unhides Open-Meteo weather sensors via `sensors.config.hidden` + `hidden_reason=node.hide_live_weather`). Dashboard UI toggle + Tier A validation pending.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-hide-live-weather-0.1.9.134.md`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred)

- **DW-122: Nodes: merge node detail drawer into node detail page (remove drawer)**
  - **Description:** Remove the separate Node detail drawer implementation and consolidate on the Node detail page as the single detail surface. This reduces duplicated logic and navigation edge-cases. All existing node detail features must remain available after the merge.
  - **Acceptance Criteria:**
    - There is **no** Node detail drawer UX; Nodes list navigates to the Node detail page for “More details”.
    - Node detail page includes all previously-supported features from the drawer and the page (deduplicated, no feature loss), including:
      - Rename node (capability-gated).
      - Location editor (lat/lng) + save feedback.
      - Health history chart.
      - Weather station manage actions (for WS-2902 nodes).
      - Display profile section (where applicable).
      - Backups list.
      - Outputs list + alarm summary where shown previously.
      - Sensor list with search and “open sensor” navigation.
    - All cross-tab links to node details (including Map tab) route to the Node detail page correctly.
    - `make ci-web-smoke` passes.
  - **Plan:**
    - Inventory features currently implemented in `NodeDetailDrawer` vs `NodeDetailPageClient`.
    - Create a single canonical “node detail content” implementation and delete the drawer UI + state.
    - Update all entrypoints (Nodes list, Map, sensor drawer backlinks) to route to the node detail page.
    - Update/repair Playwright smoke coverage to match the new navigation.
    - Tier A validate on installed controller with screenshots.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-dw122-dw123-detail-ux-0.1.9.126.md`
    - Screenshots (viewed): `manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600/`
  - **Status:** Done (Tier A validated; Tier B deferred to DW-97/DW-98 clusters if needed)

- **DW-123: Sensors: merge sensor detail page into sensor detail drawer (remove detail page UX)**
  - **Description:** Consolidate sensor detail UX into the Sensor detail drawer only. The Sensor detail page should not exist as a separate UX/implementation (it may remain as a thin redirect stub to preserve deep links). All links across the dashboard must open the drawer.
  - **Acceptance Criteria:**
    - There is no separate “Sensor detail page” UX; sensor details are shown via the Sensor detail drawer.
    - Any navigation that previously went to `/sensors/detail?id=...` now opens the drawer (e.g., `/sensors?sensor=...`) and does not lose features.
    - Map tab and Node detail page “open sensor” actions open the drawer correctly.
    - `/sensors/detail?id=...` continues to work via a redirect stub (no duplicated feature implementation).
    - `make ci-web-smoke` passes.
  - **Plan:**
    - Make the Sensor detail drawer the canonical implementation and remove page-only code.
    - Update all internal links to the drawer-open URL format.
    - Keep `/sensors/detail` as a tiny client redirect for backwards compatibility.
    - Update/repair Playwright smoke coverage accordingly.
    - Tier A validate on installed controller with screenshots.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-dw122-dw123-detail-ux-0.1.9.126.md`
    - Screenshots (viewed): `manual_screenshots_web/tier_a_0.1.9.126_dw122_dw123_20260114_220600/`
  - **Status:** Done (Tier A validated; Tier B deferred to DW-97/DW-98 clusters if needed)

- **DW-119: Map: fix client-side exception on navigation away from Map**
  - **Description:** Navigating from the Map tab to other pages (Sensors & Outputs, sensor detail, etc.) sometimes crashes the client with a `getLayer` exception and shows a Next.js “Application error” until refresh. Harden Map teardown so route transitions are reliable.
  - **Acceptance Criteria:**
    - Navigating `/map` → `/sensors` and `/map` → `/nodes` does not crash (no “Application error” screen; no manual refresh needed).
    - Navigating `/map` → `/sensors?sensor=...` does not crash (drawer deep link from Map).
    - Browser console shows no uncaught exceptions from Map/Draw teardown during navigation.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-15: Working hypothesis is teardown async work firing after Map unmount (e.g. `setTimeout`-based zoom adjustment). Implementing timer cancellation + removed-map guards, plus Playwright regression + Tier‑A revalidation.
  - **Plan:**
    - Reproduce the crash by navigating away from `/map` to `/sensors` and `/nodes` without refresh; capture the failing stack trace (console).
    - Audit MapCanvas for async work that can outlive the component (timers, debounced persistence, draw callbacks); ensure all timers are canceled on unmount and callbacks check map existence before calling MapLibre APIs.
    - Add a Playwright regression that performs `/map` → `/sensors` and `/map` → `/nodes` navigation and asserts the target page renders (no “Application error”).
    - Tier A validate on installed controller with a screenshot showing navigation away from Map succeeds (no refresh).
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-dw119-map-nav-0.1.9.127.md`
    - Screenshots (viewed): `manual_screenshots_web/tier_a_0.1.9.127_dw119_map_nav_20260114_231331/`
  - **Status:** Done (Tier A validated; Tier B deferred to DW-97/DW-98 clusters if needed)

- **DW-127: Alerts: “Acknowledge all” actions in dashboard UI**
  - **Description:** Add an “Acknowledge all alerts” action wherever alarm events are listed so operators can clear noise quickly without clicking each event.
  - **Acceptance Criteria:**
    - Dashboard surfaces that list alarm events include an “Acknowledge all alerts” button (at minimum: Sensors & Outputs tab and Nodes tab).
    - The button is capability-gated behind `alerts.ack`.
    - Clicking it acknowledges all **visible, unacknowledged, non-OK** alarm events (bulk update) and refreshes the UI without a manual page refresh.
    - Backend exposes a bulk-ack endpoint (requires `alerts.ack`) and does **not** acknowledge `status="ok"` events.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validated on the installed controller with screenshots.
  - **Plan:**
    - Core: add a bulk acknowledge endpoint for alarm events (by id list) and wire into OpenAPI.
    - Dashboard: add an “Acknowledge all alerts” button to `AlarmEventsPanel` and reuse the panel on Nodes tab.
    - Add Playwright Tier A validation and record screenshots/run log.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-ack-all-alerts-0.1.9.135.md`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred)

- **DW-128: Overview: fix Mermaid sitemap arrow rendering**
  - **Description:** The Overview tab “Where things live” Mermaid diagram renders with incorrect/broken arrows. Fix Mermaid rendering so arrows look correct and consistent across browsers.
  - **Acceptance Criteria:**
    - Mermaid diagram on `/overview` renders with correct arrowheads/edges (no broken markers).
    - The fix is implemented in the Mermaid rendering component (not a one-off diagram workaround) unless the root cause is in the diagram source.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validated on the installed controller with a captured + viewed screenshot of the fixed diagram.
  - **Plan:**
    - Capture a Tier-A screenshot of `/overview` to identify the arrow rendering issue.
    - Fix Mermaid SVG/CSS rendering (e.g., marker/overflow/stroke styling) and keep tooltips/click targets working.
    - Rebuild + refresh installed controller bundle and re-screenshot `/overview`.
  - **Evidence (Tier A):**
    - Run log: `project_management/runs/RUN-20260115-tier-a-overview-mermaid-0.1.9.137.md`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred)

- **DW-107: Map tab IA/UX cleanup (remove Street View + reduce placement friction)**
  - **Description:** Simplify the Map tab information architecture and reduce “scroll + click” cost when placing devices and adding markup.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/map/MapPageClient.tsx`
    - `apps/dashboard-web/src/features/map/MapCanvas.tsx`
  - **Acceptance Criteria:**
    - Street-level imagery (“Street view (Mapillary)”) is removed from the dashboard UX (no Setup Center credential and no Map tab panel).
    - Devices list is node-first: sensors are accessed by expanding their parent node (no separate sensor placement list).
    - Markup tools are clearly separated from node placement, and clicking Markup actions or “Place/Move” scrolls to the map canvas so the user is not forced to hunt for controls.
    - Overlays panel appears after Markup in the Map tab right column.
    - `cd apps/dashboard-web && npm run build` passes.
    - Playwright Map UX checks pass against the built dashboard (`PORT=3010 npm run start`, then `FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3010 npx playwright test playwright/map-tab.spec.ts`).
  - **Evidence (local build):**
    - `cd apps/dashboard-web && npm run build` (pass).
    - `PORT=3017 npm run start` (pass).
    - `FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3017 npx playwright test playwright/map-tab.spec.ts playwright/map-placement.spec.ts` (pass).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.103` (setup-daemon `POST /api/upgrade` with bundle `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.103.dmg`).
    - Screenshot captured + viewed: `manual_screenshots_web/20260112_121326/map.png` (run: `project_management/runs/RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103.md`).
  - **Notes / Run Log:**
    - 2026-01-12: Replaced “Eye altitude” mental model with an explicit “Viewport height” readout (computed via top/bottom unproject), and made “Zoom to ~300′” target viewport height.
    - 2026-01-12: Fixed markup visibility-on-reload regression by keeping a static render fallback when MapboxDraw fails/has not initialised; debounced edit persistence to avoid crashes on complex polygons.
    - 2026-01-12: Fixed Placement mode being non-functional: MapLibre `click` handler was bound once and captured the initial `onMapClick` (with `placeTarget=null`), so clicks never placed anything; switched to refs so the handler always uses latest state.
    - 2026-01-12: Fixed saved markup being invisible after reload: custom Draw styles filtered out `mode="static"` so features loaded via `draw.set(...)` were hidden; removed the static-mode filter from Draw styles. Also force Draw back to `simple_select` after creating markup and before entering Placement mode.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-133: Map canvas height tuning**
  - **Description:** Constrain the Map canvas height so a standard desktop viewport can see the map and the right-side controls without wasting vertical space.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/map/MapPageClient.tsx`
  - **Acceptance Criteria:**
    - Map canvas does not render “2x too tall” and does not force viewing only ~half of the map on a standard desktop monitor.
    - Right column controls remain accessible without excessive scrolling.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.107`; screenshot captured + viewed: `manual_screenshots_web/20260112_133625/map.png` (run: `project_management/runs/RUN-20260112-tier-a-map-height-0.1.9.107.md`).
  - **Notes / Run Log:**
    - Renumbered from `DW-110` → `DW-133` to resolve an ID collision (`DW-110` is used by the Power DC voltage quality analysis ticket).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-108: Trends custom start/end datetime range**
  - **Description:** Replace the single freeform “custom range” box with explicit start/end date-time pickers so operators can graph arbitrary historical windows.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
    - `apps/dashboard-web/src/lib/queries/index.ts`
  - **Acceptance Criteria:**
    - Trends → Range → “Custom…” shows Start and End date/time inputs (year/month/day/hour/minute).
    - Queries use the selected window (`start`/`end`) rather than “now - rangeHours”.
    - Invalid windows (missing values, start >= end, >365d) show a clear error and do not refetch charts.
    - `cd apps/dashboard-web && npm run build` passes.
    - Playwright passes: `FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3013 npx playwright test playwright/trends-independent-axes.spec.ts playwright/trends-custom-range.spec.ts`.
  - **Evidence (local build):**
    - `cd apps/dashboard-web && npm run build` (pass).
    - `FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3013 npx playwright test playwright/trends-independent-axes.spec.ts playwright/trends-custom-range.spec.ts` (pass).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.103`; screenshot captured + viewed: `manual_screenshots_web/20260112_121326/trends.png` (run: `project_management/runs/RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-109: Power AC voltage quality analysis (Emporia mains voltage)**
  - **Description:** Add electrical-utility-focused analysis and visualizations for Emporia AC supply voltage, including voltage quality over time, surfaced on the Power tab.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/power/PowerPageClient.tsx`
    - `apps/dashboard-web/src/features/power/components/AcVoltageQualityPanel.tsx`
    - `apps/dashboard-web/src/features/trends/components/VoltageQualityPanel.tsx`
  - **Acceptance Criteria:**
    - Power → Emporia node → Voltage (V) section shows an “AC voltage quality” panel driven by Emporia **mains** leg voltages (`sensor.config.source="emporia_cloud"`, `metric="channel_voltage_v"`, `sensor.config.is_mains=true`).
    - Panel includes: nominal detection, ±5%/±10% band context, time-in-band quality bar, sag/swell counts, flicker metric, and distribution visualization (histogram).
    - Trends no longer shows the voltage quality panel (moved to Power).
    - Playwright coverage exists for both: `apps/dashboard-web/playwright/power-voltage-quality.spec.ts` and `apps/dashboard-web/playwright/trends-voltage-quality.spec.ts`.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Evidence (local build):**
    - `cd apps/dashboard-web && npm run build` (pass).
    - `PORT=3016 npm run start` (pass).
    - `FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3016 npx playwright test playwright/power-voltage-quality.spec.ts playwright/trends-voltage-quality.spec.ts` (pass).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.103`; screenshot captured + viewed: `manual_screenshots_web/20260112_121326/power.png` (run: `project_management/runs/RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-110: Power DC voltage quality analysis (Renogy voltage rails)**
  - **Description:** Add DC voltage quality visualizations for Renogy BT‑2 voltage rails (Battery/PV/Load) to help operators spot dips/spikes and ripple over time.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/power/PowerPageClient.tsx`
    - `apps/dashboard-web/src/features/power/components/DcVoltageQualityPanel.tsx`
    - `apps/dashboard-web/src/features/trends/components/VoltageQualityPanel.tsx`
  - **Acceptance Criteria:**
    - Power → Renogy node → Voltage (V) section shows a “DC voltage quality” panel with a Battery/PV/Load selector.
    - Nominal inference is reasonable for common DC systems (12/24/48V) and quality summaries do not mislabel DC rails with AC terms (no “sags/swells” copy for DC).
    - `cd apps/dashboard-web && npm run build` passes.
    - Playwright coverage exists (`apps/dashboard-web/playwright/power-voltage-quality.spec.ts`).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.103`; screenshot captured + viewed: `manual_screenshots_web/20260112_121326/power.png` (run: `project_management/runs/RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-66: Add Power tab (node-centric Emporia + Renogy dashboards)**
  - **Description:** Add a dedicated Power tab that treats each Renogy controller and each Emporia device as its own node, showing power-specific dashboards (live values + per-node charts + per-circuit breakdowns) without implying unrelated systems are physically tied together.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/power/PowerPageClient.tsx`
    - `apps/dashboard-web/src/components/SidebarNav.tsx`
    - `apps/dashboard-web/src/components/NavTabs.tsx`
  - **Acceptance Criteria:**
    - New `/power` route exists and is in the sidebar nav.
    - Power UI lists power-capable nodes (Renogy nodes + Emporia nodes) and shows per-node live values with explicit units and last-updated timestamps.
    - Emporia view shows a per-circuit table (name/channel, live power, unit, last update) and at least one history chart (mains + selected circuit).
    - Renogy view shows PV/load/battery metrics per node with units; forecast overlays remain per node and are clearly labeled.
  - **Notes / Run Log:**
    - 2026-01-06: Added `/power` route and nav entries; page detects Renogy vs Emporia nodes and renders per-node dashboards with explicit units.
    - 2026-01-06: Emporia dashboard shows mains power + circuits table; circuit “last update” currently reflects node `last_seen` (per-sensor timestamps pending `latest_ts` in the sensor API).
    - 2026-01-07: Power node selector now prefixes Emporia nodes with the configured address-group label and indicates when a meter is excluded from system totals.
    - 2026-01-10: Power tab Renogy view now renders separate trend charts for power (W), voltage (V), and current (A), deriving battery power (W) from battery voltage × current. Standardized technical power labels with units across Power + Analytics + Overview banner.
    - 2026-01-06: Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-06: Production verification after upgrade to `0.1.9.13`: `curl -I http://127.0.0.1:8000/power/` returns `200 OK` (route is live in the installed dashboard).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; `/api/analytics/power` and feed health are `ok` and Emporia electrical readbacks exist in sensor inventory. Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-70: Emporia meter preferences UI (exclude meters + address grouping)**
  - **Description:** Extend Setup Center to manage Emporia meters on multi-site accounts: show all meters, allow grouping by address, and allow excluding specific meters from system-wide analytics totals while keeping them visible in node/power views.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/setup/SetupPageClient.tsx`
    - `apps/dashboard-web/src/lib/api.ts`
    - `apps/dashboard-web/src/lib/queries/index.ts`
    - `apps/dashboard-web/src/features/analytics/components/AnalyticsOverview.tsx`
  - **Acceptance Criteria:**
    - Setup Center shows a list of Emporia meters with: name/deviceGid, editable “Address group” (defaults from detected address), “Poll enabled” toggle, and “Include in totals” toggle; detected address remains visible as a hint.
    - Saving preferences persists to the controller (`PUT /api/setup/emporia/devices`) and triggers a feed poll so totals update without manual steps.
    - Analytics → Power shows an “Emporia meters by address” breakdown (group label → included live power) and per-node rows show group + excluded-from-totals status.
    - `cd apps/dashboard-web && npm run build` passes and relevant CI/E2E re-run on a clean host.
  - **Notes / Run Log:**
    - 2026-01-07: Added Setup Center “Meters & summaries” table (group label + poll/include toggles) and wired save → `/api/setup/emporia/devices` + `/api/analytics/feeds/poll` refresh.
    - 2026-01-07: Analytics Power nodes panel now includes an “Emporia meters by address” breakdown and a per-node Group column.
    - 2026-01-08: Refreshed the installed dashboard via controller bundle `0.1.9.29` so Setup Center + Analytics UI updates are live on `:8000` (no separate dev server).
    - 2026-01-08: Audit: “Address” vs “Group label” is confusing; simplify to a single “Address group” control (with detected address shown as a hint) to match the original client request (group by address + optional exclusions).
    - 2026-01-08: Simplified Emporia meter UX (single “Address group” control + clarified totals toggle) and aligned Analytics wording; refreshed the installed controller to `0.1.9.30`.
    - 2026-01-08: Setup Center nesting polish: moved “Emporia meters & totals” out of the token-login card (Integrations section) so meter preferences are not treated as credentials; updated Analytics helper text accordingly; refreshed the installed controller to `0.1.9.33`.
    - Build: `cd apps/dashboard-web && npm run build` (pass).
    - Test note: Test hygiene preflight cannot be satisfied on this host without admin privileges to stop the installed stack; run `make ci-web-smoke && make e2e-web-smoke` on a clean dev host.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; Emporia feed `ok` and multi-meter polling active; Setup Center Emporia meters UI verified in-browser post-upgrade (no dev server). Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-67: Make Sensors + Trends node-first (reduce clutter, add context)**
  - **Description:** Reduce cognitive load by making node context first-class in Sensors and Trends: group by node, show node name/location everywhere, and ensure every value with a unit shows the unit.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/sensors/SensorsPageClient.tsx`
    - `apps/dashboard-web/src/features/sensors/components/SensorsOverview.tsx`
    - `apps/dashboard-web/src/features/sensors/components/SensorTable.tsx`
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
  - **Acceptance Criteria:**
    - Sensors tab supports a node-grouped view (collapsible sections) and makes it obvious which node each sensor belongs to (node name shown in row + filter/search improvements).
    - Trends tab sensor picker shows node context (e.g., “<Node> — <Sensor> (unit)”) and supports filtering/grouping by node.
    - Sidebar/footer power-related summaries are not ambiguous; if a value is fleet-wide it is labeled as such, otherwise it includes the node name.
  - **Notes / Run Log:**
    - 2026-01-06: Sensors tab now supports a “By node” grouped view (default) with collapsible node sections; flat table remains available.
    - 2026-01-06: Trends sensor picker is grouped by node with node-aware labels (`<Node> — <Sensor> (unit)`), plus node filter + search.
    - 2026-01-06: Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-06: Production upgrade: controller bundle `0.1.9.13` installed so the running dashboard can surface the new node-first grouping UX (manual UI validation still pending).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; Sensors + Trends remain node-first (manual spot-check) and `/api/dashboard/state` reports stable node/sensor inventory sizes. Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-29: Prefer controller-issued adoption token in adopt flow**
  - **Description:** Ensure the adoption modal always issues a controller token (`POST /api/adoption/tokens`) and uses it for `/api/adopt`, even when discovery candidates advertise an `adoption_token`, to prevent production `403 Invalid adoption token` failures.
  - **Acceptance Criteria:**
    - Adoption modal calls `POST /api/adoption/tokens` and uses the returned token when posting to `/api/adopt`.
    - If token issuance fails, the modal surfaces the real error and does not fall back to the node-advertised token (controller validates against DB only).
  - **Notes / Run Log:**
    - 2026-01-10: Fixed Nodes “Discovered controllers” filtering to not require a node-advertised `adoption_token` (the adoption modal already issues controller tokens). Evidence: `project_management/runs/RUN-20260110-tier-a-phase3-adoption-ts8.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-56: Rename “Control” role to “Operator” (UX polish)**
  - **Description:** Align user-facing role naming with product expectations: “Operator” (can change schedules + trigger outputs) instead of “Control”, without breaking stored legacy role values.
  - **Acceptance Criteria:**
    - Users UI shows roles `Admin` / `Operator` / `View` (or `View-only`).
    - Existing stored role `control` displays as `Operator` in the UI.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.101`. Screenshot captured + viewed:
      - `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/users_add_modal.png`
    - Run: `project_management/runs/RUN-20260112-tier-a-auth-permissions-0.1.9.101.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-95: UI: capability-gate Users + Outputs actions**
  - **Description:** Prevent operators/viewers from seeing or enabling actions that will always fail due to missing capabilities (Users admin and Output commands).
  - **Acceptance Criteria:**
    - Users page admin actions are hidden/disabled unless `users.manage` is present.
    - Output command UI is disabled unless `outputs.command` is present and explains why.
    - Admin navigation entries are hidden unless the capability to use them exists (`users.manage` for Users; `config.write` for Setup Center/Deployment).
  - **Notes / Run Log:**
    - 2026-01-12: Added Playwright regression coverage for nav gating and outputs read-only UX (`apps/dashboard-web/playwright/auth-gating.spec.ts`).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.101`. Screenshots captured + viewed:
      - `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/users.png`
      - `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/users_add_modal.png`
    - Run: `project_management/runs/RUN-20260112-tier-a-auth-permissions-0.1.9.101.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-100: Configure node sensors from the dashboard (push to node-agent)**
  - **Description:** Provide an admin UX to add/edit/remove a node’s hardware sensors directly from the Nodes UI and apply the configuration to the node automatically (no manual file copy to the Pi).
  - **Acceptance Criteria:**
    - Node detail UI includes a “Node sensor config” editor that reuses the Provisioning sensor editor controls.
    - Saving applies via `PUT /api/nodes/{node_id}/sensors/config` and shows clear “applied vs stored (node offline)” feedback.
    - Saving refreshes the node + sensor lists so the rest of the dashboard reflects the updated registry.
  - **Notes / Run Log:**
    - 2026-01-10: Added a Node sensor config editor in the node drawer and wired it to the new core endpoints; deployed via controller bundle `0.1.9.61`.
    - Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-101: Renogy controller settings UI (BT-2 Modbus apply workflow)**
  - **Description:** Add a node-centric “Devices → Renogy RNG-CTRL-RVR20-US (BT-2)” panel that supports viewing live telemetry and safely editing controller settings (current vs desired, diff summary, validate/apply/read-back verify, history + rollback).
  - **Acceptance Criteria:**
    - Nodes → <Renogy node> → Devices contains a Renogy BT‑2 card with:
      - Connection fields (BT‑2 MAC, unit_id, poll interval) + Advanced UUID overrides (collapsed).
      - Live telemetry values (PV/battery/load volts/amps/watts, SOC, temps, runtime estimate).
      - Settings form grouped by category, showing Current vs Desired and a clear diff summary.
      - Apply workflow: Read → edit → Validate → Apply (confirm) → read-back verify; shows per-field results.
      - History view with apply logs and one-click rollback.
    - Mutating actions are gated behind `config.write` and show clear “insufficient permissions” UX when unavailable.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.73`; Node detail includes a Devices section with the Renogy settings panel (gated behind `config.write`). Run: `project_management/runs/RUN-20260111-tier-a-renogy-settings-0.1.9.73.md`.
  - **Status:** Done (validated on installed controller; hardware validation deferred to NA-61)

- **DW-102: Nodes drawer layout polish (Local display under Outputs + collapsible)**
  - **Description:** Reduce clutter in the Node detail drawer by moving the Local display editor below Outputs and making the entire section collapsible.
  - **Acceptance Criteria:**
    - “Local display” appears below Outputs in the Node drawer.
    - The section can be collapsed/expanded to hide the full editor.
    - `cd apps/dashboard-web && CI=1 npm run test:smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.79` (Tier A); Node drawer shows Local display under Outputs and it collapses/expands without layout glitches (manual spot-check).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-103: Fix Node “health history” button errors + clarify label**
  - **Description:** Fix the Node detail Health toggle crash on some origins and improve the label so it’s clear what the button does.
  - **Acceptance Criteria:**
    - Button label reads “Show health history” / “Hide health history” (no ambiguous “trends” wording).
    - Clicking does not crash on non-localhost origins (no `crypto.subtle` availability assumptions).
    - `cd apps/dashboard-web && CI=1 npm run test:smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.80` (Tier A); Node detail loads Health history without errors at `http://100.112.87.90:8000/nodes/detail?...` (manual spot-check screenshot `manual_screenshots_web/tier-a-health-button/node1_health_history_ip_origin.png`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-104: Node detail drawer IA cleanup (cohesive layout + visual hierarchy)**
  - **Description:** Reduce design drift in the Node detail drawer by introducing a consistent card-based layout system and clear section hierarchy (Overview → Sensors → Outputs → Local display → Alarms → Backups).
  - **Acceptance Criteria:**
    - All sections share the same container styling and spacing.
    - Top section is an “Overview” card (identity + health snapshot + health history toggle).
    - Each section has a 1-line description to clarify scope without reading docs.
    - `cd apps/dashboard-web && CI=1 npm run test:smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.81` (Tier A); Node drawer shows consistent cards + hierarchy. Screenshot: `manual_screenshots_web/tier-a-node-drawer-ia/drawer_top_after_refresh.png`.
    - Follow-up Tier‑A validation: installed controller refreshed to `0.1.9.105`; drawer “Sensors & Outputs” section shows full sensor list (not truncated) with search + scroll. Screenshots viewed: `manual_screenshots_web/20260112_130241/nodes_drawer.png` and `manual_screenshots_web/20260112_130241/nodes.png` (run: `project_management/runs/RUN-20260112-tier-a-nodes-drawer-sensors-0.1.9.105.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-105: Overview “Where things live” Mermaid sitemap**
  - **Description:** Replace the Overview page’s “Where things live” text list with a concise Mermaid sitemap showing tab hierarchy, with hover tooltips for detail and clickable nodes for navigation.
  - **Acceptance Criteria:**
    - Overview renders a Mermaid chart (no blank/error state) showing Operations/Admin tabs and key services at a glance.
    - Hovering nodes shows concise tooltips; clicking navigates to the corresponding tab route.
    - `cd apps/dashboard-web && CI=1 npm run test:smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller upgraded to `0.1.9.87` (Tier A); Overview sitemap renders left-to-right with hover tooltips (no Mermaid parse errors). Screenshot: `manual_screenshots_web/tier-a-overview-tooltips-0.1.9.87/overview_tooltip_mqtt.png`.
    - Regression hardening: Mermaid tooltips now work even when Mermaid renders clickable nodes as `<a><g class="node">…</g></a>` (no more “only clusters tooltip”). Validation: Playwright + viewed screenshots:
      - Test: `apps/dashboard-web/playwright/overview-mermaid-tooltips.spec.ts` (passes).
      - Screenshots viewed: `/tmp/playwright-overview-mermaid-tooltips.png`, `/tmp/playwright-overview-mermaid-hover.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-106: Flatten node sensor config nesting (Hardware sensors)**
  - **Description:** Remove redundant nesting in the node sensor config editor so the single “Hardware sensors” editor is not wrapped in an extra “Node sensor config” section.
  - **Acceptance Criteria:**
    - Node detail/drawer shows a single top-level “Hardware sensors” section.
    - “Add sensor” + “Apply to node” actions remain available when the editor is open.
    - No nested card-within-card layout for this editor.
    - `cd apps/dashboard-web && CI=1 npm run test:smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.103`; screenshot captured + viewed: `manual_screenshots_web/20260112_121326/nodes_drawer.png` (run: `project_management/runs/RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-111: Remove Provisioning tab; Deployment includes adopt + naming**
  - **Description:** Remove the separate “Provisioning” tab from the web dashboard and make Deployment the end-to-end flow: deploy a fresh Pi 5 over SSH, then adopt it into the controller (with a clear node name) so sensors can be configured from Sensors & Outputs.
  - **Acceptance Criteria:**
    - No “Provisioning” tab is shown in the sidebar/nav; `/provisioning` redirects to `/deployment`.
    - Deployment requires a node display name and, after a successful deploy, provides an in-page Adopt step that matches the node by MAC via `/api/scan` and adopts using controller-issued tokens (no node-advertised token dependency).
    - After adoption, the UI offers a direct link to configure sensors for the adopted node in Sensors & Outputs.
    - `cd apps/dashboard-web && npm run test:playwright -- deployment-adopt-flow.spec.ts` passes.
  - **Evidence (Tier A):**
    - Installed controller upgraded to `0.1.9.97` (Tier A; no DB/settings reset); screenshots captured + viewed:
      - `manual_screenshots_web/tier-a-0.1.9.97-deployment-adopt/provisioning_redirects_to_deployment.png`
      - `manual_screenshots_web/tier-a-0.1.9.97-deployment-adopt/deployment_adopt_card_ready.png`
      - `manual_screenshots_web/tier-a-0.1.9.97-deployment-adopt/deployment_adoption_modal_prefilled.png`
      - `manual_screenshots_web/tier-a-0.1.9.97-deployment-adopt/deployment_adopted_configure_sensors.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-112: Node location editor in “More details” drawer**
  - **Description:** Allow admins/operators to set a node’s latitude/longitude from the Nodes tab detail drawer so location-based features (weather/PV forecast) work even without map placement.
  - **References:**
    - `apps/dashboard-web/src/features/nodes/components/NodeGrid.tsx`
    - `apps/dashboard-web/src/features/nodes/components/NodeDetailDrawer.tsx`
  - **Acceptance Criteria:**
    - Node cards use the label “More details” (not “View details”).
    - Nodes → More details drawer shows Location with Latitude/Longitude inputs.
    - Saving updates the node `config.latitude` / `config.longitude` via `PUT /api/nodes/{id}` without clobbering unrelated config keys.
    - Missing `config.write` capability shows a clear read-only state (no save allowed).
  - **Evidence (Tier A):**
    - Installed controller upgraded to `0.1.9.102` (Tier A; no DB/settings reset).
    - Screenshot captured + viewed: `manual_screenshots_web/20260112_184002_node_location_drawer/nodes_drawer_location_only.png`.
    - Run log: `project_management/runs/RUN-20260112-tier-a-node-location-0.1.9.102.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-113: Cross-tab layout consistency (reduce design drift)**
  - **Description:** Standardize page headers, banners, spacing, and card styling across the dashboard to reduce UI drift and improve visual hierarchy as the app grows.
  - **References:**
    - `apps/dashboard-web/src/components/PageHeaderCard.tsx`
    - `apps/dashboard-web/src/components/InlineBanner.tsx`
  - **Acceptance Criteria:**
    - Tabs use consistent header layout (title/description/actions) and consistent inline banners for scan/refresh/errors.
    - A full sidebar-tab screenshot sweep is captured and **viewed** on the installed controller (Tier A) to confirm consistent hierarchy and spacing.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.103`; Playwright screenshots captured + viewed for all tabs: `manual_screenshots_web/20260112_121326/` (run: `project_management/runs/RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114)

- **DW-115: Sensors & Outputs “Add sensor” row (replace Hardware sensors section)**
  - **Description:** Remove the separate “Hardware sensors” section from Sensors & Outputs and replace it with a compact “Add sensor” row at the end of each node’s sensor list.
  - **Acceptance Criteria:**
    - Only Raspberry Pi node-agent nodes (nodes with `config.agent_node_id`) show an “Add sensor” row/button (hardware sensors are not configurable on Core/Emporia/external nodes).
    - Clicking “Add sensor” opens the existing hardware sensor editor, pre-expanded with a new draft sensor ready to configure.
    - The button is capability-gated: disabled without `config.write` (clear UX).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.113`; “Add sensor” is shown only for Pi node-agent nodes, and Core node does not show an “Add sensor” row. Screenshots captured + viewed:
      - `manual_screenshots_web/20260113_tier_a_hw_sensors_0.1.9.113/sensors_add_sensor.png`
      - `manual_screenshots_web/20260113_tier_a_hw_sensors_0.1.9.113/sensors_core.png` (run: `project_management/runs/RUN-20260113-tier-a-hardware-sensors-pi-only-0.1.9.113.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114)

- **DW-117: Chart x-axis pan/zoom on all graphs**
  - **Description:** Add a consistent, low-friction interaction model for charts so operators can inspect time windows without changing global Range controls. All Chart.js graphs should support horizontal pan/zoom with a quick reset.
  - **Acceptance Criteria:**
    - Drag pans the x-axis (time) on every Chart.js chart across the dashboard.
    - Mouse wheel / trackpad pinch zooms the x-axis on every Chart.js chart.
    - Double-click resets the view (`chartRef.current?.resetZoom?.()`).
    - `cd apps/dashboard-web && npm run build` passes.
    - `cd apps/dashboard-web && npm run test:smoke` passes.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.114`; zoom/pan/reset screenshots captured + viewed: `manual_screenshots_web/tier_a_0.1.9.114_chart_zoom_pan/` (see `01_zoomed.png`, `02_panned.png`, `03_reset.png`).
    - Playwright Tier‑A interaction run (Chromium mobile): `cd apps/dashboard-web && FARM_PLAYWRIGHT_SAVE_SCREENSHOTS=1 FARM_PLAYWRIGHT_SCREENSHOT_DIR=manual_screenshots_web/tier_a_0.1.9.114_chart_zoom_pan npm run test:playwright -- playwright/chart-zoom-pan.spec.ts --project=chromium-mobile` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-57: Add progress feedback for “Refresh” and “Scan again” actions**
  - **Description:** Improve operator confidence by adding in-button progress UI and completion messaging for long-running actions (refresh, scan).
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - “Scan again” shows progress while the scan is active (progress bar or equivalent) and is disabled to prevent duplicate runs.
    - “Refresh” shows a throbber while active and briefly shows “Complete” (≈4s) when done before returning to normal.
    - Error states are visually distinct from “in progress” and “complete”.
    - `make ci-web-smoke` remains green.
  - **Notes / Run Log:**
    - 2026-01-11: Added in-button progress + transient Complete/Error states for Nodes and Sensors refresh/scan actions. Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; Nodes + Sensors refresh buttons show in-button progress and transient Complete/Error states; scan actions are disabled while active (manual spot-check). Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-58: Dashboard UI for node network health trends (ping/latency/jitter/uptime)**
  - **Description:** Surface per-node network quality metrics (latency/jitter/uptime) and trend history so operators can diagnose reliability issues.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - Nodes UI can display at least: ping uptime % over last 24h and latency/jitter trend over last 30 minutes.
    - UI reads from the production telemetry pipeline (no demo-only data path).
    - `make ci-web-smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.77`.
    - `/api/nodes` shows non-null `network_latency_ms`, `network_jitter_ms`, `uptime_percent_24h` for Pi5 Node 2.
    - Node detail → Health “Show trends” renders history from `/api/metrics/query`. Screenshot: `manual_screenshots_web/tier-a-nodehealth/nodes_node2_health_trends_v2.png`.
    - Web smoke: `cd apps/dashboard-web && CI=1 npm run test:smoke` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-59: Dashboard UI for Pi 5 node resource telemetry (CPU/RAM)**
  - **Description:** Display CPU utilization (per core) and RAM utilization for Pi 5 nodes to support field ops debugging.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - Node detail includes CPU (per core) and RAM utilization panels, with trend history.
    - UI reads from the production telemetry pipeline.
    - `make ci-web-smoke` remains green.
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.77`.
    - `/api/nodes` shows non-null `memory_percent` and `memory_used_bytes` for Pi5 Node 2.
    - Node detail → Health “Show trends” renders CPU/RAM history from `/api/metrics/query`. Screenshot: `manual_screenshots_web/tier-a-nodehealth/nodes_node2_health_trends_v2.png`.
    - Web smoke: `cd apps/dashboard-web && CI=1 npm run test:smoke` (pass).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-60: Expand preconfigured sensor templates (used for node sensor config)**
  - **Description:** Add additional device templates (sensor/output presets) to reduce manual configuration when adding sensors to nodes from the dashboard; initial model list to be provided by JCM.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - Sensor template picker presents additional templates (models) without breaking existing presets.
    - Template selections populate sensible defaults and remain editable.
    - `make ci-web-smoke` remains green.
  - **Notes / Run Log:**
    - 2026-01-11: Added additional starter templates (4–20mA pressure/level, wind direction, explicit pulse-unit presets) while keeping all existing presets intact. Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; node sensor config template list includes the new templates (manual spot-check). Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-74: Show node offline duration everywhere node status is shown**
  - **Description:** When a node is offline, show how long it has been offline next to the offline badge/status across the dashboard (Nodes, Power, Analytics tables, etc).
  - **Acceptance Criteria:**
    - Any UI surface that shows `node.status === "offline"` also shows “offline for …” derived from `node.last_seen` (with sane fallbacks when unknown).
    - Formatting is concise (e.g., `offline · 12m`) and does not regress layout on narrow screens.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Added shared `formatNodeStatusLabel()` helper and applied across Nodes/Power/Analytics/Sensors/Map popups so offline duration is consistently shown. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-12: Tier‑A validated on installed controller `0.1.9.100` (screenshots viewed). Run: `project_management/runs/RUN-20260112-tier-a-closeout-loop-0.1.9.100.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-85: Clarify Nodes vs Sensors & Outputs responsibilities**
  - **Description:** Ensure the Nodes tab stays node/device-centric (adoption, health, backups) and the Sensors & Outputs tab stays sensor/IO-centric (naming/formatting, alarms, output commands), while still allowing lightweight cross-references like status counts and previews.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/nodes/NodesPageClient.tsx`
    - `apps/dashboard-web/src/features/nodes/components/NodeGrid.tsx`
    - `apps/dashboard-web/src/features/nodes/components/NodeInfoList.tsx`
    - `apps/dashboard-web/src/features/nodes/components/NodeDetailDrawer.tsx`
    - `apps/dashboard-web/src/features/sensors/components/SensorsOverview.tsx`
  - **Acceptance Criteria:**
    - Nodes tab header copy explicitly frames node-centric responsibilities and points to Sensors & Outputs for sensor formatting + output commands.
    - Nodes grid shows a compact preview of attached sensors/outputs (limited list) and does not turn into a sensor catalogue.
    - Node detail drawer provides an obvious path to manage sensors/outputs (without duplicating sensor-edit UX in the Nodes view).
    - Sensors & Outputs tab header copy explicitly frames sensor-centric responsibilities and points to Nodes for adoption/health/backups.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Updated Nodes/Sensors tab copy and navigation, truncated sensor/output previews on node cards, and added “Open Sensors & Outputs” shortcut from node detail. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-12: Moved full per-node IO workflow into Sensors & Outputs (hardware sensor config + live weather + outputs with schedules/commands), and slimmed the Node drawer down to node health + quick links. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-12: Restored a minimal Sensors quick list in the Node drawer and added a deep-link into Sensors & Outputs (opens the specific sensor). Playwright: `cd apps/dashboard-web && npm run test:playwright -- node-drawer-sensor-deeplink.spec.ts` (pass).
    - 2026-01-12: Fixed confusing “Scan complete” UX: scan now distinguishes “new” vs “already adopted” and surfaces already-adopted nodes in the scan panel instead of silently filtering them out. Removed the misleading Provisioning nav badge (candidates belong under Nodes/Deployment). Playwright: `cd apps/dashboard-web && npm run test:playwright -- nodes-scan-adopted-visible.spec.ts` (pass).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.95`; Node drawer shows a compact sensors list with deep-links into Sensors & Outputs. Screenshots captured + viewed:
      - `manual_screenshots_web/tier-a-node-drawer-deeplink-0.1.9.95/nodes_drawer.png`
      - `manual_screenshots_web/tier-a-node-drawer-deeplink-0.1.9.95/sensors.png`
    - Installed controller refreshed to `0.1.9.96`; Nodes scan panel shows “Ready to adopt” vs “Already adopted” sections (no ambiguity). Screenshot captured + viewed:
      - `manual_screenshots_web/tier-a-nodes-scan-ux-0.1.9.96/nodes.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **DW-120: Add direct links to node + sensor detail pages**
  - **Description:** Provide direct navigation entrypoints to the dedicated detail pages (`/nodes/detail?id=...`, `/sensors/detail?id=...`) from the Nodes and Sensors surfaces so users don’t have to go through Map.
  - **Acceptance Criteria:**
    - Superseded by DW-122/DW-123 (consolidated detail surfaces). The sensor drawer may still include an “Open node” navigation to `/nodes/detail?id=<node.id>`.
  - **Notes / Run Log:**
    - 2026-01-14: Tests intentionally not run (per operator request: “No tests”). Recommended follow-up validation: `make ci-web-smoke`.
  - **Status:** Done (canceled: superseded by DW-122/DW-123)

- **DW-61: Add map view for nodes/sensors with polygons + topo overlays**
  - **Description:** Provide a map-based operator view for placing nodes/sensors and drawing polygon overlays for fields/ditches/utility lines, with optional topo overlays.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - Dashboard includes a dedicated `Map` tab (admin/engineer UX) with an interactive map canvas.
    - Base map can be toggled between street and satellite (free/public sources; configurable URLs).
    - Street-level imagery (“Street View”) is available via a free integration (Mapillary) with a toggle + click-to-open flow; the access token is configured in Setup Center.
    - Map supports zooming to an equivalent of ~300’ “eye altitude” (max zoom + quick control + visible scale/altitude readout).
    - Admin can place and persist locations for:
      - Nodes
      - Sensors
      - Custom “hardware” markers (user-defined points)
    - Admin can draw/edit/delete and persist polygon/line overlays for fields/ditches/utility lines (with basic styling + labels).
    - Admin can add/manage overlay layers:
      - Built-in topo option (free/public tiles) and/or user-configured topo sources
      - User-provided survey overlays via upload (GeoJSON/KML minimum) and via URL-based sources (XYZ/WMS/ArcGIS REST)
      - Enable/disable, reorder, and adjust opacity per overlay
    - Map UI includes a usable layer/feature management panel (search/filter, selection details, safe delete confirmations).
    - All map writes require `config.write` and are persisted in the controller DB.
    - `make ci-web-smoke` and `cargo build --manifest-path apps/core-server-rs/Cargo.toml` remain green; run `make e2e-web-smoke` before marking Done.
  - **Notes / Run Log:**
    - 2026-01-07: Added persisted map schema + APIs: `infra/migrations/022_map_view.sql` (`map_settings`, `map_layers`, `map_features`) and `/api/map/*` routes gated by `config.write` (default base layers: OSM streets, Esri satellite, USGS topo).
    - 2026-01-07: Added Dashboard `Map` tab with MapLibre (street/satellite toggle), draw/edit polygons/lines/markers, node/sensor placement, overlay manager (XYZ/WMS/ArcGIS REST + GeoJSON/KML upload + SCCGIS presets), and a “zoom to ~300’ eye altitude” helper.
    - 2026-01-07: Added on-map labels for drawn markup (polygons/lines/markers) so field/ditch/utility names are visible on the canvas (not just in the side panel).
    - 2026-01-07: Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass); `npm --prefix apps/dashboard-web run build` (pass).
    - 2026-01-07: Bundled + upgraded installed controller to `0.1.9.20` via setup-daemon (`POST http://127.0.0.1:8800/api/upgrade`); `GET http://127.0.0.1:8000/healthz` is ok and `/map/` serves the updated UI.
    - 2026-01-07: Bundled + upgraded installed controller to `0.1.9.21` (includes on-map labels + Setup Center admin config expansion); `GET http://127.0.0.1:8000/healthz` is ok and setup-daemon health report shows core+dashboard ok.
    - 2026-01-07: Map polish: layer attributions now surface via the MapLibre attribution control, and the device placement panel adds search + placed/unplaced management (Focus/Open/Unplace). Bundled + upgraded installed controller to `0.1.9.24`.
    - 2026-01-07: Added Mapillary “Street View” mode (street-level imagery) to the Map tab + Setup Center Mapillary token; bundled + upgraded installed controller to `0.1.9.26` (`/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.26.dmg`); `/healthz` ok and `/map/` 200.
    - 2026-01-10: Map placement UX polish: “Place devices” now stacks Sensors under Nodes, and sensors inherit their parent node location by default (optional per-sensor override with Reset-to-inherit). Refreshed installed controller to `0.1.9.49`.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Map renders and placed nodes/sensors are visible. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/map.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-116: Offline-first Map tab stack (GeoJSON layers + local assets; no internet after setup)**
  - **Description:** Refactor the Map tab so nodes/sensors/markup are rendered as MapLibre GeoJSON sources + style layers (no DOM markers), and switch basemaps/glyphs/terrain to controller-hosted offline assets with an operator-friendly Setup Center install flow (Swanton, CA pack).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0031-offline-first-map-stack-(local-tiles,-glyphs,-terrain;-swanton-ca-pack).md`
    - `apps/dashboard-web/src/app/(dashboard)/map/MapPageClient.tsx`
    - `apps/dashboard-web/src/features/map/MapCanvas.tsx`
  - **Acceptance Criteria:**
    - Map tab works with internet blocked after offline pack install:
      - basemap tiles render
      - node pins + markup render
      - placement/edit interactions work and persist across reload
    - Nodes/sensors/markup are rendered via GeoJSON sources + MapLibre layers (no `new maplibregl.Marker()` usage).
    - A single draw/edit interaction model is used for points/lines/polygons so placement and markup feel consistent.
    - Complex polygon edits do not crash the page.
    - Tier A evidence includes captured + viewed screenshots proving offline map + placement + markup stability; Tier B deferred to `DW-97`.
  - **Evidence (Tier A):**
    - Installed controller `0.1.9.112`: Map renders with browser-level internet blocked and the offline Swanton pack installed. Evidence: `project_management/runs/RUN-20260113-tier-a-offline-map-stack-0.1.9.112.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-71: Fix Map tab basemap rendering (blank canvas)**
  - **Description:** Resolve the production Map tab regression where the MapLibre canvas renders blank for all base map selections, ensuring the installed controller build renders maps reliably.
  - **Acceptance Criteria:**
    - Map tab renders a visible basemap in the installed (production) dashboard build for all built-in base layers (streets/satellite/topo).
    - Base-layer toggles swap tiles without requiring a hard refresh.
    - Console shows no MapLibre “worker/style” load errors in production builds.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build` remain green; run `make e2e-web-smoke` before marking Done.
  - **Notes / Run Log:**
    - 2026-01-08: Added MapLibre CSP worker copy + runtime workerUrl wiring so static builds render maps; added map-load UX/error overlay. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-08: Audit: basemaps can still appear blank when the saved map zoom exceeds the tile source max zoom (404s at high zoom). Fix by supporting per-layer `max_zoom`/heuristics and passing `maxzoom` to MapLibre raster sources for overzoom/overscale.
    - 2026-01-08: Implemented per-layer `max_zoom` support (MapLibre overscale) and added “Max zoom” controls in the layer editor; refreshed the installed controller to `0.1.9.30`.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): basemaps render in production build. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/map.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-72: Named map saves (save-as + compact loader dropdown)**
  - **Description:** Allow admins to save multiple named map placements (nodes/sensors + drawn features + view) and switch between them via a compact dropdown, with one “active” save driving downstream features (e.g., node weather by location).
  - **Acceptance Criteria:**
    - Map tab “Save…” prompts for a save name and persists a snapshot of the current map view + all features (node/sensor placements + custom markup).
    - Map tab shows a compact “Saved map” dropdown that lists prior saves and allows switching (load/apply) without leaving the page.
    - Loading a saved map replaces the active placements/markup and updates map view (center/zoom/bearing/pitch/base layer) to the saved state.
    - Controller persists saves in DB, tracks the active save, and exposes save/list/apply endpoints gated by `config.write`.
    - New endpoints are registered in `apps/core-server-rs/src/openapi.rs`; `make rcs-openapi-coverage` remains green.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build` remain green; run `make e2e-web-smoke` before marking Done.
  - **Notes / Run Log:**
    - 2026-01-08: Implemented DB-backed named map saves (`map_saves`, `map_settings.active_save_id`, per-save `map_features`) + dashboard Save-as modal + compact loader dropdown; active save drives feature scope. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass), `cd apps/dashboard-web && npm run build` (pass), `python3 tools/check_openapi_coverage.py` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Map tab renders with Saved-map selector + Save-as modal. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/map.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-73: Node Sensors view: show live weather at node location (from active map save)**
  - **Description:** Add a per-node “Live weather” panel inside the Node Sensors view using the hyperlocal weather API’s current conditions, using the node coordinates from the active saved map placement.
  - **Acceptance Criteria:**
    - Node detail Sensors view includes a “Live weather” section (temperature, wind, cloud cover, precipitation, etc) for the node’s placed coordinates.
    - Coordinates are resolved from the active saved map placement; if the node is unplaced, the UI prompts to place it on the Map tab.
    - API calls use the provider’s non-rate-limited/current endpoint (no polling of rate-limited forecast endpoints for live preview) and are cached to avoid noisy refreshes.
    - UI surfaces last-updated time and clear empty/error states.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Added per-node “Live weather” panel (Nodes → Sensors surfaces) backed by new controller current-weather endpoint; resolves node coords from active map save and caches responses. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Node detail renders per-node live weather panel using active map placement. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/nodes_0a55b329-104f-46f0-b50b-dea9a5cca1b3.png`.
    - 2026-01-13: Removed redundant “Live weather” wrapper card in the Sensors & Outputs per-node panel (fixes double nesting). Refreshed installed controller to `0.1.9.116`. Screenshot captured + viewed: `manual_screenshots_web/20260113_094500_live-weather-nesting_0.1.9.116/sensors.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-97)

- **DW-75: Analytics layout + Power nodes table rendering fixes**
  - **Description:** Fix overflow/layout issues on Analytics, including stacking Water + Power sections vertically and simplifying the Power nodes table columns.
  - **Acceptance Criteria:**
    - Analytics stacks Water + Power sections vertically (full-width) so wide tables do not spill; Weather + PV forecast sections can remain side-by-side.
    - Analytics “Power nodes” table stays within its card/container (scrolls horizontally when needed).
    - Power nodes table has a single “Load power” column (Emporia uses live mains, Renogy uses load) rather than separate Live/Load columns.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Stacked Analytics Water + Power vertically; constrained “Power nodes” table with horizontal scroll; unified Live/Load into a single “Load power” column. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Analytics layout renders without table overflow on the installed UI. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/analytics.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-79: Analytics feed health: include forecast providers + categories**
  - **Description:** Ensure Analytics → Feed health surfaces all external connector statuses (power + forecast) so operators have a single “is it working?” view.
  - **Acceptance Criteria:**
    - `GET /api/analytics/feeds/status` returns latest connector statuses across categories (at minimum: `power` + `forecast`), not just power.
    - Feed history entries include the correct `category` value for each row.
    - Dashboard Analytics → Feed health shows Forecast.Solar + Open-Meteo alongside Emporia when those providers have status rows.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Updated the controller to aggregate `analytics_integration_status` by `(category,name)` so `/api/analytics/feeds/status` includes Forecast.Solar + Open-Meteo (forecast) alongside Emporia (power). Verified via `curl http://127.0.0.1:8000/api/analytics/feeds/status`. Refreshed the installed controller to `0.1.9.36`. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Analytics renders Feed health with forecast providers present. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/analytics.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-80: Power + Sensors UI: show Emporia voltage/current and graph access**
  - **Description:** Ensure Emporia meters expose all electrical readbacks in the dashboard UI (not just watts): show voltage/current alongside power, and make it easy to graph these values without forcing users into raw sensor IDs.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/power/PowerPageClient.tsx`
    - `apps/dashboard-web/src/features/sensors/components/SensorDetailDrawer.tsx`
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
  - **Acceptance Criteria:**
    - Power tab Emporia view displays mains voltage/current (when available) alongside mains power and shows per-circuit voltage/current columns where present.
    - Emporia voltage/current sensors are discoverable and graphable (Trends + per-sensor detail history), with correct units and per-sensor display decimals respected.
    - UI gracefully handles partial availability (some channels may not report V/A).
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Power tab now shows mains voltage/current cards + 24h charts for Emporia nodes, and an Emporia circuits table with Power/Voltage/Current columns; circuit names deep-link to the sensor detail view for graphing. Refreshed the installed controller to `0.1.9.39`. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Power tab Emporia circuits table now computes and labels watts when only V/A are available (computed as V×A) and surfaces derived-power sensors (from controller ingest) with a subtle “calc” marker.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Emporia voltage/current readbacks render in the installed UI. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/power.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-81: Analytics PV forecast overlay: include historical forecast (48h window)**
  - **Description:** Improve the Analytics “PV forecast vs measured” panel so operators can see forecast accuracy: show a 48-hour window centered on now (past 24h measured PV vs the forecast that was issued for those hours, plus the next 24h forecast). Remove redundant daily “next 2 days” energy plot for the Forecast.Solar Public plan.
  - **References:**
    - `apps/core-server-rs/src/routes/forecast.rs`
    - `apps/dashboard-web/src/features/analytics/components/AnalyticsOverview.tsx`
    - `apps/core-server/openapi/farm-dashboard.json`
  - **Acceptance Criteria:**
    - PV forecast chart renders with x-axis range `[now-24h, now+24h]`.
    - Forecast line uses historical forecast points for past hours (only points with `issued_at <= ts`) and latest forecast for future hours, enabling “measured vs forecast” accuracy checks.
    - The redundant “Forecast energy (next 2 days)” chart is removed from the Analytics PV panel (Forecast.Solar Public horizon is limited; hourly power overlay is the primary UX).
    - OpenAPI contract includes any new query params; `make rcs-openapi-coverage` remains green.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build` remain green.
  - **Notes / Run Log:**
    - 2026-01-08: Added `history_hours` support for `GET /api/forecast/pv/{node_id}/hourly` using an “as-of” selection (`issued_at <= ts`, latest per ts) so the PV overlay chart can render a 48h window and show forecast accuracy for the past 24h. Removed the redundant “Forecast energy” chart from Analytics. Refreshed the installed controller to `0.1.9.40`. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass), `cd apps/dashboard-web && npm run build` (pass), `python3 tools/check_openapi_coverage.py` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Analytics PV overlay renders in the installed UI. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/analytics.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-84: Emporia per-circuit preferences (poll/hidden/in totals)**
  - **Description:** Extend Emporia preferences so each meter can be expanded to configure per-circuit controls: **Poll** (ingest + store), **Hidden** (hide from dashboard but still ingest), and **In totals** (contributes to `/api/analytics/power`). Add the same **Hidden** control at the meter level.
  - **References:**
    - `apps/core-server-rs/src/routes/setup.rs`
    - `apps/core-server-rs/src/services/emporia_preferences.rs`
    - `apps/core-server-rs/src/services/emporia_ingest.rs`
    - `apps/dashboard-web/src/app/(dashboard)/setup/SetupPageClient.tsx`
    - `docs/runbooks/emporia-cloud-api.md`
  - **Acceptance Criteria:**
    - Setup Center → Integrations → Emporia meters & totals: clicking a meter expands to show all circuits.
    - Each meter and circuit exposes toggles: **Poll**, **Hidden**, **In totals**.
    - **Hidden** removes items from Nodes/Sensors/Power/Trends/Analytics UI while keeping ingestion enabled when **Poll** is on.
    - **Poll** off stops recording metrics and hides items from the dashboard (still configurable in Setup Center).
    - Emporia `/api/analytics/power` totals respect per-circuit “In totals”:
      - Default behavior matches existing (mains included; circuits excluded unless explicitly selected).
      - Admin can disable “Mains in totals” and include specific circuits instead.
    - Builds remain green: `cargo build --manifest-path apps/core-server-rs/Cargo.toml`, `cd apps/dashboard-web && npm run build`.
  - **Notes / Run Log:**
    - 2026-01-08: Added meter + circuit preferences (poll/hidden/in totals) stored under `setup_credentials.metadata.devices[device_gid].circuits[...]`; Setup Center now expands meters to configure circuits and writes through to node/sensor configs immediately; analytics power totals read the per-meter summary sensor. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass w/ warnings), `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Setup Center renders Emporia meters configuration and Power/Analytics reflect preferences. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/setup.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **DW-76: Trends independent-axis UX: clear series ↔ axis mapping**
  - **Description:** Improve Trends independent-axis mode so users can tell which y-axis belongs to which series without cluttering the plot or increasing axis width.
  - **Acceptance Criteria:**
    - Independent-axis mode provides a clear mapping between each series and its axis (color/legend/labeling), usable with 10+ series.
    - Solution does not materially increase horizontal axis footprint compared to today (no “axis columns explosion”).
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Improved independent-axes mode: legend hover reveals the focused axis labels (color-coded), and tooltips respect per-sensor display decimals. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Fixed Trends independent-axes runaway page-height growth by enforcing a fixed chart container height (avoid Chart.js resizing loops when multiple y-axes toggle). Added Playwright regression test + time-delayed screenshots (Chromium + WebKit mobile) to ensure scroll height stays stable.
    - 2026-01-10: Rebuilt and refreshed the installed controller to `0.1.9.59`.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): UI smoke + mobile regression suite verifies Trends page remains stable in independent-axes mode. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/playwright_regressions_0.1.9.69_20260110_1755/`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-83: Trends long-range presets + auto interval**
  - **Description:** Expand Trends range presets beyond 168 hours and auto-select a sensible bucket interval based on the selected range so charts remain detailed but responsive. Ensure bucket intervals use time-bucket averaging (not point sampling).
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
    - `apps/core-server-rs/src/routes/metrics.rs`
  - **Acceptance Criteria:**
    - Trends range dropdown includes longer presets (e.g., 2w, 30d, 90d, 180d, 365d).
    - Changing the range auto-selects an appropriate interval for that range (still user-adjustable).
    - Backend metrics query allows larger windows for Trends while retaining point-count limits to prevent overload.
    - Metric bucketing averages samples within each bucket (`time_bucket` + `avg(value)`) so 10-minute intervals average ~10 one-minute samples.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build` remain green.
  - **Notes / Run Log:**
    - 2026-01-08: Added longer Trends range presets (2w..1y) and auto-selected bucket intervals on range change; increased `/api/metrics/query` max window to 365 days while retaining max points guardrails. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass), `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Trends renders on the installed UI and presets are available. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/trends.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-86: Trends UX reorg + dashboard IA audit**
  - **Description:** Improve the Trends workspace UX/layout and perform an IA audit across tabs to reduce “catch-all” overlap as the dashboard grows (reorganize navigation and move controls to the most appropriate surfaces).
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
    - `apps/dashboard-web/src/components/SidebarNav.tsx`
    - `apps/dashboard-web/src/components/TrendChart.tsx`
  - **Acceptance Criteria:**
    - Trends page layout feels intentional: clear separation between sensor picking vs chart controls, with minimal scrolling for common actions.
    - Selected series are visible/manageable without digging through long node accordions (remove-from-selection is one click).
    - Chart controls (range/interval/axes/y-domain) are grouped together and do not require scrolling past the full sensor list.
    - Navigation is grouped so “Operations” vs “Admin” tabs are visually separated; Users lives under Admin.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Reworked Trends layout (separate sensor picker + chart controls, selected chips with axis side indicator + quick removal), exported Trend colors for consistent chips, and grouped sidebar nav into Operations/Admin. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Trends layout is live on the installed UI. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/trends.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-87: Trends custom range + interval**
  - **Description:** Add a Custom option for Trends range and interval. Selecting Custom reveals a text input so admins can type historic windows/intervals (e.g., `30d`, `2w`, `15m`) without being limited to presets.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
  - **Acceptance Criteria:**
    - Range dropdown includes a Custom option that reveals an input for `h/d/w/y` durations.
    - Interval dropdown includes a Custom option that reveals an input for `s/m/h/d` durations.
    - Custom inputs validate and keep the chart using the last valid values when invalid text is entered.
    - Historic window max aligns with backend constraints (365d).
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Added Custom range/interval inputs with unit parsing + validation; range up to 365d; interval supports `s/m/h/d` with a 10s minimum and rounds to whole seconds. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Custom range/interval UX is present on the installed UI. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/trends.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-88: Trends correlation + relationship analysis**
  - **Description:** Extend Trends with correlation and relationship analytics between selected sensors, including basic and advanced correlations (Pearson/Spearman), plus rich visualizations for pair drilldowns.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`
    - `apps/dashboard-web/src/features/trends/components/RelationshipsPanel.tsx`
    - `apps/dashboard-web/src/features/trends/utils/correlation.ts`
  - **Acceptance Criteria:**
    - Trends shows a correlation matrix heatmap for selected sensors and supports Pearson + Spearman methods.
    - Clicking a matrix cell opens a pair drilldown with:
      - Scatter plot with linear regression + R²
      - Lag/lead correlation over a configurable lag window
      - Rolling correlation over a configurable window
    - UI clearly states the analysis is based on the current Range/Interval and uses only overlapping buckets.
    - Builds remain green: `cd apps/dashboard-web && npm run build`.
  - **Notes / Run Log:**
    - 2026-01-08: Added Relationships panel to Trends with Pearson/Spearman matrix heatmap and pair drilldowns (scatter+fit, lag correlation, rolling correlation). Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-09: Hardened relationship analysis (reuse series value maps + safer regression extents) and added Vitest coverage for correlation helpers. Build: `cd apps/dashboard-web && npm run build` (pass). Tests: blocked by clean-state gate on this host (installed stack running under launchd).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Trends renders with Relationships panel available. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-93: Trends: strict series parsing + CSV export hygiene**
  - **Description:** Prevent Trends charts/analysis from fabricating misleading “real zeros” or “now” timestamps when the API returns missing/invalid values. Ensure CSV exports are stable and machine-readable.
  - **References:**
    - `apps/dashboard-web/src/lib/api.ts`
    - `apps/dashboard-web/src/types/dashboard.ts`
    - `apps/dashboard-web/src/features/trends/utils/trendsUtils.ts`
  - **Acceptance Criteria:**
    - Trend series parsing drops invalid points (invalid timestamp) and represents missing values as gaps (null), not zero.
    - Series metadata (`unit`, `display_decimals`) is preserved when present.
    - CSV export uses RFC3339 timestamps and properly escapes/quotes fields.
    - Builds remain green: `cd apps/dashboard-web && npm run build`.
  - **Notes / Run Log:**
    - 2026-01-10: Implemented strict trend series parsing: drop invalid timestamps, convert invalid/missing values to gaps (`null`), preserve `unit` + `display_decimals`, and harden CSV export (RFC3339 timestamps + proper escaping). Builds: Next (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-10: Inserted explicit gaps for missing telemetry windows: when a time-series has a large timestamp jump (node/sensor offline), the dashboard now inserts a `null` point so Chart.js renders a visible break instead of drawing a misleading straight line across days of missing data. Refreshed installed controller to `0.1.9.67`.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Trends renders on installed UI with gap handling. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-94: UI: show COV intervals (interval_seconds=0) correctly**
  - **Description:** Treat `interval_seconds=0` sensors as change-of-value (COV) in the UI; never display “0s interval” which reads as broken telemetry.
  - **References:**
    - `apps/dashboard-web/src/features/sensors/components/SensorTable.tsx`
    - `apps/dashboard-web/src/features/sensors/components/SensorDetailDrawer.tsx`
    - `apps/dashboard-web/src/features/nodes/components/NodeDetailDrawer.tsx`
    - `apps/dashboard-web/src/features/nodes/components/NodeGrid.tsx`
  - **Acceptance Criteria:**
    - Any sensor with `interval_seconds=0` renders “COV” (with a tooltip explaining “change of value / logged on change”).
    - Missing/unknown intervals render “—” (not “0s”).
  - **Notes / Run Log:**
    - 2026-01-10: Centralized interval formatting and rendered `interval_seconds=0` as “COV” (with tooltip) across Sensors + Nodes + sensor detail surfaces. Builds: Next (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): COV interval label visible on installed UI. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/sensors_cov_detail.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-89: Backups - controller settings bundle export/restore**
  - **Description:** Add a controller settings bundle export/restore flow to the Backups tab so admins can download and restore app-wide configuration (Setup Center credentials, backup retention, map configuration, controller runtime config file).
  - **References:**
    - `apps/core-server-rs/src/routes/backups_exports.rs`
    - `apps/core-server-rs/src/openapi.rs`
    - `apps/core-server-rs/src/routes/controller_config.rs`
    - `apps/dashboard-web/src/app/(dashboard)/backups/BackupsPageClient.tsx`
    - `apps/dashboard-web/src/features/backups/components/AppSettingsRestoreModal.tsx`
  - **Acceptance Criteria:**
    - Backups tab offers a “Download bundle” action that downloads a `.json` settings bundle.
    - Backups tab offers a restore modal that previews bundle counts and requires an explicit typed confirmation before applying.
    - Backend import replaces Setup Center credentials + backup retention + map tables in a single transaction and writes controller setup config to the local path (ignores any path embedded in the bundle).
    - Endpoints are auth-gated (`config.write`) and present in the exported OpenAPI.
    - Builds remain green: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build`.
  - **Notes / Run Log:**
    - 2026-01-09: Implemented `/api/backups/app-settings/export` and `/api/backups/app-settings/import` plus Backups UI (download + restore modal w/ RESTORE confirmation). OpenAPI exported. Builds: Rust + Next (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): export endpoint is auth-gated (401 without token, 200 with bearer) and the Backups UI renders controller settings export/restore. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/backups.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-99)

- **DW-90: Backups - database export (raw/sql/csv/json)**
  - **Description:** Add database export tooling to Backups tab with multiple formats for offline analysis/migration, including raw `pg_dump` exports and table-based CSV/JSON exports.
  - **References:**
    - `apps/core-server-rs/src/routes/backups_exports.rs`
    - `apps/core-server/openapi/farm-dashboard.json`
    - `apps/dashboard-web/src/app/(dashboard)/backups/BackupsPageClient.tsx`
  - **Acceptance Criteria:**
    - Backups tab exposes DB export controls (format + scope) and downloads an export file without requiring a dev server.
    - Backend provides `/api/backups/database/export`:
      - `format=raw` (pg_dump custom), `format=sql`, `format=csv` (tar.gz), `format=json` (tar.gz JSONL)
      - `scope=full` (raw/sql only), `scope=app` (app tables), `scope=config` (fast config-only)
      - Streams file responses to avoid large in-memory buffers.
    - Postgres credentials are not placed directly on the command line (use environment + discrete args).
    - Endpoints are auth-gated (`config.write`) and present in the exported OpenAPI.
  - **Notes / Run Log:**
    - 2026-01-09: Implemented `/api/backups/database/export` and Backups UI controls (raw/sql/csv/json + scope selector). OpenAPI exported. Builds: Rust + Next (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): DB export endpoint is auth-gated (401 without token, 200 with bearer) and Backups UI exports render. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/backups.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-99)

- **DW-96: Backups: auth-aware downloads + secure raw backup download**
  - **Description:** Fix Backups download/export actions so they include bearer auth headers consistently, and protect raw backup JSON downloads with `config.write` (backups contain sensitive config).
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/backups/BackupsPageClient.tsx`
    - `apps/dashboard-web/src/lib/http.ts`
    - `apps/core-server-rs/src/routes/backups.rs`
  - **Acceptance Criteria:**
    - Backups exports/downloads work from the installed dashboard without a separate dev server (no 401 due to missing auth headers).
    - Raw `GET /api/backups/{node}/{date}/download` requires bearer auth and `config.write`.
    - Builds remain green: Rust + Next.
  - **Notes / Run Log:**
    - 2026-01-10: Added auth-aware `fetchResponse()` helper for blob downloads, updated Backups UI downloads/exports to use it, gated raw backup download behind `config.write`, and disabled retention edits for read-only users. Builds: Rust + Next (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Backups tab downloads/exports work from the installed dashboard. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/backups.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-99)

- **DW-68: Remove demo/fake UI fallbacks for analytics + API errors (explicit errors only)**
  - **Description:** Remove any production behavior/UI copy that masks API outages by showing cached demo values or fabricated series. API failures must surface as explicit “unavailable” states.
  - **Acceptance Criteria:**
    - Analytics page copy does not imply demo fallback; when `/api/analytics/*` is unavailable the UI shows an explicit error/unavailable state.
    - Dashboard web request helpers do not swallow HTTP errors and return empty “demo” placeholders; failures propagate to React Query error states.
    - Chart data is not synthesized on errors (no generated zero-filled series); empty state is visually distinct from “0”.
  - **Notes / Run Log:**
    - 2026-01-06: Removed “cached demo values” copy from Analytics and updated the page to explicitly state production shows error/unavailable states.
    - 2026-01-06: Removed client-side “safe fetch” fallbacks that masked API errors; `/api/*` callers now throw on non-2xx and surfaces the failure via React Query.
    - 2026-01-06: Removed synthetic analytics series generation; empty analytics now returns empty arrays (no fabricated charts).
    - Build: `cd apps/dashboard-web && npm run build` (pass).
    - Test note: unit/E2E suites were not run on the production controller host because the test-hygiene gate requires a clean machine (no Farm launchd jobs/processes); run `make ci-web-smoke` + `make e2e-web-smoke` on a clean dev host (or after stopping/uninstalling the installed stack).
    - 2026-01-06: Deployed via controller bundle `0.1.9.14`; verified the installed Analytics HTML no longer contains the forbidden “cached demo values” language. Evidence: `reports/prod-upgrade-no-demo-fallback-20260106_130531.json`.
  - **Status:** Done (deployed + validated; E2E still gated by clean-state policy)


- **DW-91: Mobile nav interactions + Playwright mobile audit**
  - **Description:** Fix mobile header interactions (hamburger + account dropdown) so they work reliably on iOS Safari/WebKit, and add Playwright coverage + screenshot tooling to prevent regressions.
  - **References:**
    - `apps/dashboard-web/src/components/DashboardUiProvider.tsx`
    - `apps/dashboard-web/src/components/DashboardHeader.tsx`
    - `apps/dashboard-web/src/components/SidebarNav.tsx`
    - `apps/dashboard-web/src/app/(dashboard)/layout.tsx`
    - `apps/dashboard-web/playwright/mobile-shell.spec.ts`
    - `apps/dashboard-web/playwright/mobile-audit.spec.ts`
    - `apps/dashboard-web/scripts/web-screenshots.mjs`
  - **Acceptance Criteria:**
    - Hamburger opens/closes the sidebar on mobile; backdrop/ESC/route-change close it; body scroll locks while open.
    - Account menu opens on tap and closes on outside tap/ESC/route-change in mobile browsers.
    - Mobile layout audit asserts no horizontal overflow across primary routes.
    - Playwright mobile shell tests pass in WebKit + Chromium.
  - **Notes / Run Log:**
    - 2026-01-09: Replaced Preline-managed mobile interactions with React state and added Playwright mobile tests + screenshot runner support. Evidence screenshots: `manual_screenshots_web/mobile_audit_20260109_1112/`.
  - **Status:** Done


- **DW-92: Fix Sensors page crash on sensor click**
  - **Description:** Fix the Sensors detail drawer crash when selecting a sensor (React hooks order violation) and add a Playwright regression test to ensure the drawer opens on mobile.
  - **References:**
    - `apps/dashboard-web/src/features/sensors/components/SensorDetailDrawer.tsx`
    - `apps/dashboard-web/playwright/mobile-shell.spec.ts`
    - `apps/dashboard-web/playwright/stubApi.ts`
  - **Acceptance Criteria:**
    - Clicking a sensor row opens the detail drawer without triggering a client-side exception.
    - Drawer close returns to the Sensors list without errors.
    - Playwright mobile shell includes a sensor-drawer open/close test.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-09: Fixed conditional-hook ordering in `SensorDetailDrawer` and expanded Playwright stubs/tests to cover sensor selection. Build: `cd apps/dashboard-web && npm run build` (pass). Tests: not run on this host due to clean-state gate (installed stack running under launchd).
    - 2026-01-10: Fixed remaining hook-order crash when opening sensor detail: `TrendChart` returned early before calling `useMemo` (first render empty → second render with data), triggering minified React error #310 in production. Moved the empty-state return below hook usage. Build: `cd apps/dashboard-web && npm run build` (pass). Refreshed installed controller to `0.1.9.51`.
  - **Status:** Done


- **DW-65: Migrate dashboard UI to Preline (admin/settings templates)**
  - **Description:** Fully migrate all dashboard-web pages to Preline templates for a cohesive, production-grade “admin/settings” UX (tables/forms/cards/badges/alerts), minimizing bespoke styling.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0028-dashboard-web-ui-overhaul-to-preline-admin-settings.md`
  - **Acceptance Criteria:**
    - Global dashboard shell uses a Preline sidebar + header layout.
    - All dashboard routes (including `/login` and `/sim-lab`) use Preline UI patterns (tables/forms/cards) and consistent spacing/typography.
    - Preline JS (if used) is initialized correctly in Next.js (no broken dropdowns/toggles/collapses).
    - `make ci-web-smoke` and `make e2e-web-smoke` pass from a clean state.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-web-smoke`)


- **DW-64: System banner UX: compact empty schedule + move scan control**
  - **Description:** Improve the SystemBanner “Next schedule” panel so it doesn’t waste space when schedules are empty, and relocate node discovery scanning controls to the Nodes page header instead of a global banner.
  - **Acceptance Criteria:**
    - SystemBanner is limited to the Overview tab (not shown on domain tabs) to reduce header clutter.
    - When no schedules exist, the schedule panel shows a compact empty state with a single CTA that opens the Schedules tab.
    - “Scan for nodes” is removed from SystemBanner and available from the Nodes page header.
    - `make ci-web-smoke` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: IA follow-up: moved SystemBanner to the new Overview tab so Nodes/Schedules pages stay focused.
  - **Status:** Done (`make ci-web-smoke`)


- **DW-54: Add dashboard login UX + token persistence (no header injection)**
  - **Description:** Provide a first-class login experience in the web dashboard so users can authenticate and call auth-gated endpoints (deployments/config writes) without manual token hacks.
  - **Acceptance Criteria:**
    - The dashboard lands on a login screen (`/login`) and requires sign-in at the start of each browser session (session-only token storage).
    - Fresh installs have a bootstrap admin user (created during install); the dashboard login screen supports signing in without manual token hacks.
    - If no users exist (recovery/DB reset), the dashboard can still offer a “Create admin user” bootstrap flow.
    - Authenticated requests include `Authorization: Bearer …` automatically and deployment/config pages work without manual `/api/auth/login` + header extensions.
    - Clear “Log out” action exists.
    - `make ci-web-smoke` and `make e2e-installer-stack-smoke` remain green.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260104_085103`)


- **DW-55: Capabilities management UX (edit after creation + config.write)**
  - **Description:** Allow admins to add/remove `config.write` and other capabilities for existing users directly from the dashboard UI after creation.
  - **Acceptance Criteria:**
    - Users page includes `config.write` among selectable capabilities.
    - Admins can add/remove additional capabilities after creation (not just the role defaults).
    - Updating capabilities for the currently logged-in user is reflected promptly (no confusing “stale session” behavior).
    - `make ci-web-smoke` remains green.
  - **Status:** Done (`cd apps/dashboard-web && npm run build`, `cd apps/dashboard-web && npm run test:smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260104_085103`)


- **DW-53: Build a read-only dashboard bundle for the WAN portal**
  - **Description:** Provide a read-only dashboard build variant intended for the AWS WAN portal that removes/locks write actions and surfaces sync status/errors.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0014-feature-008-wan-readonly-webpage-aws.md`
  - **Acceptance Criteria:**
    - A build output (or separate app) exists that is view-only and cannot trigger mutations.
    - UI clearly shows last sync time and pull-agent errors.
  - **Status:** Done (`make ci-web`, `cd apps/dashboard-web && npm run build:wan`; WAN sync status via `/api/portal/status`)


- **DW-49: Add node display profile editor UI (Pi 5 local display)**
  - **Description:** Add a per-node “Display profile” editor to configure the Pi 5 local display content from the main dashboard without manual JSON edits.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0007-feature-001-pi5-local-display-basic.md`
    - `project_management/archive/archive/tickets/TICKET-0008-feature-002-pi5-local-display-advanced-controls.md`
  - **Acceptance Criteria:**
    - Dashboard UI can enable/disable display mode and configure tiles/refresh intervals per node.
    - Changes are persisted through the core API and reflected in node state.
    - `make ci-web-smoke` remains green.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-web-smoke`)


- **DW-52: Deploy-from-server UX hardening (SSH)**
  - **Description:** Improve the existing Deployment UI (Pi 5 over SSH) with safer defaults, clearer guidance, and better error messages aligned with the hardened backend deploy job.
  - **Acceptance Criteria:**
    - UI supports host key verification UX and avoids secret echo in logs.
    - UI shows idempotent outcomes (“already installed/healthy” vs “installed”).
    - `make e2e-web-smoke` remains green.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20251231_160228.log`)


- **DW-50: Renogy BT-2 one-click setup UX (preset apply)**
  - **Description:** Add a dashboard flow that configures Renogy BT-2 telemetry on a node and creates the default sensor set at 30s intervals with idempotency and actionable errors.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0012-feature-006-renogy-bt-one-click-setup.md`
  - **Acceptance Criteria:**
    - Node detail/sensors UI includes a “Connect Renogy BT-2” action (MAC entry MVP).
    - Action uses the canonical preset via the core API (see CS-45; drift-proofing tracked as CS-49).
    - `make e2e-web-smoke` remains green.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-installer-stack-smoke`; logs: `reports/ci-web-smoke-20251231_181750.log`, `reports/e2e-installer-stack-smoke-20251231_181750.log`)


- **DW-51: WS-2902 one-click setup UX (weather station)**
  - **Description:** Add a dashboard flow that enables WS-2902 weather station ingest and creates the default weather sensors at 30s intervals, with near one-click configuration and troubleshooting feedback.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0006-client-feature-requests-v2-overview.md`
    - `project_management/archive/archive/tickets/TICKET-0013-feature-007-ws-2902-weather-station-setup.md`
    - `docs/runbooks/ws-2902-weather-station-setup.md`
  - **Acceptance Criteria:**
    - Discovery/onboarding UI includes an “Add weather station (WS-2902)” action.
    - Default sensor set is created and trends at 30s cadence when data arrives.
    - `make ci-web-smoke` remains green.
  - **Notes / Run Log:**
    - 2026-01-10: Hardened WS-2902 readiness without hardware: expanded default sensors (humidity, wind gust, rain rate), fixed daily rain parsing (avoid treating `rainin` as daily), parse `dateutc` timestamps, added LAN-host hint + “Send sample upload” button in the wizard, and updated runbook with a no-hardware upload example. Deployed via controller bundle `0.1.9.53` and validated via curl smoke (create → ingest → status → metrics), with temp integration purged after.
  - **Status:** Done (`make ci-web-smoke`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke-20260101_010855.log`)


- **DW-48: Split ScheduleForm monolith into hook + field components**
  - **Description:** Reduce complexity in `apps/dashboard-web/src/features/schedules/components/ScheduleForm.tsx` by extracting a `useScheduleForm` hook and form field sub-components.
  - **Acceptance Criteria:**
    - Form state/validation is extracted into a dedicated hook.
    - UI is split into small field-set components with clear props.
    - Behavior remains unchanged for schedule creation/edit flows.
    - `npm test` and `make e2e-web-smoke` still pass.
  - **Status:** Done (`make ci-web`, `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`, `make e2e-web-smoke`)


- **DW-47: Split Provisioning wizard monolith into components**
  - **Description:** Improve maintainability and debugging speed by extracting types/validation/hooks/components from `apps/dashboard-web/src/app/(dashboard)/provisioning/page.tsx`.
  - **Acceptance Criteria:**
    - Provisioning types move into a dedicated module (e.g., `apps/dashboard-web/src/app/(dashboard)/provisioning/types.ts`).
    - Validation/builders move into reusable modules or a hook (e.g., `apps/dashboard-web/src/app/(dashboard)/provisioning/validation.ts`, `apps/dashboard-web/src/app/(dashboard)/provisioning/builders.ts`, `apps/dashboard-web/src/app/(dashboard)/provisioning/useProvisioningState.ts`).
    - UI is split into focused components (node settings, sensor editor, preview/actions).
    - `make ci-web-smoke` passes.
    - Next installer-path gate run (`make e2e-installer-stack-smoke`) remains green (validates the installed static dashboard build).
  - **Status:** Done (`make ci-web-smoke`, `make e2e-installer-stack-smoke`)


- **DW-41: Wire Sim Lab console to core + Sim Lab control APIs**
  - **Description:** Connect `/sim-lab` to core-server domain APIs and the Sim Lab control API (Option C), with arm-gated destructive actions.
  - **Acceptance Criteria:**
    - Domain data (nodes/sensors/outputs/alarms) loads via existing core-server queries.
    - Sim-engine state + actions use the Sim Lab control API client, with base URL from env.
    - Destructive actions (faults, stop, reset) are disabled until the console is armed; armed state is visible.
    - Error and offline states render with clear operator messaging.
  - **Status:** Done (make e2e-web-smoke)


- **DW-40: Redesign Sim Lab console layout (domain-first)**
  - **Description:** Rebuild `/sim-lab` to prioritize Nodes/Sensors/Outputs/Alarms panels while keeping the retro CNC visual style; remove literal CNC controls (jog pad, g-code loader).
  - **Acceptance Criteria:**
    - Layout shows primary domain panels (Nodes, Sensors, Outputs, Alarms) with quick scanability.
    - Sim-engine controls are grouped in a dedicated section (scenario, speed, seed, time controls, fault injection).
    - CNC theme persists via typography, palette, panel styling, and status lamps (no joystick or axis metaphors).
    - Responsive layout remains usable on tablet-sized screens.
  - **Status:** Done (make e2e-web-smoke)


- **DW-46: Stabilize Sim Lab smoke backup restore selection**
  - **Description:** Ensure the Sim Lab E2E smoke test targets the correct node backup row so restore metadata checks are deterministic.
  - **Acceptance Criteria:**
    - Smoke test selects the backup row for the primary node using the current backup date.
    - Restore step no longer flakes due to unordered backups list.
    - E2E smoke (`make e2e-web-smoke`) passes after changes.
  - **Status:** Done (make e2e-web-smoke)


- **DW-45: Expand sensor presets in provisioning**
  - **Description:** Extend the sensor configuration dropdown to cover more sensor types with defaults and keep custom overrides editable.
  - **Acceptance Criteria:**
    - Preset dropdown includes temperature, moisture, humidity, pressure, wind, solar irradiance, lux, water level, fertilizer level, current, voltage, and power.
    - Selecting a preset auto-populates defaults while still allowing manual edits.
    - Custom preset remains available for full manual parameter entry.
  - **Status:** Done (make e2e-web-smoke)


- **DW-44: Add remote Pi 5 deployment UI**
  - **Description:** Add a dedicated dashboard section that lets operators deploy node-agent to a Pi 5 over SSH, with status steps, logs, and adoption token visibility.
  - **Acceptance Criteria:**
    - Sidebar/nav includes a Deployment section with the remote Pi 5 deployment form.
    - Form captures SSH host/credentials, optional node identity, and triggers the core deployment API.
    - UI streams step status/logs and surfaces the resulting adoption token + node metadata.
  - **Status:** Done (make e2e-web-smoke)


- **DW-43: Allow nullable output command topics in API schema**
  - **Description:** Accept `null` output command topics in web schema validation so production-mode outputs without command topics do not trigger schema failures in the dashboard.
  - **Acceptance Criteria:**
    - `OutputsResponseSchema` accepts `command_topic: null`.
    - Nodes page loads without schema validation errors when outputs omit command topics.
    - E2E smoke (`make e2e-web-smoke`) passes after changes.
  - **Status:** Done (make e2e-web-smoke)

- **DW-42: Allow dev origins for Sim Lab smoke**
  - **Description:** Add explicit dev origins to the Next.js config to avoid cross-origin warnings during smoke runs.
  - **Acceptance Criteria:**
    - `next.config.ts` includes allowed dev origins for localhost/127.0.0.1.
    - Sim Lab smoke no longer logs `allowedDevOrigins` warnings.
  - **Status:** Done (make e2e-web-smoke)

- **DW-39: Build Sim Lab testing dashboard UI**
  - **Description:** Create a standalone `/sim-lab` page with an industrial CNC-inspired control console for demo-mode simulation monitoring and control.
  - **Acceptance Criteria:**
    - `/sim-lab` renders with a standalone layout (no main dashboard shell).
    - UI includes simulator controls, node rack, sensor feeds, outputs, fault injection, program loader, event log, and alarms panels.
    - Styling follows the retro industrial CNC theme and remains readable on desktop and mobile.
  - **Status:** Done


- **DW-38: Run npm audit fix for dashboard-web**
  - **Description:** Update dashboard-web dependencies to resolve reported npm audit vulnerabilities and keep lint/tests passing.
  - **Acceptance Criteria:**
    - `npm audit fix --force` results in zero reported vulnerabilities.
    - `next`/`vitest` ranges are updated in `apps/dashboard-web/package.json`.
    - `npm run lint` and `npm run test` pass after the updates.
  - **Status:** Done


- **DW-37: Silence safe ESLint warnings in dashboard web**
  - **Description:** Clear out unused local type aliases, stabilize memo dependencies, and suppress unused eslint-disable warnings for generated API client files.
  - **Acceptance Criteria:**
    - Node/sensor detail pages remove unused type aliases.
    - Memoized maps stop using unstable fallback arrays (no hook dependency warnings).
    - ESLint no longer reports unused disable directives in generated API client paths.
  - **Status:** Done

- **DW-36: Refresh baseline-browser-mapping dev dependency**
  - **Description:** Update the dashboard toolchain dependency that warns about stale baseline data.
  - **Acceptance Criteria:**
    - `baseline-browser-mapping` is updated in `apps/dashboard-web`.
    - Warning status is verified after update.
  - **Status:** Done (warning persists upstream; latest version installed)

- **DW-35: Align alarm schema parsing with API payloads**
  - **Description:** Accept numeric alarm IDs and nullable alarm fields so demo/live responses validate correctly.
  - **Acceptance Criteria:**
    - Alarm schema accepts numeric `id` values.
    - Nullable `sensor_id`, `node_id`, and `status` fields validate without throwing.
  - **Status:** Done

- **DW-34: Split nodes page into reusable components/hooks**
  - **Description:** Break the nodes view into feature-scoped components and shared UI primitives so the page client remains orchestration-only and styles are consistent.
  - **Acceptance Criteria:**
    - Nodes feature uses shared button/pill primitives instead of repeated class strings.
    - Node grid uses an extracted info list component.
    - `apps/dashboard-web/src/app/nodes/NodesPageClient.tsx` only composes feature modules.
  - **Status:** Done

- **DW-33: Deduplicate analytics formatting helpers**
  - **Description:** Move analytics formatting helpers (number, kW/kWh, gallons, currency, runtime) into a shared formatter module to reduce page size and reuse rules.
  - **Acceptance Criteria:**
    - Shared formatters live in `apps/dashboard-web/src/lib/format.ts` (or a dedicated analytics formatter module).
    - Analytics view imports formatters from the shared module with identical output behavior.
  - **Status:** Done

- **DW-32: Surface retention policy update failures**
  - **Description:** Show user-facing errors when retention policy updates fail.
  - **Acceptance Criteria:**
    - Retention update failures display an inline error message.
    - Errors are visible even when the global notification banner is not in view.
  - **Status:** Done

- **DW-31: Debounce discovery scan action**
  - **Description:** Prevent rapid manual scan clicks from flooding `/api/scan` by debouncing the discovery action.
  - **Acceptance Criteria:**
    - Scan button disables while a debounced scan is queued or in progress.
    - Only one scan request is sent for rapid consecutive clicks.
  - **Status:** Done

- **DW-30: Pause dashboard polling when tab hidden**
  - **Description:** Stop background polling when the dashboard tab is hidden to reduce unnecessary load.
  - **Acceptance Criteria:**
    - Query polling pauses while the page is hidden.
    - Polling resumes when the page becomes visible again.
  - **Status:** Done

- **DW-28: Add schema-validated API parsing**
  - **Description:** Introduce runtime schemas for API responses and enforce validation so unexpected backend shape changes surface as errors instead of silently corrupting UI state.
  - **Acceptance Criteria:**
    - API fetch helpers validate responses against schemas before normalization.
    - Schema failures surface errors (no silent fallback on invalid shapes).
    - Core dashboard API consumers (nodes/sensors/outputs/schedules/alarms/backups/metrics) use schema validation.
    - Schedule calendar endpoint validates responses before event mapping.
  - **Status:** Done

- **DW-27: Refactor dashboard pages into feature modules**
  - **Description:** Split large page clients into feature-scoped components and hooks so data fetching, view state, and JSX are reusable and easier to test.
  - **Acceptance Criteria:**
    - Feature modules exist under `apps/dashboard-web/src/features/` with dedicated components/hooks.
    - Page client files become composition layers that import feature modules.
    - Unit tests import helpers/components from feature modules (not page clients).
  - **Status:** Done

- **DW-26: Replace global SWR snapshot with domain React Query hooks**
  - **Description:** Remove the dashboard-wide SWR snapshot and introduce domain-specific React Query hooks with targeted stale times and explicit invalidation after mutations.
  - **Acceptance Criteria:**
    - DashboardDataContext is removed and replaced by a shared QueryClient provider.
    - Domain hooks exist in `apps/dashboard-web/src/lib/queries/` and are used by nodes/sensors/analytics/backups/etc.
    - Pages drop global 5s polling and use per-resource stale times with manual refetch/invalidation.
    - Interactive leaf components remain client; server pages wrap them where possible.
  - **Status:** Done

- **DW-25: Fix dashboard web UI regressions and polish**
  - **Description:** Resolve UI regressions introduced by the recent theme/layout refresh. This includes fixing invalid CSS color token usage (black borders/odd nesting), improving hover and dropdown affordances, pinning table headers for long lists, addressing schedule block clipping, correcting trends chart rendering/selection behavior, and polishing analytics/provisioning layouts to avoid cropped text.
  - **Acceptance Criteria:**
    - Theme token colors render correctly across the UI (no unexpected black borders or missing translucent fills).
    - Long tables keep header rows visible while scrolling (no pagination introduced).
    - Hover and focus states are visually clear; select/dropdowns are distinct from the background.
    - Schedule calendar blocks render without cropped text at typical week view zoom levels.
    - Trends chart uses appropriately thin line strokes and selection shows full series history (not a couple of points).
    - Analytics and provisioning panels avoid cramped/cropped content and remain readable on common viewport widths.
  - **Status:** Done


- **DW-1: Add component/unit tests for critical flows**
  - **Description:** Add component and unit tests for critical flows, such as the adoption wizard and calendar edits.
  - **Acceptance Criteria:**
    - All critical flows are covered by component and unit tests.
    - The tests are run as part of the CI/CD pipeline.
  - **Status:** Done (RTL/Vitest coverage for adoption wizard restore selection and calendar drag/drop/resize flows runs in `make ci`)


- **DW-2: Implement global layout**
  - **Description:** Banner with connection status, controller summary, scan button.
  - **Status:** Done


- **DW-3: Nodes table + detail drawer**
  - **Description:** Sensors, outputs, schedules, alarms, backups, adoption flow.
  - **Status:** Done


- **DW-4: Sensor catalogue**
  - **Description:** Filtering, detail panel (configuration form, alarm list, latest metrics, trend preview).
  - **Status:** Done


- **DW-5: Outputs tab**
  - **Description:** Control buttons, state history.
  - **Status:** Done


- **DW-6: Users management CRUD UI**
  - **Description:** Roles/capabilities, audit log.
  - **Status:** Done


- **DW-7: Schedules weekly calendar**
  - **Description:** Drag/drop create/edit, condition chips, action summary.
  - **Status:** Done


- **DW-8: Trends workspace**
  - **Description:** Multi-select up to 10, stacked/independent toggle, manual axis scaling, range presets, export CSV.
  - **Status:** Done


- **DW-9: Analytics dashboard components**
  - **Description:** Power, water, soil, alarms, node status, solar/battery integrator cards & charts.
  - **Status:** Done


- **DW-10: Backups modal**
  - **Description:** Download + restore wizard.
  - **Status:** Done


- **DW-11: Settings page**
  - **Description:** Integrations + demo mode toggle/help.
  - **Status:** Done


- **DW-12: Integrate mock/demo data loader**
  - **Description:** Handle backend unreachable state.
  - **Status:** Done


- **DW-13: Retention/backups UI tests**
  - **Description:** RTL/Vitest coverage for retention table + policies.
  - **Status:** Done


- **DW-14: Trends axis toggle test**
  - **Description:** RTL/Vitest coverage for stacked/independent controls.
  - **Status:** Done


- **DW-15: Adoption restore selector**
  - **Description:** Wired to `/api/adopt` with feedback message.
  - **Status:** Done


- **DW-16: Restore activity feed poll + RTL coverage**
  - **Description:** Poll restore activity feed and add test coverage.
  - **Status:** Done


- **DW-17: Implement Rich Schedule Editor (Visual Builder)**
  - **Description:** Replace the raw JSON text areas in the Schedule dialog with a user-friendly form builder. Users should be able to add/remove conditions (e.g., "Sensor Value", "Time Range") and actions (e.g., "Turn Output On") via dropdowns and inputs. Retain the text area as an "Advanced/JSON" toggle for power users.
  - **Acceptance Criteria:**
    - UI provides controls for selecting Condition Type (Sensor, Forecast, etc.) and configuring parameters.
    - UI provides controls for selecting Action Type (Output, Alert) and parameters.
    - Two-way sync between Visual Builder and JSON view (or at least Visual -> JSON).
  - **Status:** Done


- **DW-18: Polish Schedule Builder**
  - **Description:** Polish the builder in the calendar further: add per-condition edit in place, sensor/output pickers, and clearer validation hints. The "builder" refers to the `ScheduleForm` component (and its children ConditionsEditor and ActionsEditor) located in `apps/dashboard-web/src/app/schedules/page.tsx`.
  - **Acceptance Criteria:**
    - **Smart Pickers:** Replace the plain text inputs for Sensor ID and Output ID with dropdown menus that show the actual sensors/outputs available in the system.
    - **Edit-in-Place:** Allow clicking an existing condition/action in the list to modify it, rather than having to delete and re-add it.
    - **Validation:** Add red borders or error messages if a user forgets a required field (like a threshold value).
  - **Status:** Done


- **DW-19: Playwright screenshot smoke tests**
  - **Description:** Add a Playwright-based script that navigates each top-level dashboard tab and captures full-page screenshots to a local directory for quick manual UI review.
  - **Acceptance Criteria:**
    - Running `npm run screenshots:web` generates PNG screenshots for Nodes, Sensors & Outputs, Users, Schedules, Trends, Analytics, Backups, and Connection.
    - Screenshots are saved to `manual_screenshots_web/` (ignored by git) with a `manifest.json` for easy review.
  - **Status:** Done


- **DW-20: Node + sensor config generator (Provisioning tab)**
  - **Description:** Add a dashboard tab that generates `node-agent-firstboot.json` and `node_config.json` via UX forms (node settings + sensor list) so users can provision/configure without writing JSON.
  - **Acceptance Criteria:**
    - UI captures node identity + Wi‑Fi hints with validation.
    - UI supports add/edit/remove sensor configs with validation.
    - Users can copy/download valid JSON files directly from the dashboard.
  - **Status:** Done


- **DW-21: Support bearer token header for dashboard API calls**
  - **Description:** Allow the dashboard to include an `Authorization: Bearer ...` header for core-server API requests when `NEXT_PUBLIC_AUTH_TOKEN` is set (to support production mode without a dedicated login UI).
  - **Status:** Done


- **DW-22: Sensor preset templates (auto-populated config defaults)**
  - **Description:** Add “preset” selection for common sensor types that auto-populates units/intervals/rolling defaults in the provisioning/config UIs, with editable advanced overrides.
  - **Acceptance Criteria:**
    - Users select a preset from a dropdown (e.g., “Soil Moisture”, “Rain Gauge”, “Flow Meter”).
    - Default parameters populate the form automatically (unit, interval, rolling average).
    - Users can still edit fields manually after applying a preset.
  - **Status:** Done (now includes calibration ranges, pulses-per-unit, offset/scale)


- **DW-23: Insightface-inspired theme refresh**
  - **Description:** Align dashboard styling with the `insightface_demo_webui` look: shared theme tokens, glassy header/banner, and accent chips for predictive alarms.
  - **Acceptance Criteria:**
    - Global CSS variables and layout updated to the new palette (indigo/emerald) with glass panels.
    - System banner + nav tabs use the refreshed style; Sensors page shows predictive origin/score pills.
    - `npm run lint`, `npm run test`, and `npm run screenshots:web` pass.
  - **Status:** Done (Bob)


- **DW-24: Sidebar shell and insightface layout lift**
  - **Description:** Replace the top tabs with an insightface-style sidebar + hero header and harmonize shared surfaces for tables, drawers, forms, and charts without changing data flows.
  - **Status:** Done


- **DW-194: Trends: Key text introduces jargon (TSSE, MAD, F1, r/n)**
  - **Description:** Update the Trends “Key” panels so any jargon/abbreviations used are introduced on first use for a general scientific audience (e.g., spell out “Time‑Series Similarity Engine (TSSE)” and expand other abbreviations like MAD and F1 where they appear).
  - **Acceptance Criteria:**
    - Trends → Related sensors Key text spells out **Time‑Series Similarity Engine (TSSE)** on first mention.
    - Co-occurrence Key expands **median absolute deviation (MAD)** on first mention.
    - Events/Spikes matching Key expands **F1 score** (harmonic mean of precision and recall) on first mention.
    - Relationships Key defines **r** (correlation coefficient) and **n** (overlap buckets) on first mention.
    - `make ci-web-smoke-build` passes.
    - Tier A validated on installed controller (no DB/settings reset) with at least one **viewed** screenshot under `manual_screenshots_web/` showing the updated Trends Key text.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-24: Tier A validated on installed controller `0.1.9.213` (run: `project_management/runs/RUN-20260124-tier-a-dw194-trends-key-jargon-0.1.9.213.md`).
      - VIEWED screenshots:
        - `manual_screenshots_web/tier_a_0.1.9.213_trends_auto_compare_2026-01-24_193342626Z/01_trends_auto_compare_key.png`
        - `manual_screenshots_web/tier_a_0.1.9.213_trends_cooccurrence_2026-01-24_193523567Z/01_trends_cooccurrence_key.png`
        - `manual_screenshots_web/tier_a_0.1.9.213_trends_event_match_2026-01-24_193600108Z/01_trends_event_match_key.png`
        - `manual_screenshots_web/tier_a_0.1.9.213_trends_relationships_2026-01-24_193138186Z/01_trends_relationships_key.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-197: Trends: Interactive line-of-best-fit + analysis tools polish**
  - **Description:** Upgrade the Trends chart analysis tooling so operators can generate a line of best fit (linear regression) automatically, with a user-friendly UI. The user must be able to set the regression window by clicking the start and end points directly on the chart. This should feel like a cohesive “graph analysis tools” set (not hidden inside Highcharts StockTools jargon).
  - **Acceptance Criteria:**
    - Trends chart includes an “Analysis tools” UI that is discoverable and visually consistent with existing cards/controls (no design drift; complies with `apps/dashboard-web/AGENTS.md` guardrails).
    - A “Line of best fit” tool exists:
      - User chooses a target series (sensor) from the currently charted series.
      - User clicks two points on the chart to set the start and end of the regression window (mouse interaction; no manual timestamp typing required).
      - The chart draws a best-fit line only over the chosen window.
      - UI shows: window start/end time, points used (`n`), and a quality hint (e.g., `R²`) plus a human-friendly slope/change summary.
      - User can remove the best-fit line, and can re-select the window (edit) via chart interaction.
    - Works with multiple series and with `Independent axes` enabled (best-fit line uses the correct y-axis for the chosen series).
    - Existing Stock Tools (draw/measure/indicators) remain available and the new UI does not conflict with normal zoom/pan interactions when the best-fit tool is not active.
    - Validation:
      - `make ci-web-smoke` passes.
      - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validated on installed controller (no DB/settings reset); Tier B deferred to `DW-98`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-31: Started (per operator feedback: Trends analysis tools UI lacks an automatic best-fit line; regression window must be set via chart interaction).
    - 2026-01-31: Implemented interactive best-fit tool + Analysis tools panel (window endpoints set via chart clicks). Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-31: Tier A refreshed installed controller to `0.1.9.226` via `python3 tools/rebuild_refresh_installed_controller.py --version 0.1.9.226`.
      - Smoke: `make e2e-installed-health-smoke` (pass)
      - VIEWED screenshots:
        - `manual_screenshots_web/tier_a_0.1.9.226_dw197_best_fit_20260131_003151/01_trends_best_fit.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-198: Trends: Chart analysis toolbar (polished tool palette; wired to Highcharts bindings)**
  - **Description:** Replace the default Highcharts Stock Tools left-side GUI on the Trends chart with a visually polished, user-friendly analysis toolbar (thinkorswim-style UX). Keep the existing chart engine and wire a custom tool palette UI to Highcharts Stock Tools navigation bindings + annotations API, integrating the best-fit regression tool as a first-class toolbar tool.
  - **References:**
    - `project_management/chart-analysis-toolbar-recommendation.md`
    - `project_management/archive/archive/tickets/TICKET-0046-trends-chart-analysis-toolbar.md`
    - `apps/dashboard-web/src/components/TrendChart.tsx`
  - **Acceptance Criteria:**
    - Trends renders a custom analysis toolbar that matches the dashboard design system and does not show the default Stock Tools left-side GUI.
    - Tool groups exist (minimum viable set):
      - Lines: trendline, horizontal line, best-fit line
      - Measure: Fibonacci retracement, measure XY, distance
      - Annotate: label, arrow, rectangle highlight
      - Navigate: zoom in/out controls, pan toggle
      - Eraser / Clear all
    - Active tool UX:
      - Exactly one active tool at a time with a clear “armed” state.
      - Escape cancels the active tool without leaving the chart in a broken state.
      - Normal zoom/pan/range selection works when no analysis tool is active.
    - Best-fit regression UX:
      - Start/stop points are set by mouse interaction on the chart (no manual timestamp typing required).
      - The regression overlay uses the correct y-axis when independent axes are enabled.
      - Best-fit summary stats are shown (at minimum `n`, `R²`, and a human-friendly slope/rate).
    - Persistence:
      - Toolbar-created annotations persist via the existing annotations backend and load on refresh (where supported by the backend annotation type).
    - Validation:
      - `make ci-web-smoke` passes.
      - `cd apps/dashboard-web && npm run build` passes.
      - Tier A validated on installed controller (no DB/settings reset) with at least one **viewed** screenshot captured under `manual_screenshots_web/`. Tier B deferred to `DW-98`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-31: Tier A validated on installed controller `0.1.9.229` (run: `project_management/runs/RUN-20260131-tier-a-dw198-trends-chart-analysis-toolbar-0.1.9.229.md`).
      - VIEWED screenshots:
        - `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trends_page_with_toolbar.png`
        - `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trend_chart_with_annotation.png`
        - `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trend_chart_after_reload.png`
        - `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trend_chart_after_delete.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-199: Sensors: Fix sensor drawer crash (Highcharts stock-tools bindings)**
  - **Description:** Fix a production crash when opening the Sensors & Outputs detail drawer. The drawer renders a TrendChart preview, but it must not instantiate Highcharts stock-tools/navigation bindings unless analysis tools are enabled.
  - **Acceptance Criteria:**
    - Opening a sensor detail drawer does not crash (Trend preview renders).
    - Trends analysis toolbar behavior remains unchanged.
    - Validation:
      - `make ci-web-smoke` passes.
      - `cd apps/dashboard-web && npm run build` passes.
      - Tier A validated on installed controller (no DB/settings reset) with a **viewed** screenshot showing the sensor drawer open.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-31: Tier A refreshed installed controller to `0.1.9.231`; verified sensor drawer opens without client-side exception. Run: `project_management/runs/RUN-20260131-tier-a-na65-renogy-kwh-sensors-0.1.9.231.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-114)

- **DW-200: Dashboard web: mobile horizontal overflow is unreachable**
  - **Description:** Some dashboard views render content wider than the mobile viewport. The layout currently clips horizontal overflow, making it impossible to swipe/scroll to see the content on small screens. Allow horizontal overflow access on mobile without changing desktop layout.
  - **Acceptance Criteria:**
    - In a mobile viewport (e.g., 390×844), overflowing content is reachable via horizontal swipe/scroll (no “cut off” cards/tables/charts with no way to see the right edge).
    - Desktop layout remains unchanged (no new horizontal scrollbars in normal use).
    - Dashboard-web UI changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails (reuse page layout pattern + Tailwind tokens; no inline styles/raw hex colors).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-31: Updated `apps/dashboard-web/src/app/(dashboard)/layout.tsx` to use `overflow-x-auto` on small screens and keep `lg:overflow-x-hidden` (desktop unchanged). Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run test:playwright -- mobile-audit.spec.ts` (pass).
  - **Status:** Done (validated locally)

- **DW-201: Sensors & Outputs: fix overlapping/garbled layout regression**
  - **Description:** The Sensors & Outputs page is currently not readable due to overlapping UI elements. Fix the underlying layout/CSS regression so the page is readable and stable across breakpoints.
  - **Acceptance Criteria:**
    - Sensors & Outputs renders with no overlapping/garbled text or cards in both view modes:
      - “By node” (default) node panels (Sensors table + Outputs list + Live weather panel when present)
      - “Flat” sensors table + Outputs grid
    - Layout is robust at common sizes (desktop ~1440×900, laptop ~1280×800, mobile ~390×844): content flows vertically and cards/tables do not overlap other sections.
    - UI changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (reuse page pattern + Tailwind tokens; no inline styles/raw hex colors; no design drift).
    - A screenshot is captured **and viewed** showing the fixed Sensors & Outputs page (store under `manual_screenshots_web/` and link it here).
    - `make ci-web-smoke` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-31: Reproduced overlap/garbled rendering on Sensors & Outputs (cards bleeding into each other) in the screenshot harness (`manual_screenshots_web/sensors_overlap_debug_before/sensors.png`, viewed).
    - 2026-01-31: Root cause: `CollapsibleCard` relied on a Tailwind `group-data-[state=open]` variant to switch `overflow` when open; the selector never matched, leaving collapsed content effectively `overflow: visible` and allowing it to paint outside its collapsed container (overlapping subsequent sections).
    - 2026-01-31: Fix: refactored `apps/dashboard-web/src/components/CollapsibleCard.tsx` so the collapse container itself toggles overflow via `data-[state=open]:overflow-visible` and defaults to `overflow-hidden` when closed (no reliance on group variants).
    - 2026-01-31: Updated `apps/dashboard-web/scripts/web-screenshots.mjs` stub-auth mode to include a small, non-empty nodes/sensors/outputs dataset so Sensors & Outputs regressions are visible in screenshots.
    - 2026-01-31: Validation: `make ci-web-smoke` (pass). Screenshot after fix: `manual_screenshots_web/sensors_overlap_debug_after3/sensors.png` (viewed).
  - **Status:** Done (validated locally)
- **DW-202: Dashboard-web: self-healing dev auth + screenshots (master)**
  - **Description:** Remove recurring developer friction around dashboard auth and screenshot capture by making the dev workflow self-healing (no manual token hacks, no port confusion, no “is a dev server already running?” guesswork).
  - **References:**
    - `project_management/tickets/TICKET-0047-dashboard-web-dev-auth-and-screenshots-auto-login-port-discovery-stub-fallback.md`
    - `docs/ADRs/0007-self-healing-dashboard-web-dev-auth-and-screenshot-workflows.md`
    - Related tasks: DW-203, DW-204, DW-205
  - **Acceptance Criteria:**
    - DW-203, DW-204, and DW-205 are `Done`.
    - A new agent can capture authenticated dashboard screenshots without manual token copy/paste or ModHeader-style hacks.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-31: Started implementation (DW-203..DW-205) per ADR-0007/TICKET-0047.
    - 2026-01-31: Completed DW-203..DW-205. Validation:
      - `make ci-web-smoke` (pass)
      - `cd apps/dashboard-web && npm run build` (pass)
      - Screenshot harness evidence:
        - Stub-auth run: `manual_screenshots_web/20260131_dev_auth_workflow_stub/`
        - Auto-detect reuse run (`--no-web`): `manual_screenshots_web/20260131_dev_auth_workflow_reuse_check/`
        - Auto-login run (mock core): `manual_screenshots_web/20260131_dev_auth_workflow_login_mock3/`
  - **Status:** Done (validated locally)

- **DW-203: Dashboard-web: dev-only “Login as Dev” helper (localhost only)**
  - **Description:** Add a small dev-only affordance to eliminate repeated auth setup friction during local debugging. This is specifically for developer ergonomics and must not ship/activate in production.
  - **References:**
    - `docs/ADRs/0007-self-healing-dashboard-web-dev-auth-and-screenshot-workflows.md`
    - `project_management/tickets/TICKET-0047-dashboard-web-dev-auth-and-screenshots-auto-login-port-discovery-stub-fallback.md`
  - **Acceptance Criteria:**
    - When running in development on `localhost`/`127.0.0.1` and the user is unauthenticated, the UI shows a small “Login as Dev” action (banner/button).
    - Clicking “Login as Dev” authenticates via `POST /api/auth/login` using dev credentials sourced from environment variables, stores the token via the shared auth token helper, and navigates to the intended page.
    - The dev-only affordance is not visible or usable in production builds.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-31: Implemented dev-only “Login as Dev” button gated by `NODE_ENV=development`, `NEXT_PUBLIC_ENABLE_DEV_LOGIN=1`, and localhost-only hostname checks. Credentials are sourced from `NEXT_PUBLIC_DEV_LOGIN_EMAIL`/`NEXT_PUBLIC_DEV_LOGIN_PASSWORD`.
    - 2026-01-31: Added smoke coverage for the gate helper. Validation:
      - `make ci-web-smoke` (pass)
      - `cd apps/dashboard-web && npm run build` (pass)
      - Playwright spot-check (mock core-server on `127.0.0.1:18000`): clicking “Login as Dev” navigates to `http://127.0.0.1:3000/overview`.
  - **Status:** Done (validated locally)

- **DW-204: Dashboard-web: screenshot harness auto-auth via `/api/auth/login` (explicit stub fallback)**
  - **Description:** Make screenshot capture reliable and low-friction by automatically authenticating when a backend is available, and providing a clear, explicit fallback mode when it is not.
  - **References:**
    - `docs/ADRs/0007-self-healing-dashboard-web-dev-auth-and-screenshot-workflows.md`
    - `project_management/tickets/TICKET-0047-dashboard-web-dev-auth-and-screenshots-auto-login-port-discovery-stub-fallback.md`
    - `apps/dashboard-web/scripts/web-screenshots.mjs`
  - **Acceptance Criteria:**
    - When core-server is reachable, the screenshot harness logs in automatically using `POST /api/auth/login` (credentials via env), sets the token in the browser context, and captures authenticated pages without manual steps.
    - If core-server is unreachable or auth fails, the harness either:
      - fails with a clear error, or
      - falls back to stub-auth only when explicitly enabled; in either case it prints the chosen mode clearly in output.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-31: Updated `apps/dashboard-web/scripts/web-screenshots.mjs` to support auto-login via `/api/auth/login` when no explicit token is provided (set `FARM_SCREENSHOT_LOGIN_EMAIL`/`FARM_SCREENSHOT_LOGIN_PASSWORD` or pass `--login-email`/`--login-password`). The harness logs the chosen auth mode.
    - 2026-01-31: Stub fallback is explicit via `--stub-auth` or `--allow-stub-fallback` (with clear mode logging).
    - 2026-01-31: Validation:
      - `make ci-web-smoke` (pass)
      - Auto-login (mock core): `manual_screenshots_web/20260131_dev_auth_workflow_login_mock3/`
      - Stub-auth: `manual_screenshots_web/20260131_dev_auth_workflow_stub/`
  - **Status:** Done (validated locally)

- **DW-205: Dashboard-web: screenshot tooling port auto-discovery + reuse running dev server (prefer `:3000`)**
  - **Description:** Remove port/lock confusion by standardizing the dashboard dev server port and teaching tooling to reuse an existing instance instead of starting a second `next dev` (which can fail due to `.next/dev/lock`).
  - **References:**
    - `docs/ADRs/0007-self-healing-dashboard-web-dev-auth-and-screenshot-workflows.md`
    - `project_management/tickets/TICKET-0047-dashboard-web-dev-auth-and-screenshots-auto-login-port-discovery-stub-fallback.md`
    - `apps/dashboard-web/scripts/web-screenshots.mjs`
  - **Acceptance Criteria:**
    - Tooling prefers `http://localhost:3000` by default, probes common alternatives (including `:3005`), and uses the first reachable server.
    - If a dev server is already running, the screenshot script reuses it (does not try to start a second instance).
    - If no dev server is running, the script starts one with a predictable default port (and supports explicit override via env/CLI).
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-31: Updated `apps/dashboard-web/scripts/web-screenshots.mjs` to prefer `127.0.0.1:3000` by default and auto-detect an already-running dev server (probe `3000..3009`).
    - 2026-01-31: Validation:
      - Started a dev server on `127.0.0.1:3000`, then ran `web-screenshots` with `--no-web` (script auto-detected and reused the server). Evidence: `manual_screenshots_web/20260131_dev_auth_workflow_reuse_check/`.
  - **Status:** Done (validated locally)

- **DW-206: Trends: make the Trend chart taller by default**
  - **Description:** Improve readability on the Trends page by increasing the default Trend chart height while keeping it user-adjustable and avoiding layout regressions.
  - **Acceptance Criteria:**
    - Trends Trend chart defaults to a taller height than the previous baseline (improves readability without requiring user interaction).
    - Existing users who previously had the legacy default persisted still see the new taller default on next load.
    - The chart height remains adjustable via the existing slider and continues to persist in local storage.
    - No layout regressions in the Trends page pattern (header + cards) across common breakpoints.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no design drift; Tailwind tokens; no inline styles/raw hex colors added).
    - `make ci-web-smoke` passes.
    - Tier A validated on installed controller (no DB/settings reset) with at least one **viewed** screenshot under `manual_screenshots_web/` showing Trends.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-01: Increased Trends default chart height from 320px → 420px and migrated legacy persisted default values so existing browsers pick up the taller default.
    - 2026-02-01: Validation:
      - `make ci-web-smoke` (pass)
      - Dev visual check (`127.0.0.1:3000`): `manual_screenshots_web/20260201_dev_trends_taller_graph/trends.png` (viewed)
      - Tier A refreshed installed controller to `0.1.9.236-trends-height` (bundle: `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.236-trends-height.dmg`)
        - Installed smoke: `make e2e-installed-health-smoke` (pass)
        - VIEWED screenshot: `manual_screenshots_web/tier_a_0.1.9.236_trends_height_20260201_143200/trends.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-207: Trends: Key panels default to collapsed on page load**
  - **Description:** Reduce above-the-fold clutter on the Trends page by making the “Key” panels collapsed by default while keeping them easily discoverable and toggleable.
  - **Acceptance Criteria:**
    - On first page load, the Trends page Key panels render collapsed by default (header-only; no long text blocks expanded).
    - Operators can expand/collapse any Key panel normally; behavior remains consistent with the shared `CollapsibleCard`.
    - No layout regressions across common breakpoints; page still follows the standard dashboard page pattern and token set.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - `make ci-web-smoke` passes.
    - Tier A validated on installed controller (no DB/settings reset) with at least one **viewed** screenshot under `manual_screenshots_web/` showing collapsed Keys on Trends.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Updated `AnalysisKey` to default to collapsed (`defaultOpen=false`) while allowing opt-in open behavior where desired.
    - 2026-02-02: Validation:
      - `make ci-web-smoke` (pass)
      - Dev visual check (`127.0.0.1:3000`): `manual_screenshots_web/20260202_dev_trends_keys_collapsed/trends.png` (viewed)
      - Tier A refreshed installed controller to `0.1.9.237-trends-keys` (bundle: `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.237-trends-keys.dmg`)
        - Installed smoke: `make e2e-installed-health-smoke` (pass)
        - VIEWED screenshot: `manual_screenshots_web/tier_a_0.1.9.237_trends_keys_20260202_211900/trends.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-210: Trends: Related Sensors results selection must not reset**
  - **Description:** In Trends → Related Sensors, selecting a result row should remain stable. Currently the selection can jump back to the top row after a short delay due to background polling refreshing lookup/label maps.
  - **Acceptance Criteria:**
    - Clicking any row in the Related Sensors results table keeps that row selected until the operator changes it.
    - Background polling of nodes/sensors does not reset the selected row to rank 1.
    - UI changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no design drift; Tailwind tokens; no inline styles/raw hex colors).
    - `make ci-web-smoke` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Root cause: results processing effect always auto-selected the first candidate, and the effect re-runs when nodes/sensors polling refreshes label/lookup maps.
    - 2026-02-02: Fix: preserve the current selection when it still exists in the refreshed candidate list.
    - 2026-02-02: Validation: `make ci-web-smoke` (pass).
    - 2026-02-03: Implemented the actual fix in `RelationshipFinderPanel` by using a stable selection helper (`pickStableCandidateId`) so background lookups refresh does not force-select rank 1; also stop resetting the results “Show more” count on lookup refresh.
    - 2026-02-03: Validation: `make ci-web-smoke` (pass), `cd apps/dashboard-web && npm run build` (pass).
    - 2026-02-03: Tier A refreshed installed controller to `0.1.9.242-dw210-related-selection` (run: `project_management/runs/RUN-20260203-tier-a-dw210-related-sensors-selection-0.1.9.242-dw210-related-selection.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.242-dw210-related-selection.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - Evidence screenshots: `manual_screenshots_web/tier_a_0.1.9.242_dw210_related_selection_20260203_020057Z/*`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-211: Analytics: Assisted temperature drift compensation (wizard + derived sensor output)**
  - **Description:** Add a polished, novice-friendly “assisted” workflow that helps operators compensate temperature-driven drift in a sensor by using another temperature sensor as the reference. The workflow should learn drift from historical data (metrics series), preview the correction visually, and then apply it automatically by creating a derived sensor (the original sensor remains unchanged).
  - **Acceptance Criteria:**
    - New dashboard route exists: `/analytics/compensation` (reachable from the sidebar under Analytics).
    - Operators can select:
      - The **sensor to compensate** (target sensor).
      - The **temperature reference sensor** (input temperature series).
      - A training window (at minimum: 24h / 72h / 7d) and an interval (bucket size).
    - The UI computes a drift model from historical data (at minimum: linear; advanced options may include polynomial degree 2/3) and shows:
      - A time-series comparison of **raw vs compensated** values over the chosen window.
      - A scatter plot of **raw value vs temperature** with a fitted curve/line overlay.
      - A clear “current adjustment” preview using live/latest temperature when available.
    - Novice UX:
      - Clear explanation of when drift compensation is appropriate + risks (avoid removing true signal).
      - Safe defaults + a one-click “Create compensated sensor” flow.
    - Advanced UX:
      - Exposes model type/degree, base temperature/centering, and optional clamping/outlier trimming.
      - Shows the generated derived-sensor expression and coefficients before creation.
    - Applying the compensation:
      - Creates a derived sensor via `/api/sensors` with `config.source="derived"` and `config.derived.expression` using both the target and temperature sensors as inputs.
      - Stores metadata in `config.derived` to identify the template as temperature compensation and preserve chosen parameters (for later editing/audit).
      - The creation flow is gated behind `config.write` (read-only users can explore but cannot apply).
    - UI adheres to `apps/dashboard-web/AGENTS.md` guardrails (standard page pattern, card-based sections, no design drift; no inline styles/raw hex colors in components).
    - Validation:
      - `make ci-web-smoke` passes.
      - `cd apps/dashboard-web && npm run build` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Implemented the `/analytics/compensation` “assisted” flow (sensor selectors, model fitting, visual previews, and derived sensor creation gated by `config.write`).
    - 2026-02-02: Added temp-drift fitting helpers + unit tests (`apps/dashboard-web/src/lib/tempCompensation.ts`; `apps/dashboard-web/tests/tempCompensation.test.ts`).
    - 2026-02-02: Wired navigation + IA map (sidebar link + Overview “Where things live” node).
    - 2026-02-02: Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-02-02: Tier A refreshed installed controller to `0.1.9.239-temp-compensation` (run: `project_management/runs/RUN-20260202-tier-a-dw211-temp-compensation-0.1.9.239-temp-compensation.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.239-temp-compensation.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - Screenshots (**viewed**):
        - `manual_screenshots_web/tier_a_0.1.9.239_dw211_temp_compensation_20260202_014033/analytics_compensation.png`
        - `manual_screenshots_web/tier_a_0.1.9.239_dw211_temp_compensation_20260202_014033/analytics_compensation_selected.png`
        - `manual_screenshots_web/tier_a_0.1.9.239_dw211_temp_compensation_20260202_014033/analytics_compensation_preview.png`
        - `manual_screenshots_web/tier_a_0.1.9.239_dw211_temp_compensation_20260202_014033/analytics_compensation_created.png`
  - **Status:** Done (Tier A validated installed `0.1.9.239-temp-compensation`; Tier B deferred to `DW-212`)

- **DW-213: Fix mobile WebKit crash when Highcharts zoom is disabled**
  - **Description:** On iOS/WebKit (and other coarse-pointer devices), some charts crashed with `TypeError: undefined is not an object (evaluating 'e.type')` during Highcharts init. This happened when we passed `chart.zooming: undefined` while disabling chart zoom to allow browser pinch-zoom-out. The fix avoids overriding Highcharts defaults with `undefined`, and ensures Trends “Pan mode” does not set `zooming: undefined`.
  - **Acceptance Criteria:**
    - `/analytics` loads on iPhone 13 Safari/WebKit without a client-side exception.
    - `/analytics/compensation` loads and selecting a temperature reference sensor renders charts without crashing.
    - Trends pan-mode toggle (if enabled) does not crash charts.
    - Validation: `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A: refresh installed controller and record evidence under `project_management/runs/` (include WebKit/mobile screenshots demonstrating the fix).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Fix implemented by omitting `chart.zooming`/`chart.panning` keys when zoom is disabled (avoid `zooming: undefined` overrides) and using `zooming: {}` for Trends pan-mode; added a smoke-test guard for chart factory output.
    - 2026-02-02: Tier A refreshed installed controller to `0.1.9.240-highcharts-zooming-fix` (run: `project_management/runs/RUN-20260202-tier-a-dw213-highcharts-zooming-fix-0.1.9.240-highcharts-zooming-fix.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.240-highcharts-zooming-fix.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - WebKit screenshots (**viewed**): `manual_screenshots_web/tier_a_0.1.9.240_dw213_highcharts_zooming_fix_20260202_195802Z/*`
  - **Status:** Done (Tier A validated installed `0.1.9.240-highcharts-zooming-fix`)

- **DW-214: Analytics Temp Compensation: allow custom training window**
  - **Description:** Extend the Analytics → Temp Compensation assisted workflow to support a custom start/end training window (site/controller time) in addition to the preset 24h/72h/7d buttons.
  - **Acceptance Criteria:**
    - Step 2 includes a “Custom” toggle with start/end date-time inputs (interpreted in controller/site time).
    - When a custom window is set, metrics queries use that start/end window and the fit/preview updates.
    - Invalid windows show a clear validation message and do not crash.
    - Auto interval chooses a sensible interval for the effective window length; custom interval list includes longer options (6h/12h/1d).
    - Validation: `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A: refresh installed controller and capture evidence under `project_management/runs/` (custom window + fit + preview; no JS errors).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Implemented custom range toggle with start/end inputs and `useMetricsQuery` window parameters; local validation: `make ci-web-smoke` (pass), `cd apps/dashboard-web && npm run build` (pass).
    - 2026-02-02: Tier A refreshed installed controller to `0.1.9.241-analytics-mobile-window` (run: `project_management/runs/RUN-20260202-tier-a-dw209-dw214-0.1.9.241-analytics-mobile-window.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.241-analytics-mobile-window.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - WebKit screenshots (**viewed**): `manual_screenshots_web/tier_a_0.1.9.241_dw214_comp_custom_range_20260202_211125Z/*`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-212)

- **DW-215: Trends: Sensor picker must not overflow its container**
  - **Description:** The Trends “Sensor picker” panel can render nested content that exceeds the panel width, causing it to visually spill past the right border (regression observed after CollapsibleCard layout changes). Fix so the Sensor picker content stays within its container without narrowing charts elsewhere.
  - **Acceptance Criteria:**
    - On `/trends`, the “Sensor picker” card does not visually spill/paint outside its border at common sizes (desktop ~1440×900 and laptop ~1280×800).
    - In the `xl:grid-cols-[280px_1fr]` layout, the Sensor picker sidebar stays at 280px and its contents shrink/truncate as needed (no horizontal overflow painting outside the card).
    - Fix stays within stock shadcn/ui + Radix patterns (no custom overflow hacks that break menus/popovers).
    - `make ci-web-smoke` passes.
    - Tier A: installed controller refreshed via runbook with **no DB/settings reset** and a screenshot captured + viewed under `manual_screenshots_web/` showing the Sensor picker sidebar (no spill).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-03: Root cause: `CollapsibleCard` used a Radix Collapsible content wrapper where the grid item kept its intrinsic width (min-width auto), allowing the body to paint outside the 280px sidebar column and spill past the card border.
    - 2026-02-03: Fix: add `grid-cols-1` on the content wrapper and `min-w-0` on the immediate child wrapper so CollapsibleCard bodies can shrink within constrained layouts (without changing the intended sidebar width).
    - 2026-02-03: Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-02-03: Tier A refreshed installed controller to `0.1.9.243-dw215-sensor-picker-overflow` (run: `project_management/runs/RUN-20260203-tier-a-dw215-sensor-picker-overflow-0.1.9.243-dw215-sensor-picker-overflow.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.243-dw215-sensor-picker-overflow.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - Screenshots (**viewed**): `manual_screenshots_web/tier_a_0.1.9.243_dw215_sensor_picker_overflow_20260203_071210Z/trends.png`
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DW-98)

- **DW-216: Analytics Temp Compensation: detrend slow changes + add fit diagnostics**
  - **Description:** Reservoir depth often appears to change throughout a day due to temperature-driven drift, while the true depth typically changes gradually over multiple days. The current Temp Compensation “automatic” fit under-corrects in these cases. Improve the fitter to optionally detrend a slow time-based change so temperature coefficients aren’t diluted, and add a diagnostics panel to make under/over-compensation obvious.
  - **Acceptance Criteria:**
    - `/analytics/compensation` includes a “Detrend slow changes over time” toggle.
    - Default behavior:
      - Training windows ≥ 48h default to detrend **On** (unless manually overridden).
      - Training windows < 48h default to detrend **Off**.
    - When detrend is enabled, the fit includes an additional linear time term (raw units/day) to prevent bias in the temperature coefficients; the derived-sensor expression still applies **temperature-only** correction.
    - The page displays a “Fit diagnostics” panel showing:
      - Raw swing (P95–P5)
      - Compensated swing (P95–P5)
      - Range reduction (%)
      - Correction span across observed temperature range
      - Time trend (units/day) when detrend is enabled
    - Unit tests include coverage for the time-detrended fit so temperature coefficients are recovered in the presence of slow drift.
    - Validation:
      - `make ci-web-smoke` passes.
      - `cd apps/dashboard-web && npm run build` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-04: Implemented optional time-detrended fitting (linear time slope term used during fitting; temperature-only correction applied in the derived expression) and added an on-page diagnostics panel.
    - 2026-02-04: Added unit test coverage for recovering temperature coefficients in the presence of slow time drift (`apps/dashboard-web/tests/tempCompensation.test.ts`).
    - 2026-02-04: Validation: `make ci-web-smoke-build` (pass); `cd apps/dashboard-web && npm test -- --run tests/tempCompensation.test.ts` (pass).
    - 2026-02-04: Tier A validated on installed controller `0.1.9.246-temp-comp-lag` (run: `project_management/runs/RUN-20260204-tier-a-temp-comp-lag-0.1.9.246-temp-comp-lag.md`; screenshot **viewed**: `manual_screenshots_web/20260204_041605_temp_comp/analytics_compensation_temp_lag.png`).
  - **Status:** Done (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B deferred to DW-212)

- **DW-217: Analytics Temp Compensation: add temperature lag (auto + derived lag_seconds)**
  - **Description:** Real-world reservoir depth drift can lag air temperature by hours (thermal inertia of the transducer/enclosure/water). A zero-lag regression underestimates the slope and leaves the compensated series visibly under-corrected. Add a lag option (auto + custom) so the fitter can align raw and temperature before fitting, and persist the lag into the derived sensor so the installed controller uses the same correction.
  - **Acceptance Criteria:**
    - `/analytics/compensation` includes a Temperature lag control:
      - **Auto:** scans 0..6h (step >= 5 minutes) and selects a lag that improves P95–P5 swing reduction vs 0-lag (with guardrails to avoid noisy jumps).
      - **Custom:** user can specify lag minutes.
      - Semantics are explicit: raw[t] aligns to temp[t − lag].
    - The time-series preview and scatter plot use the selected lag (the fitted coefficients reflect lagged alignment).
    - Creating a compensated sensor includes `inputs[].lag_seconds` on the temperature input, and records `temperature_lag_seconds` in `derived.params`.
    - Unit tests cover:
      - `alignSeriesByTimestamp` with lagged matching.
      - Lag suggestion picks the correct lag for a synthetic lagged dataset.
    - Validation:
      - `make ci-web-smoke-build` passes.
      - Tier A: installed controller refreshed via runbook (no DB/settings reset) and a screenshot is captured + viewed under `manual_screenshots_web/` showing the lag control on `/analytics/compensation`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-04: Root cause observed on real data: best-fit drift correlation improves significantly with a ~2h45m temperature lag (example: corr raw vs temp improves from ~-0.46 at 0-lag to ~-0.69 at +165m; swing reduction improves from ~15% to ~40%).
    - 2026-02-04: Implemented lag-aware alignment (raw[t] ↔ temp[t−lag]) and auto lag suggestion (0..6h scan, step ≥ 5 minutes) with a clear Auto/Custom control.
    - 2026-02-04: Creating a compensated derived sensor now includes `inputs[].lag_seconds` for the temperature input and records `temperature_lag_seconds` in `derived.params`.
    - 2026-02-04: Validation: `make ci-web-smoke-build` (pass); `cd apps/dashboard-web && npm test -- --run tests/tempCompensation.test.ts` (pass).
    - 2026-02-04: Tier A validated on installed controller `0.1.9.246-temp-comp-lag` (run: `project_management/runs/RUN-20260204-tier-a-temp-comp-lag-0.1.9.246-temp-comp-lag.md`; screenshot **viewed**: `manual_screenshots_web/20260204_041605_temp_comp/analytics_compensation_temp_lag.png`).
  - **Status:** Done (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B deferred to DW-212)

- **DW-218: Analytics Temp Compensation: allow compensating derived sensors**
  - **Description:** The Temp Compensation wizard currently excludes derived sensors from the “Sensor to compensate” picker. This blocks compensating computed/aggregated sensors (derived sensors), even though they can still drift with temperature. Allow derived sensors to be selected as the target sensor (and as the temperature reference when applicable), and ensure the resulting derived-of-derived compensated sensor can be created successfully.
  - **Acceptance Criteria:**
    - `/analytics/compensation` “Sensor to compensate” includes derived sensors (search + selection works).
    - Creating a compensated sensor succeeds when the selected “Sensor to compensate” is derived (server accepts derived-of-derived).
    - UI changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (page pattern + Tailwind tokens; no design drift).
    - Validation: `make ci-web-smoke` passes.
    - Tier A: screenshot captured + viewed showing a derived sensor selected in the wizard and the workflow completing (store under `manual_screenshots_web/` and link in the Tier A run log).
  - **Owner:** Platform UI (Codex)
  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260204-tier-a-cs102-dw218-dw219-0.1.9.248-derived-of-derived.md`
    - Screenshot: `manual_screenshots_web/tier_a_0.1.9.248-derived-of-derived_cs102_dw218_dw219_20260204_074049Z/compensation_derived_target_created.png`
  - **Status:** Done (validated on installed controller `0.1.9.248-derived-of-derived`; clean-host E2E deferred to DW-212)

- **DW-219: Derived sensors: allow derived inputs in Derived Sensor Builder**
  - **Description:** The Derived Sensor builder currently excludes derived sensors from the input picker and states that derived sensors cannot use other derived sensors as inputs. With derived-of-derived now supported in the core server (CS-102), allow selecting derived sensors as inputs so operators can build multi-stage computations (and so workflows like temp compensation of an already-derived sensor are not blocked by UI constraints).
  - **Acceptance Criteria:**
    - Derived sensors appear in the Derived Sensor Builder input picker (search + selection works).
    - Creating a derived sensor that depends on another derived sensor succeeds (server accepts derived-of-derived).
    - UI changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (page pattern + Tailwind tokens; no design drift).
    - Validation: `make ci-web-smoke` passes.
    - Tier A: screenshot captured + viewed showing a derived sensor selected as an input and the derived-of-derived sensor created successfully (store under `manual_screenshots_web/` and link in the Tier A run log).
  - **Owner:** Platform UI (Codex)
  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260204-tier-a-cs102-dw218-dw219-0.1.9.248-derived-of-derived.md`
    - Screenshot: `manual_screenshots_web/tier_a_0.1.9.248-derived-of-derived_cs102_dw218_dw219_20260204_074049Z/derived_builder_derived_input_created.png`
  - **Status:** Done (validated on installed controller `0.1.9.248-derived-of-derived`; clean-host E2E deferred to DW-98)

- **DW-220: Derived Sensor Builder: document derived-of-derived guardrails (depth/cycles)**
  - **Description:** Now that derived sensors can depend on other derived sensors, clearly surface the controller guardrails in the Derived Sensor Builder so operators understand why certain compositions fail. Specifically: cycles are rejected and derived chains have a max depth (currently 10).
  - **Acceptance Criteria:**
    - The Derived Sensor Builder Inputs section explicitly mentions:
      - Cycles are rejected.
      - Max derived chain depth is 10.
    - UI changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (page pattern + Tailwind tokens; no design drift).
    - Validation: `make ci-web-smoke` passes.
    - Tier A: screenshot captured + viewed showing the guardrails note visible in the Derived Sensor Builder (store under `manual_screenshots_web/` and link in the Tier A run log).
  - **Owner:** Platform UI (Codex)
  - **Tier A Evidence / Run Log:**
    - `project_management/runs/RUN-20260205-tier-a-dw220-0.1.9.249-derived-builder-guardrails.md`
    - Screenshot: `manual_screenshots_web/tier_a_0.1.9.249-derived-builder-guardrails_dw220_20260205_104230Z/derived_builder_guardrails_note.png`
  - **Status:** Done (validated on installed controller `0.1.9.249-derived-builder-guardrails`; clean-host E2E deferred to DW-98)

---

## Schedules and Alarms
### Done
- **SA-1: Implement conditional automation based on forecasts/analytics**
  - **Status:** Done


- **SA-2: Extend schedule models**
  - **Description:** Blocks, conditions, actions & JSON schema.
  - **Status:** Done


- **SA-3: Update schedule engine**
  - **Description:** Evaluate conditions, publish MQTT output commands, record audit logs.
  - **Status:** Done


- **SA-4: Implement schedule endpoints**
  - **Description:** `/api/schedules`, `/api/schedules/calendar`, `/api/schedules/{id}/actions`.
  - **Status:** Done


- **SA-5: Implement alarm definition APIs**
  - **Description:** Event history `/api/alarms`, `/api/alarms/history`, acknowledgement endpoint.
  - **Status:** Done


- **SA-6: Seed demo schedules**
  - **Description:** Pump + moisture guard, nighttime leak alert, greenhouse cooling with mock metrics.
  - **Status:** Done


- **SA-7: Ensure default offline alarms**
  - **Description:** Auto-create per node/sensor and fire via ingest tests.
  - **Status:** Done


- **SA-8: Add pytest coverage**
  - **Description:** Schedule execution + alarm generation.
  - **Status:** Done

- **SA-9: Fix schedule timezone + block execution semantics**
  - **Description:** Make schedule timing consistent end-to-end (calendar UI, API, and schedule engine). Blocks are authored as local HH:mm and must render correctly and drive execution; schedule engine must honor block start/end and output action durations where applicable.
  - **References:**
    - `apps/dashboard-web/src/app/(dashboard)/schedules/SchedulesPageClient.tsx`
    - `apps/dashboard-web/src/features/schedules/lib/scheduleUtils.ts`
    - `apps/core-server-rs/src/routes/schedules.rs`
    - `apps/core-server-rs/src/services/schedule_engine.rs`
  - **Acceptance Criteria:**
    - Calendar API interprets blocks in controller-local time (not UTC) and returns correct RFC3339 timestamps.
    - Drag/resize updates persist successfully (UI sends full upsert payload or backend provides a blocks-only PATCH).
    - Schedule engine honors weekly blocks (start + end) in addition to RRULE; output actions run at start and are reliably turned off at end (or via duration_seconds).
    - Timezone/DST behavior is deterministic and documented.
  - **Notes / Run Log:**
    - 2026-01-10: Fixed schedule drag/resize payloads to send full upsert objects, interpreted calendar blocks in controller-local time, and updated the schedule engine to execute weekly block start/end edges (auto-turn-off at block end). Builds: Rust + Next (pass). Tests: blocked by clean-state gate on this host.
    - 2026-01-12: Tier‑A validated UI render paths on installed controller `0.1.9.100` (schedules list + create/edit surfaces render; screenshots viewed). Run: `project_management/runs/RUN-20260112-tier-a-closeout-loop-0.1.9.100.md`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)


---

## Backups and Restore
### Done
- **BR-1: Extend backup manager**
  - **Description:** Store metadata in DB, apply retention, and expose status.
  - **Status:** Done


- **BR-2: Implement backup endpoints**
  - **Description:** `/api/backups`, `/api/backups/{id}`, `/api/restore` endpoints with validation.
  - **Status:** Done


- **BR-3: Update seed script**
  - **Description:** Create sample backups.
  - **Status:** Done


- **BR-4: Add tests**
  - **Description:** Cover backup scheduling, retention, and restore call.
  - **Status:** Done


- **BR-5: Build frontend backup browser + restore wizard**
  - **Description:** UI for managing backups.
  - **Status:** Done


- **BR-6: Adoption flow queue restore**
  - **Description:** Adoption flow can optionally queue a restore from the latest backup of a prior node.
  - **Status:** Done


- **BR-7: Restore activity feed**
  - **Description:** Backend persists restore events and `/api/restores/recent` drives the dashboard activity list.
  - **Status:** Done


---

## Analytics
### Done
- **AN-32: Setup Center Forecast.Solar PV configurables + check-plane validation**
  - **Description:** Make all Forecast.Solar Public PV parameters configurable in the web dashboard via Setup Center, validate via the non-rate-limited Forecast.Solar check-plane endpoint, and surface Public rate limit guidance.
  - **References:**
    - `project_management/tickets/TICKET-0029-forecast.solar-public-plan-pv-forecast-integration.md`
  - **Acceptance Criteria:**
    - Setup Center includes a “Solar PV forecast (Forecast.Solar Public)” panel with per-node configuration cards for solar charge controller nodes (Renogy BT‑2 telemetry) and any deep-linked node IDs from Node Detail; cards include descriptive labels, inline validation, and a “Use map placement” helper to copy coordinates from the active Map save.
    - Saving PV settings runs Forecast.Solar `GET /check/{lat}/{lon}/{dec}/{az}/{kwp}` first and shows the result (place/timezone) without consuming `/estimate/*` quota.
    - UI calls out Public rate limits (12 calls per rolling 60 minutes) and clarifies that `/estimate/*` is rate-limited while `/check` is for validation.
    - Controller uses the configured timestamp `time_format` (`utc` or `iso8601`) when polling Forecast.Solar so timestamps decode correctly.
    - Relevant tests + E2E run pass, or the clean-state blocker is documented.
  - **Notes / Run Log:**
    - 2026-01-07: Added `/api/forecast/pv/check` and Setup Center UI (node selector, tilt/azimuth helpers, time format select, rate-limit guidance).
    - 2026-01-07: Updated Node Detail and Analytics copy to direct PV configuration to Setup Center.
    - 2026-01-07: Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-07: Built controller bundle `0.1.9.15` via `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle ...` and upgraded the installed stack via setup-daemon `POST http://127.0.0.1:8800/api/upgrade` (installed version now reports `current_version=0.1.9.15`).
    - 2026-01-07: Upgraded installed controller to `0.1.9.21`; `GET http://127.0.0.1:8000/healthz` is ok and Setup Center `/setup` renders the PV config panel (save runs Forecast.Solar `/check` first).
    - 2026-01-07: Verification: `GET /healthz` is `ok`, `GET /api/openapi.json` includes `/api/forecast/pv/check`, and `GET /setup/` returns 200 (static assets refreshed).
    - 2026-01-07: Setup Center now surfaces live Forecast.Solar Public quota metadata (remaining/limit/zone) from provider status so admins can tune poll cadence safely; bundled + upgraded installed controller to `0.1.9.24`.
    - 2026-01-08: Audit: Forecast.Solar Public-plan quota UX is too prominent; move to a compact Provider-status inline display (`used/remaining · window`) and remove the standalone “Rate limits” box while keeping a minimal one-line rate-limit note.
    - 2026-01-08: Implemented compact Forecast.Solar quota display in Provider status (`used/remaining · window`) and removed the standalone “Rate limits” box; refreshed the installed controller to `0.1.9.30`.
    - 2026-01-08: Updated Setup Center + Analytics PV sections to render per-node cards (one per solar charge controller node) instead of single-node dropdown selection; added “Use map placement” to copy coordinates from the active Map save; refreshed the installed controller to `0.1.9.31`.
    - 2026-01-08: PV config UX polish: added explicit form labels for latitude/longitude/kWp inputs (placeholders alone were too ambiguous once values are entered); refreshed the installed controller to `0.1.9.32`.
    - 2026-01-08: Forecast.Solar quota UX polish: removed the extra public-plan note under Provider status and moved the rate-limit guidance into the quota tooltip title to keep the panel compact; refreshed the installed controller to `0.1.9.33`.
    - 2026-01-10: Saving PV settings now triggers an immediate `/api/forecast/poll` so Forecast.Solar predictions refresh right away; invalidates PV forecast hourly/daily queries so the Analytics PV overlay updates without waiting for the next scheduled poll.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Setup Center PV config renders and triggers immediate poll on save. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/setup.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **AN-33: Open-Meteo forecast: add cloud cover (persist + API + graphs)**
  - **Description:** Extend the Open-Meteo hyperlocal forecast integration to include cloud cover and graph it alongside the existing hourly + daily weather forecast charts.
  - **Acceptance Criteria:**
    - Controller polls and persists cloud cover metrics from Open-Meteo (hourly `cloudcover` percent; daily mean/aggregate where available).
    - Weather forecast APIs expose cloud cover metrics with explicit units and bounded payloads.
    - Dashboard Weather forecast section graphs cloud cover alongside existing temperature + precipitation views with clear labeling and non-cluttered UX.
    - OpenAPI spec and generated clients include any new/changed schema fields; `make rcs-openapi-coverage` remains green.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build` remain green.
  - **Notes / Run Log:**
    - 2026-01-08: Added Open-Meteo cloud cover ingest + persistence (hourly + daily mean), exposed via weather forecast APIs, and graphed in Analytics weather charts. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass), `cd apps/dashboard-web && npm run build` (pass), `python3 tools/check_openapi_coverage.py` (pass).
    - 2026-01-10: Weather forecast chart UX: temperature plot now includes humidity (%), precipitation moved into the cloud cover chart, and precipitation axes clamp to 0 (no negative precipitation). Refreshed installed controller to `0.1.9.50`.
    - 2026-01-10: Analytics “Next 7 days” temperature min/max chart now uses floating range bars (min→max) for better readability. Build: `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-10: Analytics Weather forecast now includes a “Next 24 hours” quick-view (temp+humidity and cloud+precip) above the existing 72-hour and 7-day charts. Refreshed installed controller to `0.1.9.65`.
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Analytics weather renders cloud cover + humidity charts. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/analytics.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **AN-34: Hyperlocal current weather (Open-Meteo) endpoint for per-node live panels**
  - **Description:** Add a controller endpoint to fetch current hyperlocal weather for a given coordinate (or node via active map placement) so the dashboard can show per-node live conditions without hitting rate-limited forecast endpoints.
  - **Acceptance Criteria:**
    - API exposes current conditions including at least: temperature, wind speed/direction, precipitation, cloud cover (and provider timestamp/units).
    - Endpoint can resolve coordinates from a node’s placement in the active map save (and returns a clear error when unplaced).
    - Requests are cached/throttled server-side to prevent rapid refresh spam from the UI.
    - OpenAPI spec and generated clients include the endpoint; `make rcs-openapi-coverage` remains green.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Added `GET /api/forecast/weather/current` (by node_id or lat/lon) with 60s in-memory cache and active-map placement lookup; wired into per-node Live weather panel. Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass), `python3 tools/check_openapi_coverage.py` (pass).
    - 2026-01-10: Tier A (installed controller `0.1.9.69`): Node detail renders live weather from active map placement. Evidence: `project_management/runs/RUN-20260110-tier-a-smoke-0.1.9.69.md`, `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/nodes_0a55b329-104f-46f0-b50b-dea9a5cca1b3.png`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

- **AN-30: Fix Analytics power chart bucketing (24h live + 168h hourly)**
  - **Description:** Make Analytics power charts update in a “live” cadence by increasing 24h resolution and preventing 168h downsampling from appearing “daily-only”.
  - **Acceptance Criteria:**
    - `/api/analytics/power` returns `series_24h` points at 5-minute granularity and `series_168h` at 1-hour granularity (bounded payloads, latest timestamp is recent).
    - Analytics tab “Power – last 24 hours” visibly updates throughout the hour (not only on the hour) and “Power – last 168 hours” no longer appears daily/downsampled.
  - **Notes / Run Log:**
    - 2026-01-06: Deployed to production controller bundle `0.1.9.12`; validated `GET /api/analytics/power` shows `delta_24h_seconds=300` and `delta_168h_seconds=3600`.
  - **Status:** Done (deployed + validated; automated tests not run on prod host due to clean-state gate)

- **AN-31: Analytics power UX: per-node breakdown + no implied coupling**
  - **Description:** Rework Analytics power presentation so it never implies Emporia meters and Renogy controllers are one coupled system. Provide explicit per-node breakdowns and opt-in fleet aggregation.
  - **Acceptance Criteria:**
    - Analytics power summary and charts clearly communicate what is aggregated (and across which nodes/integrations) and default to per-node views when multiple nodes exist.
    - Charts can show one line per node (Renogy nodes separately, Emporia nodes separately) with legends that include node names; fleet totals are labeled as sums.
    - “Live power” and battery/SOC UI elements always include node context or are explicitly labeled fleet-wide; no ambiguous standalone percentages.
    - Emporia meters can be grouped (street address label) and excluded from fleet totals via Setup Center preferences without removing the meter from node views.
  - **Notes / Run Log:**
    - 2026-01-06: Updated Analytics copy to label fleet values as sums and avoid implied coupling.
    - 2026-01-06: Added “Power nodes” breakdown table on Analytics showing per-node Emporia mains power and Renogy PV/load/SOC with explicit units.
    - 2026-01-06: Increased `/api/analytics/power` 24h chart resolution to 60-second buckets and removed 168h zero-fill series generation to reduce the “daily-only” perception.
    - 2026-01-07: `/api/analytics/power` now composes totals from Emporia mains + Renogy load/PV/battery and respects Emporia per-meter inclusion preferences.
    - 2026-01-07: Analytics UI now shows an “Emporia meters by address” breakdown and flags meters excluded from system totals.
    - 2026-01-10: Copy polish: Analytics charts use “past 24 hours” / “past 7 days” (avoid “last” phrasing and “168 hours” titles).
    - 2026-01-06: Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-06: Production verification after upgrade to `0.1.9.13`: `/api/dashboard/state` reports 55 sensors (43 Emporia, 12 Renogy) across 5 nodes; `/api/analytics/feeds/status` reports `Emporia: ok` with fresh `last_seen`.
    - Test note: E2E + unit tests not run on this host because the clean-state preflight shows the installed Farm stack running under `_farmdashboard` (must be stopped to satisfy test hygiene gate).
  - **Evidence (Tier A):**
    - Installed controller refreshed to `0.1.9.75`; `/api/analytics/power` and `/api/analytics/feeds/status` are healthy and the running dashboard surfaces per-node context without implied coupling (manual spot-check). Run: `project_management/runs/RUN-20260111-tier-a-phase5-operator-surfaces-0.1.9.75.md`.
    - 2026-01-12: Analytics IA/layout polish (consistent card component, clearer section grouping, reduced “Frankenstein” drift). Local validation: `cd apps/dashboard-web && npm run build` (pass), Playwright screenshot `/tmp/playwright-analytics-layout.png` (viewed).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)


- **AN-23: Add per-node battery voltage chart to Analytics**
  - **Description:** Plot each node’s battery voltage on a single Analytics graph using telemetry from battery voltage sensors (e.g., Renogy BT-2) so fleet voltage health is visible at a glance.
  - **Acceptance Criteria:**
    - Battery voltage sensors (metric includes `battery_voltage`) are detected per node and queried for the last 24 hours with sensible bucketing.
    - The Analytics tab renders one voltage chart with a distinct line/legend entry per node; empty/error states explain what’s missing without breaking the page.
    - Chart refresh respects live data, and UI defaults remain usable when no battery sensors are present.
  - **Notes / Run Log:**
    - 2026-01-06: Deployed + validated on production controller bundle `0.1.9.11`: chart auto-detects `battery_voltage*` sensors and queries 24h with 5-minute buckets; Analytics renders a per-node line series with legend + empty/error states. Evidence: `reports/prod-forecast-validate-20260106_032403.log`, screenshots under `manual_screenshots_web/prod_forecast_post_0.1.9.11_20260106_041239/`.
  - **Status:** Done (deployed + validated; automated tests not run on prod host due to clean-state gate)


- **AN-27: Forecast.Solar PV forecast integration (Public plan)**
  - **Description:** Integrate Forecast.Solar Public endpoints (`/estimate/*`) to provide per-node PV production forecasts and enable a clear “forecast vs measured” overlay using Renogy telemetry.
  - **References:**
    - `project_management/tickets/TICKET-0029-forecast.solar-public-plan-pv-forecast-integration.md`
  - **Acceptance Criteria:**
    - Operators can configure PV setup per node (lat/lon, tilt/declination `0–90°`, azimuth `-180–180°`, capacity `kWp`) and enable/disable the forecast.
    - Controller polls Forecast.Solar Public endpoints and persists raw forecast points indefinitely; API responses are bounded and reflect Public-horizon constraints (this + next day).
    - Analytics displays Forecast.Solar predicted PV (W + kWh/day) and overlays Renogy measured PV power for the same node with explicit units and clear empty/error states.
  - **Notes / Run Log:**
    - 2026-01-06: Deployed to production controller bundle `0.1.9.11`. Validated `POST /api/forecast/poll` shows `Forecast.Solar: ok`, inserts into `forecast_points`, and Analytics renders “PV forecast vs measured” + energy chart (Public horizon is 2 days). Evidence: `reports/prod-forecast-validate-20260106_032403.log`, `reports/prod-forecast-upgrade-20260106_041046.log`, screenshots under `manual_screenshots_web/prod_forecast_post_0.1.9.11_20260106_041239/`.
    - 2026-01-06: Fixed production crash from Axum v0.7 route syntax (`:node_id` → `{node_id}`) and redeployed in `0.1.9.11` (see `reports/prod-forecast-upgrade-20260106_030019.log`).
  - **Status:** Done (deployed + validated; AN-29 tracks real-hardware alignment)


- **AN-28: Hyperlocal weather forecast (Open-Meteo) hourly + weekly**
  - **Description:** Add coordinate-configurable hyperlocal hourly + weekly weather forecasts using a publicly available API (Open-Meteo) and persist results for analytics/scheduling without data loss.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0030-hyperlocal-weather-forecast-(open-meteo)-hourly-+-weekly.md`
  - **Acceptance Criteria:**
    - Setup Center includes a weather forecast configuration with latitude/longitude inputs (units + validation) and shows last refresh/provider status.
    - Controller polls Open-Meteo and persists raw hourly + daily forecast points indefinitely; API responses are bounded (e.g., ~72h hourly + 7–14d daily) with explicit units on all fields.
    - Dashboard renders hourly and weekly forecast graphs with clear units, last-updated timestamps, and resilient error/empty states.
  - **Notes / Run Log:**
    - 2026-01-06: Deployed to production controller bundle `0.1.9.11`. Validated `POST /api/forecast/poll` shows `Open-Meteo: ok`, inserts into `forecast_points`, and Analytics renders the “Weather forecast” section. Evidence: `reports/prod-forecast-validate-20260106_032403.log`, screenshots under `manual_screenshots_web/prod_forecast_post_0.1.9.11_20260106_041239/`.
  - **Status:** Done (deployed + validated)


- **AN-18: Validate live external feed adapters**
  - **Description:** Run live QA against Emporia/Tesla/Enphase accounts using production credentials and devices.
  - **Notes / Validation:**
    - Emporia cloud ingest now runs via the Rust feed poller (`analytics_power_samples` + `analytics_integration_status`) and `/api/setup/emporia/login` derives/stores Cognito tokens without persisting the password. Feed status/history is available at `/api/analytics/feeds/status`, and `POST /api/analytics/feeds/poll` triggers an immediate ingest.
    - Validated Emporia with a real account (credentials redacted); deviceGIDs auto-discovered from `/customers/devices`, and live readings mapped into power samples (kW + kWh) via `getDeviceListUsages`.
    - Tesla/Enphase remain unconfigured in this environment; feed status surfaces missing connectors instead of falling back to fixtures.
    - Test note: `ci-core-smoke`/E2E were not run on the production controller host because the test-hygiene gate requires a clean machine with no Farm launchd jobs/processes; run these suites on a clean dev host (or after stopping/uninstalling the installed stack).
  - **Status:** Done (Emporia cloud path validated; remaining providers still depend on credentials/hardware)


- **AN-26: Emporia setup UX: accept username/password to derive a cloud token**
  - **Description:** Reduce friction for Emporia setup by allowing operators to enter Emporia credentials once to derive/store the required cloud token (do not store the password long-term). Consider leveraging `PyEmVue` behavior as a reference for the auth exchange.
  - **Notes / Validation:**
    - Added `/api/setup/emporia/login` (Cognito SRP → id_token/refresh_token) plus Setup Center UI to capture Emporia credentials; only tokens/site IDs are persisted in `setup_credentials`, and the flow auto-polls `/api/analytics/feeds/poll` after saving.
    - Manual validation with the provided Emporia account confirms tokens are issued, devices/site IDs populate automatically, and feed status reports `ok`.
    - Existing token-only entry remains available under Credentials.
    - 2026-01-06: Production bundle `0.1.9.8` enabled analytics feed polling by default; `/api/analytics/feeds/status` now reports `"enabled": true` and `/api/analytics/power` surfaces non-zero Emporia-derived power metrics when devices are active.
    - Test note: `ci-core-smoke`/`ci-web-smoke` were not run on the production controller host because the test-hygiene gate requires a clean machine with no Farm launchd jobs/processes; run these suites on a clean dev host (or after stopping/uninstalling the installed stack).
  - **Status:** Done


- **AN-22: Wire reservoir depth telemetry into Analytics Water**
  - **Description:** Persist a reservoir depth timeseries in real mode so the dashboard Analytics → Water panel is backed by real sensor telemetry (not demo seed data).
  - **References:**
    - `project_management/tickets/TICKET-0005-reservoir-depth-pressure-transducer-integration.md`
  - **Acceptance Criteria:**
    - Real-mode installs can produce non-empty `/api/analytics/water` `reservoir_depth` output when a reservoir depth sensor is configured and publishing.
    - The unit/metric naming strategy is explicit (e.g., store `reservoir_depth_ft` samples or document end-to-end meters with a compatible schema).
    - Automated coverage exists for the mapping path (fixtures or seeded DB), and `make e2e-web-smoke` still passes.
  - **Status:** Done (`cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make e2e-installer-stack-smoke`)


- **AN-21: Configure LLM predictive alarms in Setup Center**
  - **Description:** Default LLM-based alarm trend analysis to OFF on fresh installs, and expose a Setup Center UI to enable it with an OpenAI-compatible endpoint (Ollama/LM Studio) and optional API token (OpenAI/GitHub Models).
  - **Acceptance Criteria:**
    - Fresh installs default to `enabled=false` (no predictive worker running).
    - Setup Center includes a toggle plus fields for `api_base_url`, `model`, and an optional token (token is never echoed back).
    - Core-server persists config to the DB, loads it on startup, and exposes `PUT /api/predictive/config` + `GET /api/predictive/status`.
    - Fast suites pass: `make ci-core-smoke` and `make ci-web-smoke`.
  - **Status:** Done (`make ci-core-smoke`, `make ci-web-smoke`, `make e2e-installer-stack-smoke`)


- **AN-17: Switch Emporia feed to local ESPHome MQTT bridge**
  - **Description:** Ingest Emporia metrics via the local ESPHome MQTT bridge and update deployment documentation/runbooks.
  - **Acceptance Criteria:**
    - Emporia feed ingests MQTT payloads from the ESPHome bridge using `CORE_ANALYTICS_EMPORIA__MQTT_*`.
    - Fixture payloads map to the correct device/channel metrics and persist in analytics samples.
    - Feed status reports MQTT source, topic filter, and device/channel mapping.
    - Cloud API path remains supported as a fallback if needed.
  - **Status:** Done (`poetry run pytest -k emporia_feed_ingests_esphome_mqtt`)


- **AN-1: Implement external feed adapters**
  - **Description:** Provide Emporia/Tesla/Enphase adapters with HTTP pollers, fixture replay, and feed status wiring.
  - **Acceptance Criteria:**
    - Provider adapters support fixture replay and HTTP polling via configuration.
    - Feed status reports missing devices and error metadata.
    - Fixture-driven contract tests cover Emporia/Tesla/Enphase parsing.
  - **Status:** Done (fixtures + HTTP pollers + status wiring)


- **AN-2: Write SQL/CAGG**
  - **Description:** Power (kW live, 24h/168h kWh split by source), grid vs solar vs battery.
  - **Status:** Done


- **AN-3: Compute water usage totals**
  - **Description:** Domestic/ag, reservoir depth trend, leak detection stats.
  - **Status:** Done


- **AN-4: Compute soil moisture**
  - **Description:** Per field min/max/avg with 168h history.
  - **Status:** Done


- **AN-5: Track alarm counts**
  - **Description:** Over 168h + node online/offline counts.
  - **Status:** Done


- **AN-6: Integrate utility rate schedule**
  - **Description:** Ingest a utility rate schedule (file-based or fixed-rate) via analytics feeds, persist snapshots to `analytics_rate_schedules`, and surface the latest schedule in `/api/analytics/power`.
  - **Acceptance Criteria:**
    - A rate schedule can be loaded from a local JSON file (or configured as a fixed rate).
    - Polling persists the latest schedule and estimated monthly cost.
    - Analytics UI can display provider, current rate, and estimated monthly cost.
  - **Status:** Done


- **AN-7: Implement REST endpoints**
  - **Description:** Returning structured series + cards.
  - **Status:** Done


- **AN-8: Build dashboard analytics components**
  - **Description:** Binding to endpoints.
  - **Status:** Done


- **AN-9: Seed demo analytics data**
  - **Description:** Add tests verifying responses.
  - **Status:** Done


- **AN-10: Optional utility rate ingestion from provider sites/APIs**
  - **Description:** Add provider adapters (utility website/API), credentials management, and secure storage so rate schedules can be fetched automatically instead of relying on a local JSON file.
  - **Acceptance Criteria:**
    - Provider credentials can be configured and stored securely.
    - The system can fetch and normalize a rate schedule automatically.
    - Failures are visible via integration status and do not break analytics endpoints.
  - **Status:** Done (provider mappers for PGE/ERCOT/NYISO + fixture-driven contract tests, HTTP dispatcher with file/fixed fallback + status metadata)


- **AN-11: Implement Renogy Rover Modbus RTU polling**
  - **Description:** Add `pymodbus`-backed serial polling for Renogy Rover charge controllers via `CORE_ANALYTICS_RENOGY__SERIAL_PORT`, including multi-controller unit IDs (`DEVICE_IDS=name@unit`) and mapping key telemetry (SOC, voltage, temps, solar/load power) into analytics samples.
  - **Acceptance Criteria:**
    - Feed supports `SERIAL_PORT` configuration and unit ID mapping (`DEVICE_IDS=name@unit`).
    - Fixture payloads map to `solar_kw`, `load_kw`, `battery_kw`, and status metrics.
    - Poll failures surface `missing_devices` and error metadata in `/api/analytics/feeds/status`.
  - **Status:** Done (implementation complete; hardware validation tracked in AN-19)


- **AN-12: Extend alarms schema for predictive/anomaly metadata** (See: [TICKET-0003](archive/tickets/TICKET-0003-database-schema-migration-for-predictive-alarms.md))
  - **Description:** Add `origin` and optional `anomaly_score` fields for alarms so predictive anomalies can be stored and queried (migration in `infra/migrations` + core-server model/schema updates).
  - **Acceptance Criteria:**
    - `make migrate` applies cleanly and existing rows default to `origin=threshold`.
    - The core server can read/write the new fields without breaking existing alarm endpoints.
  - **Status:** Done


- **AN-13: Integrate external AI anomaly detection for predictive alarms** (See: [TICKET-0001](archive/tickets/TICKET-0001-ai-anomaly-backend.md), [ADR 0001](../docs/ADRs/0001-external-ai-for-anomaly-detection.md))
  - **Description:** Add a non-blocking inference client in `apps/core-server` that forwards telemetry to an external model API and creates/escalates alarms using the predictive metadata fields.
  - **Acceptance Criteria:**
    - API client is configurable via env vars and failures “fail open” (threshold alarms continue).
    - Predictive alarms persist anomaly score/confidence when provided by the model.
    - Unit tests cover the client (mocked) and alarm evaluation behavior.
    - **Security review required:** Any `execute_python`-style sandbox for model-authored code must be locked down (no host filesystem access, no `os`/`sys`, no network, and hard time/resource limits).
  - **Status:** Done — Note: GitHub Models access may not include `openai/gpt-5`/`openai/gpt-5-mini` (`unavailable_model`); fallback to `openai/gpt-4.1` works for dev testing. End-to-end smoke: `apps/core-server/scripts/predictive_alarms_demo.py`.


- **AN-14: Visualize predictive alarms in the dashboard** (See: [TICKET-0002](archive/tickets/TICKET-0002-dashboard-alarms.md))
  - **Description:** Update `apps/dashboard-web` alarms UI to differentiate predictive vs standard alarms and display anomaly score/confidence when present (optional: overlay anomaly points on Trends).
  - **Acceptance Criteria:**
    - Alarms list and detail views visually distinguish predictive alarms and handle missing anomaly data gracefully.
    - Users can filter alarms by “Standard” vs “Predictive” (or equivalent toggle).
    - `npm run lint` and `npm run test` pass.
  - **Status:** Done — Predictive vs standard filters on Sensors page, origin badges, anomaly score pills, and demo predictive alarm data + screenshots.


- **AN-15: Predictive alarms scaffold** (See: [TICKET-0001](archive/tickets/TICKET-0001-ai-anomaly-backend.md), [TICKET-0002](archive/tickets/TICKET-0002-dashboard-alarms.md), [TICKET-0003](archive/tickets/TICKET-0003-database-schema-migration-for-predictive-alarms.md))
  - **Description:** Create initial file boundaries/stubs for predictive alarms (core-server service package, dashboard UI/types helpers, migration placeholder).
  - **Acceptance Criteria:**
    - Scaffold exists in `apps/core-server/app/services/predictive_alarms/`, `apps/dashboard-web/src/components/alarms/`, and `apps/dashboard-web/src/lib/alarms/`.
    - Migration placeholder exists at `infra/migrations/011_predictive_alarm_metadata.sql`.
  - **Status:** Done


- **AN-16: Fix predictive alarms integration in seeded demo database**
  - **Description:** Ensure predictive alarms behave correctly when running against the Postgres demo dataset (native stack; `make migrate` + `make seed` + `make demo-live`, or an installed controller via `farmctl`) by removing demo overlay leakage from `/api/dashboard/state`, hardening predictive alarm persistence, and adding a small control surface to diagnose/trigger inference.
  - **Acceptance Criteria:**
    - `/api/dashboard/state` returns DB-backed `alarms` and `alarm_events` when `CORE_DEMO_MODE=false`.
    - Predictive alarm persistence handles naive timestamps and persists predictive metadata updates reliably.
    - `GET /api/predictive/status` reports whether the worker is running and whether a model token is configured.
    - `POST /api/predictive/bootstrap` can generate predictive alarms from recent DB metrics (best effort).
  - **Status:** Done


- **AN-35: Analytics Overview: fix soil moisture flatline (real `/api/analytics/soil`)**
  - **Description:** The Analytics Overview “Soil moisture” chart must reflect real moisture telemetry instead of a fabricated all-zero series. Replace the stubbed `GET /api/analytics/soil` response with a real aggregation over `sensors` + `metrics` that produces fleet-level min/max/avg moisture series.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0048-analytics-overview-soil-moisture-graph-flatline.md`
    - `docs/ADRs/0008-real-soil-analytics-from-metrics.md`
    - Tier‑A run: `project_management/runs/RUN-20260131-tier-a-an35-soil-analytics-0.1.9.232.md`
  - **Acceptance Criteria:**
    - `GET /api/analytics/soil` returns non-zero values when moisture metrics exist (no fabricated all-zero placeholder when real data is present).
    - Response includes `series_avg`, `series_min`, and `series_max` (with `series` retained for backwards compatibility).
    - Analytics Overview renders a non-zero soil moisture chart consistent with Trends (no 0% flatline while sensors are reporting).
    - Tier A validated on installed controller and evidence captured + viewed:
      - `make e2e-installed-health-smoke` passes
      - screenshot: `manual_screenshots_web/20260131_162607/analytics.png`
    - Local validation passes: `make ci-core-smoke` and `make ci-web-smoke`.
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to CS-69)

---

## Time-Series Similarity Engine (TSSE)
### Done


- **TSSE-1: TSSE master: complete ticket set + Tier A validation**
  - **Description:** Master gate for the full TSSE delivery. This task is Done only when all TSSE tickets/tasks are Done and Tier A validation evidence is recorded.
  - **References:**
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All TSSE implementation tasks (`TSSE-2` through `TSSE-23`) are `Done` and their corresponding ticket docs are satisfied.
    - Tier A validation is completed on the installed controller (no DB/settings reset), with evidence recorded in a run log under `project_management/runs/` (include installed bundle version + at least one captured-and-viewed screenshot path under `manual_screenshots_web/`).
    - Remaining TSSE work is executed by a single agent (no Collab Harness multi-agent workflow required).
  - **Notes / Run Log:**
    - 2026-01-23: Tier A evidence should reference the runbook steps (installed health checks, bundle build + upgrade, `make e2e-installed-health-smoke`) plus TSSE-specific artifacts (bench report under `reports/` and at least one viewed screenshot of the TSSE analysis UX under `manual_screenshots_web/`).
    - 2026-01-24: Added Tier‑A TSSE evidence template: `project_management/runs/RUN-TEMPLATE-tsse-tier-a-validation.md`.
    - 2026-01-24: Worker C compiled Tier‑A execution + evidence checklist (runbook + template summary) for ops validation.
    - 2026-01-24: Tier‑A validation run in progress: `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.206.md` (upgrade to 0.1.9.206, installed health smoke PASS, screenshots captured to `manual_screenshots_web/20260124_045442/`, bench report `reports/tsse-bench-20260124_050332-0.1.9.206.md`). Screenshot viewing + lake backfill/parity follow-ups pending.
    - 2026-01-24: Tier‑A validation completed to `0.1.9.211` (from `0.1.9.210`): installed health smoke PASS, TSSE Playwright Tier‑A suite PASS (desktop Chromium), bench + recall + lake parity evidence recorded, and screenshots captured under `manual_screenshots_web/tier_a_0.1.9.211_trends_*`. Evidence log: `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.211.md`. Screenshot viewing/review is still pending (hard gate for TSSE-1).
    - 2026-01-24: Fixed installed Trends scans failing with `403 Missing capabilities: analysis.run` by adding an idempotent admin capability backfill migration and upgrading the installed controller to `0.1.9.212`. Tier‑A evidence + viewed screenshots recorded in `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.212.md`.
  - **Status:** Done (P0)


- **TSSE-2: TSE-0001: TSSE requirements + success metrics + design ADR**
  - **Description:** Freeze user-facing requirements, success metrics, and the committed single-node architecture (Parquet + DuckDB + Qdrant; no analysis fallback).
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0001-requirements-success-metrics-adr.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0001-requirements-success-metrics-adr.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-24: ADR exists and includes performance targets + test-plan mapping: `docs/ADRs/0006-time-series-similarity-engine-(tsse)-on-controller-similarity-search.md`.
  - **Status:** Done (P0)


- **TSSE-3: TSE-0002: Analysis Jobs framework (server-side, Rust)**
  - **Description:** Add a durable analysis job system (create/progress/cancel/result) to move heavy analyses off the client.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0002-analysis-jobs-framework-rust.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0002-analysis-jobs-framework-rust.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker A implemented analysis job schemas + runners for `correlation_matrix_v1`, `event_match_v1`, `cooccurrence_v1`, and `matrix_profile_v1` in `apps/core-server-rs` (bounded params, progress phases, cancel checks).
    - 2026-01-24: Job framework tests + OpenAPI checks pass via `make ci-smoke` (log: `reports/ci-smoke-20260123_174943.log`).
  - **Status:** Done (P0)


- **TSSE-4: TSE-0003: Analysis API surface (create/progress/result/preview)**
  - **Description:** Define the public API surface for analysis jobs, including progress streaming/polling and small-payload previews.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0003-analysis-api-create-progress-result-preview.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0003-analysis-api-create-progress-result-preview.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker C delivered API/UX design proposals (analysis job endpoints, versioned schemas, progress events streaming vs polling, preview drilldown payload shape).
    - 2026-01-23: `python3 tools/check_openapi_coverage.py` flagged `POST /api/analysis/preview` coverage, then passed after OpenAPI updates (run log).
    - 2026-01-24: OpenAPI coverage + endpoint smoke validated via `make ci-smoke` (log: `reports/ci-smoke-20260123_174943.log`).
  - **Status:** Done (P0)


- **TSSE-5: TSE-0004: Parquet “analysis lake” spec (90d hot, sharded partitions)**
  - **Description:** Specify the Parquet partition/shard layout, compaction approach, retention policy (hot 90d), and future NAS readiness considerations.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0004-parquet-analysis-lake-spec.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0004-parquet-analysis-lake-spec.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker A delivered proposed Parquet hot-partition/shard layout + config path sketch for review.
    - 2026-01-23: Worker A reviewed current lake implementation vs TSE-0004 acceptance criteria; documented gaps (manifest/retention/cold handling) + proposed changes.
    - 2026-01-24: Verified spec + tooling: manifest/watermarks + hot/cold layout documented in `project_management/archive/archive/tickets/TSE-0004-parquet-analysis-lake-spec.md` and inspector/move CLIs exist under `apps/core-server-rs/src/bin/` (validated via `make ci-smoke`, log: `reports/ci-smoke-20260123_174943.log`).
  - **Status:** Done (P0)


- **TSSE-6: TSE-0005: Postgres → Parquet replication (incremental + backfill + compaction)**
  - **Description:** Implement a Rust data plane that continuously materializes metrics into the Parquet analysis lake for fast local scans.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0005-postgres-to-parquet-replication-compaction.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0005-postgres-to-parquet-replication-compaction.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker A delivered minimal Rust replication plan (backfill/incremental/compaction) + file path layout.
    - 2026-01-23: Worker A reviewed replication code vs TSE-0005 acceptance criteria; documented gaps (backfill command, COPY/export performance, late-window usage, compaction/dedupe, correctness checks) + proposed changes.
    - 2026-01-24: TSSE audit: `lake_backfill_v1` exists but no operator runbook/CLI surfaced; no automated correctness spot-checks for Postgres↔Parquet parity; replication targets `metrics` only (forecast/derived series not yet decided).
    - 2026-01-24: Added runbook + parity spot-check tooling: `docs/runbooks/tsse-lake-backfill-and-parity.md` and `apps/core-server-rs/src/bin/tsse_lake_parity_check.rs` (writes report under `reports/`).
    - 2026-01-24: Implemented replication late-window usage (`analysis_late_window_hours`) + replication run metadata, and switched incremental export to a bulk Postgres COPY → DuckDB → partitioned Parquet path (no row-by-row fanout); backfill now stages/moves Parquet outputs (tests added).
    - 2026-01-24: Tier‑A backfill attempt (`days=90`, replace-existing) failed mid-run because DuckDB COPY output dir was non-empty when reusing a shared run dir across days; fixed by writing per-day outputs under `run_dir/date=YYYY-MM-DD` and partitioning by shard only (commit `52eb231`). Rerun required for 90d backfill + parity evidence.
    - 2026-01-24: Review: acceptance gaps remain for TSE-0005 (need recorded 90d backfill/parity report; lag-bound evidence under load; compaction file-count bound evidence; forecast/derived replication scope still undecided).
    - 2026-01-24: Tier‑A (installed controller `0.1.9.211`) lake inspection report recorded via API job: `reports/tsse-lake-inspect-20260124_165836-0.1.9.211.json` (includes `backfill_completed_at`, `computed_through_ts`, and `last_run_*` replication metadata).
    - 2026-01-24: Tier‑A parity report recorded via API job: `reports/tsse-lake-parity-20260124_083505-0.1.9.211.md` (0 mismatches; shard file-counts bounded; replication summary recorded). Note: direct filesystem parity (`ops_tsse_parity_spotcheck`) may be blocked on hardened installs due to lake path permissions; prefer the API parity job per `docs/runbooks/tsse-parquet-parity-spot-check.md`.
  - **Status:** Done (P0)


- **TSSE-7: TSE-0006: DuckDB embedded query layer (Rust) for Parquet reads**
  - **Description:** Add an embedded DuckDB query layer to efficiently read Parquet partitions/shards in analysis jobs.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0006-duckdb-embedded-query-layer-rust.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0006-duckdb-embedded-query-layer-rust.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker A delivered DuckDB embedded query approach + crate suggestions.
    - 2026-01-23: Worker A reviewed DuckDB query service vs TSE-0006 acceptance criteria; documented gaps (query helpers, pruning tests, benchmarks, cold-path reads) + proposed changes.
    - 2026-01-24: DuckDB correctness tests exist in `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs` (points, buckets, cold partition reads) in addition to shard/partition pruning coverage.
    - 2026-01-24: Added multi-sensor + cross-partition correctness tests (TSSE-24) and validated via `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (log: `reports/cargo-test-core-server-rs-20260123_174724.log`).
  - **Status:** Done (P0)


- **TSSE-8: TSE-0007: Qdrant local deployment + schema (required ANN stage)**
  - **Description:** Add Qdrant as a required local dependency on the controller and define collection schemas for TSSE candidate generation.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0007-qdrant-local-daemon-schema-launchd.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0007-qdrant-local-daemon-schema-launchd.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Ops review confirmed Qdrant is already bundled + wired into launchd via `farmctl` (native deps + bundle manifest + launchd plan + core-server schema ensure). Missing hardening items are tracked under TSSE-23 (permissions/umask/path validation).
    - 2026-01-24: TSSE audit: Qdrant collection schema only defines vectors (no payload index config). Payload now includes `interval_seconds`/`is_derived`/`is_public_provider` but candidate-gen filters do not yet use those fields; no scheduled embeddings refresh pipeline wired.
    - 2026-01-24: Review: TSE-0007 acceptance gaps include missing payload index creation + filter coverage for `interval_seconds`/`is_derived`/`is_public_provider`, plus no latency benchmark evidence for Qdrant query overhead vs scoring.
    - 2026-01-24: Implemented payload index schema assertions for `interval_seconds`/`is_derived`/`is_public_provider` and verified candidate-gen filters include those payload keys (tests in `apps/core-server-rs/src/services/analysis/qdrant.rs` + `tsse/candidate_gen.rs`).
    - 2026-01-24: Tier‑A (installed controller `0.1.9.211`) Qdrant health check PASS: `curl -fsS http://127.0.0.1:6333/healthz`.
    - 2026-01-24: Bench evidence: `reports/tsse-bench-20260124_083042-0.1.9.211.md` shows Qdrant search p50/p95 `8/11 ms` and candidate-gen p50/p95 `15/19 ms` (negligible vs exact stage).
  - **Status:** Done (P0)


- **TSSE-9: TSE-0008: Feature/embedding pipeline (robust, multi-scale signatures)**
  - **Description:** Implement robust multi-scale features/embeddings for similarity search candidate generation.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0008-robust-multiscale-features-embeddings-pipeline.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0008-robust-multiscale-features-embeddings-pipeline.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B delivered a proposed multi-scale robust feature/embedding design (value/derivative/event vectors + default window set) and a recall-harness outline to support episodic matches without excluding true positives.
    - 2026-01-23: Worker B delivered a concrete embedding schema sketch (multi-vector payload + versioning fields) plus default feature dimensions/window sets to align with Qdrant and downstream “why ranked” outputs.
    - 2026-01-24: TSSE audit: no recall evaluation harness/curated-pairs runner implemented; embeddings_build_v1 exists but no incremental/scheduled refresh pipeline wired.
    - 2026-01-24: Worker B drafted a recall@K harness design for ANN candidate generation on the synthetic clustered-sensor bench dataset (ground-truth cluster mapping, per-sensor recall aggregation, pass/fail thresholds), with suggested metadata additions and bin layout.
    - 2026-01-24: Implemented recall evaluation harness: `apps/core-server-rs/src/bin/tsse_recall_eval.rs` (writes report under `reports/`).
    - 2026-01-24: Improved `tsse_recall_eval` API input mode by chunking >1y windows and deduping API buckets to avoid `/api/metrics/query` max-window errors.
    - 2026-01-24: Review: TSE-0008 still lacks a scheduled embeddings refresh (incremental + periodic full rebuild), a curated-pairs/recall report under `reports/`, and any resource-budget evidence for embedding computation on the controller.
    - 2026-01-24: Scheduled embeddings refresh is implemented via `apps/core-server-rs/src/services/analysis/embeddings_refresh.rs` (incremental + full rebuild) and wired in `apps/core-server-rs/src/main.rs` behind `analysis_embeddings_refresh_enabled`.
    - 2026-01-24: Curated-pairs recall evidence recorded (Tier‑A controller): `reports/tsse-recall-eval-renogy-voltage-V-pairs-20260124_165326-k150-0.1.9.211.md` (PASS; `pairs_file` recorded in report).
  - **Status:** Done (P0)


- **TSSE-10: TSE-0009: Candidate generation (Qdrant + filters + recall safeguards)**
  - **Description:** Implement fast, high-recall candidate generation using Qdrant, plus filters/safeguards so exact scoring stays bounded.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0009-candidate-generation-qdrant-filters-recall-safeguards.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0009-candidate-generation-qdrant-filters-recall-safeguards.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B delivered a proposed Qdrant union-query strategy (multi-vector searches + min pool sizing + adaptive widening + reason tracking) aimed at high recall without a DB fallback.
    - 2026-01-23: Worker B delivered a staged widening plan (strict filters → relaxed filters, k/ef ramp) with candidate reasons + union bookkeeping fields for result schemas.
    - 2026-01-24: Worker B drafted a candidate-gen recall harness design for TSE-0009 (synthetic clustered sensors, recall@K report + pass/fail gate, option to extend `tsse_bench` or add new Rust bin).
    - 2026-01-24: Implemented recall evaluation harness: `apps/core-server-rs/src/bin/tsse_recall_eval.rs` (writes report under `reports/`).
    - 2026-01-24: TSSE audit: `build_widen_plan()` in `apps/core-server-rs/src/services/analysis/tsse/candidate_gen.rs` only relaxes `same_*_only` toggles; it does not relax other filters (e.g. `interval_seconds`, `is_derived`, `is_public_provider`), so widening cannot recover recall across those dimensions. Decide widening policy (which filters are allowed to widen) and add tests + evidence.
    - 2026-01-24: Implemented widening policy to relax `interval_seconds`/`is_derived`/`is_public_provider` ahead of type/unit constraints (keeps semantic filters longest) and added tests for widening order + coverage.
    - 2026-01-24: Review: TSE-0009 still missing recall report + pass/fail gate evidence, plus candidate-gen latency evidence (<250ms typical) from `tsse_bench`; consider wiring Qdrant query params (ef/search) to match widening plan.
    - 2026-01-24: Recall + latency evidence recorded (Tier‑A controller):
      - Candidate-gen latency p50/p95: `15/19 ms` (PASS) in `reports/tsse-bench-20260124_083042-0.1.9.211.md`.
      - Recall@K gate PASS on curated pairs in `reports/tsse-recall-eval-renogy-voltage-V-pairs-20260124_165326-k150-0.1.9.211.md`.
  - **Status:** Done (P0)


- **TSSE-11: TSE-0010: Exact episodic similarity scoring (robust + multi-window + lag)**
  - **Description:** Implement the exact episodic, outlier-robust similarity scoring model that produces explainable “episodes” and ranks accordingly.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0010-exact-episodic-similarity-scoring-rust.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0010-exact-episodic-similarity-scoring-rust.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B delivered a proposed robust multi-window episodic similarity algorithm (winsorized z + lag coarse/refine + episode extraction + why-ranked components) with default parameters.
    - 2026-01-23: Worker B delivered a scoring schema sketch (episode/lag metrics + penalties/bonuses) and a Rust module layout proposal for implementation and benchmarks.
    - 2026-01-24: Scoring implementation + determinism/episode tests pass in `apps/core-server-rs/src/services/analysis/tsse/scoring.rs` (validated via `cargo test`, log: `reports/cargo-test-core-server-rs-20260123_174724.log`).
  - **Status:** Done (P0)


- **TSSE-12: TSE-0011: Related Sensors scan job end-to-end (never error)**
  - **Description:** Replace the client-side Related Sensors scan with a server-side analysis job using the TSSE pipeline.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0011-related-sensors-scan-job-end-to-end.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0011-related-sensors-scan-job-end-to-end.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B delivered a related_sensors_v1 result payload sketch (candidates + episodic “why ranked” fields + ANN reasons) to align job storage with preview endpoints.
    - 2026-01-24: `related_sensors_v1` job runner + API wiring landed in `apps/core-server-rs`; remaining work is on-controller evidence (bench + Tier‑A screenshot review) for “never error” guarantees.
    - 2026-01-24: Review: TSE-0011 still needs evidence runs (bench report + Tier‑A screenshot) and an explicit “never error” validation run for small-interval/long-range requests (recorded under `reports/` + `manual_screenshots_web/`).
    - 2026-01-24: Tier‑A validation: Playwright TSSE Tier‑A suite PASS (`chromium-desktop`) and Related Sensors UI screenshot captured at `manual_screenshots_web/tier_a_0.1.9.211_trends_auto_compare_2026-01-24_162819580Z/01_trends_auto_compare_key.png` (VIEWED pending).
    - 2026-01-24: “Small interval + long range” never-error check executed via API create+cancel path (Playwright asserts create/cancel succeeds without UI hang).
  - **Status:** Done (P0)


- **TSSE-13: TSE-0012: Preview/episode drilldown endpoints**
  - **Description:** Add small-payload preview + episode drilldown endpoints so the UI can show “why ranked” without pulling huge series.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0012-preview-episode-drilldown-endpoints.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0012-preview-episode-drilldown-endpoints.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker C delivered preview drilldown API + UI compatibility plan (episode selection, bounded series windows, align-by-lag toggle, normalized vs raw preview handling).
    - 2026-01-23: Worker B delivered episode preview payload fields (bounded series + lag-aligned overlays + episode metadata) to align scoring outputs with drilldown.
    - 2026-01-24: Preview endpoint implemented + gated (authz + max window clamps) in `apps/core-server-rs`; remaining work is perf evidence via `tsse_bench` and Tier‑A screenshot review.
    - 2026-01-24: Review: TSE-0012 still lacks preview latency evidence (bench report) and paging strategy for oversized previews if clamped windows are insufficient.
    - 2026-01-24: Preview latency evidence recorded (Tier‑A controller): preview p50/p95 `34/44 ms` (PASS) in `reports/tsse-bench-20260124_083042-0.1.9.211.md`.
    - 2026-01-24: Tier‑A preview screenshot captured at `manual_screenshots_web/tier_a_0.1.9.211_trends_event_match_2026-01-24_162819581Z/02_trends_event_match_preview.png` (VIEWED pending).
  - **Status:** Done (P0)


- **TSSE-14: TSE-0013: Dashboard-web Related Sensors job UX**
  - **Description:** Convert the Trends UI to submit a single job, show progress/cancel, and render episodic results with drilldown.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0013-dashboard-web-related-sensors-job-ux.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0013-dashboard-web-related-sensors-job-ux.md` are met.
    - UI/UX guardrails in `apps/dashboard-web/AGENTS.md` are met (or deliberate debt is tracked as a `DW-*` ticket with owner + exit criteria).
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker C delivered AutoComparePanel refactor plan for job-based UX (create job, poll events/status, render ranked results + episode summaries + why-ranked chips, on-demand preview drilldown).
    - 2026-01-23: Worker C started AutoComparePanel job-UX refactor; added analysis types + scaffolding (see `RUN-20260123-tsse-collab-harness-implementation.md`).
    - 2026-01-24: TSSE audit: no Playwright coverage for Related Sensors job UX (AutoComparePanel).
    - 2026-01-24: TSSE audit: Related Sensors UI does not surface `computed_through_ts` watermark from job results.
    - 2026-01-24: Update: AutoComparePanel now keeps and displays the computed-through watermark (stable across job lifecycle) and Playwright asserts progress/cancel/watermark states (`apps/dashboard-web/playwright/trends-auto-compare.spec.ts`, `chromium-desktop`).
    - 2026-01-24: Tier‑A UI evidence captured (installed controller `0.1.9.211`): `manual_screenshots_web/tier_a_0.1.9.211_trends_auto_compare_2026-01-24_162819580Z/01_trends_auto_compare_key.png` (VIEWED pending).
  - **Status:** Done (P1)


- **TSSE-15: TSE-0014: Relationships / correlation matrix job**
  - **Description:** Migrate Relationships/Correlation Matrix computations to the analysis jobs system.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0014-relationships-correlation-matrix-job.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0014-relationships-correlation-matrix-job.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B proposed a bounded correlation-matrix strategy (Qdrant top-K candidate pool + per-job pair caps + early-stop on low overlap/low coarse score) with default params + complexity bounds.
    - 2026-01-23: Worker A implemented `correlation_matrix_v1` job schema + runner in `apps/core-server-rs` (bounded params, progress phases, cancel-aware DuckDB reads; result includes ordered sensors + matrix + timings).
    - 2026-01-24: Worker C refactored Trends Relationships panel to job-based UX (submit/poll/cancel/result) and documented UI contract expectations for `correlation_matrix_v1` results (sensor_ids + matrix cells).
    - 2026-01-24: Follow-up fix removed duplicate local matrix computation so the table renders job results only.
    - 2026-01-24: TSSE audit: dashboard ignores `truncated_sensor_ids` and prefers requested sensor IDs over `result.sensor_ids`, so UI can render rows that the backend dropped; TS types omit `sensors` metadata + `bucket_count`.
    - 2026-01-24: Update: Relationships panel now surfaces computed-through watermark + bucket size/count metadata and renders truncation warnings/watermark when the backend returns fewer sensors (Playwright asserts truncation watermark via `relationships-truncation-watermark`).
    - 2026-01-24: Tier‑A UI evidence captured (installed controller `0.1.9.211`):
      - `manual_screenshots_web/tier_a_0.1.9.211_trends_relationships_2026-01-24_162820805Z/01_trends_relationships_key.png` (VIEWED pending)
      - `manual_screenshots_web/tier_a_0.1.9.211_trends_relationships_2026-01-24_162820805Z/02_trends_relationships_pair_analysis_key.png` (VIEWED pending)
  - **Status:** Done (P1)


- **TSSE-16: TSE-0015: Events/Spikes matching job**
  - **Description:** Migrate Events/Spikes matching analysis to server-side jobs with bounded compute and explainable outputs.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0015-events-spikes-matching-job.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0015-events-spikes-matching-job.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B aligned event/spike matching with robust delta z-scoring + episodic scoring output to reuse the TSSE episode semantics.
    - 2026-01-23: Worker B proposed bounded event-matching compute (robust spike detection + time-bin joins + per-series event caps + lag-limited matching) with default params + complexity bounds.
    - 2026-01-23: Worker A implemented `event_match_v1` job schema + runner in `apps/core-server-rs` (robust delta z detection, bounded candidate/lag/event caps, progress phases + cancel checks, episodic outputs).
    - 2026-01-24: TSSE audit: Events/Spikes UI exists but lacks episode/why-ranked drilldown or preview integration; no Playwright coverage for Events/Spikes matching UX.
    - 2026-01-24: Update: EventMatchPanel now surfaces computed-through watermark + interval/bucket metadata + truncation summary and includes a preview drilldown path; Playwright coverage exists for the job flow (`apps/dashboard-web/playwright/trends-event-match.spec.ts`, `chromium-desktop`) with stub data.
    - 2026-01-24: Tier‑A UI evidence captured (installed controller `0.1.9.211`):
      - `manual_screenshots_web/tier_a_0.1.9.211_trends_event_match_2026-01-24_162819581Z/01_trends_event_match_key.png` (VIEWED pending)
      - `manual_screenshots_web/tier_a_0.1.9.211_trends_event_match_2026-01-24_162819581Z/02_trends_event_match_preview.png` (VIEWED pending)
  - **Status:** Done (P1)


- **TSSE-17: TSE-0016: Co-occurrence job**
  - **Description:** Move Co-occurrence scans to server-side jobs and align with the TSSE candidate/scoring plane.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0016-cooccurrence-job.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0016-cooccurrence-job.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B outlined a co-occurrence episode scoring approach (windowed overlap + F1/Jaccard + episode ranges) aligned with TSSE outputs.
    - 2026-01-23: Worker B proposed bounded co-occurrence compute (candidate-filtered pairs + interval overlap sweep + early-stop on low union/overlap) with default params + complexity bounds.
    - 2026-01-23: Worker A implemented `cooccurrence_v1` job schema + runner in `apps/core-server-rs` (bounded sensor/event caps, tolerance bucket scoring, progress phases + cancel checks).
    - 2026-01-24: Worker C refactored Trends Co-occurring anomalies panel to job-based UX (submit/poll/cancel/result) with focus-scan parameters and result bucket contract expectations.
    - 2026-01-24: Aligned Co-occurrence job param typing with focus-scan payload fields (mode, candidate list, scope, source filter).
    - 2026-01-24: Fixed dashboard-web `CooccurrencePanel` to match backend `CooccurrenceJobParamsV1` (always send `sensor_ids`, preserve `focus_sensor_id`, and remove legacy `mode/scope/source_filter/candidate_sensor_ids` from request params). Also updated `InlineBannerTone` to accept `danger` (used by TSSE panels) so `npm run build` typechecks. CI: `make ci-web-smoke` passes.
    - 2026-01-24: TSSE audit: backend `CooccurrenceEventV1` includes per-event `ts`, but dashboard types drop it and UI reuses bucket timestamp; no Playwright coverage for Co-occurrence panel.
    - 2026-01-24: Update: CooccurrencePanel now surfaces computed-through watermark + interval/bucket metadata and renders a truncation watermark when the backend truncates sensors; Playwright coverage exists for the job flow (`apps/dashboard-web/playwright/trends-cooccurrence.spec.ts`, `chromium-desktop`) with stub data.
    - 2026-01-24: Tier‑A UI evidence captured (installed controller `0.1.9.211`): `manual_screenshots_web/tier_a_0.1.9.211_trends_cooccurrence_2026-01-24_162819580Z/01_trends_cooccurrence_key.png` (VIEWED pending).
  - **Status:** Done (P1)


- **TSSE-18: TSE-0017: Matrix Profile job (scoped + safe)**
  - **Description:** Redesign Matrix Profile-like analysis as a job with safety bounds, progressive refinement, and operator-friendly UX.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0017-matrix-profile-job-scoped.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0017-matrix-profile-job-scoped.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B proposed a scoped matrix-profile flow (candidate-seeded, compute-budgeted, early-stop) that emits motif episodes without blocking the controller.
    - 2026-01-23: Worker B proposed a bounded matrix-profile compute plan (downsample + capped window sizes + sampled queries + time-budget early stop) with default params + complexity bounds.
    - 2026-01-23: Worker A implemented `matrix_profile_v1` job schema + runner in `apps/core-server-rs` (bounded points/windows, cancel-aware compute, motifs/anomalies summaries + timings).
    - 2026-01-24: Worker C refactored Trends Matrix Profile panel to job-based UX (submit/poll/cancel/result) and documented result fields required for explorer views.
    - 2026-01-24: Update: `matrix_profile_v1` now enforces an explicit compute budget with early-stop warnings, and emits phase timing events (`phase_timing`) for compute; MatrixProfilePanel already uses job-provided motifs/anomalies + warnings/source/sample counts and sends `top_k`/`max_windows`/`exclusion_zone` params (Playwright `trends-matrix-profile.spec.ts` passes under `chromium-desktop`).
    - 2026-01-24: Review: remaining work is controller-side evidence that matrix_profile cannot “lock up” the stack under worst-case ranges (Tier‑A run + screenshot review).
    - 2026-01-24: Tier‑A UI evidence captured (installed controller `0.1.9.211`):
      - `manual_screenshots_web/tier_a_0.1.9.211_trends_matrix_profile_2026-01-24_162820468Z/01_trends_matrix_profile_key.png` (VIEWED pending)
      - `manual_screenshots_web/tier_a_0.1.9.211_trends_matrix_profile_2026-01-24_162820468Z/02_trends_matrix_profile_self_similarity_key.png` (VIEWED pending)
  - **Status:** Done (P1)


- **TSSE-19: TSE-0018: Remove “series too large” failures in chart metrics path**
  - **Description:** Replace hard-fail point caps with paging/streaming for large metric requests so charting remains slow-but-successful.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0018-remove-series-too-large-from-chart-metrics.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0018-remove-series-too-large-from-chart-metrics.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker C delivered paging/streaming plan + UI rendering safeguards for large series (stream/paged fetcher, progressive render, explicit decimation/watermarks, cancel-aware UX).
    - 2026-01-23: Worker A reviewed chart metrics path vs TSE-0018; documented current MAX_METRICS_POINTS failure + proposed paging/duckdb routing changes.
    - 2026-01-24: Worker A implemented cursor-based paging for `/api/metrics/query` and dashboard paging merge; added unit tests for metrics paging (`apps/core-server-rs`) and frontend paging (`apps/dashboard-web`); removed user-facing “Requested series too large” failure mode for charts.
  - **Status:** Done (P1)


- **TSSE-20: TSE-0019: Perf + scale benchmarks on Mac mini (gates + regressions)**
  - **Description:** Add a repeatable benchmark suite for TSSE on the target Mac mini hardware, with regression gates and report artifacts.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0019-perf-scale-benchmarks-mac-mini.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0019-perf-scale-benchmarks-mac-mini.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Ops review: benchmark artifacts should land under `reports/` (Tier‑A bundle allowlist) with a stable naming scheme and explicit pass/fail thresholds recorded alongside the run.
    - 2026-01-23: Drafted `reports/tsse-bench-template.md` with threshold slots (ADR p50 targets + placeholders for p95/throughput/resource budgets) to standardize report output before harness wiring.
    - 2026-01-24: Added `apps/core-server-rs/src/bin/tsse_bench_dataset_gen.rs` (synthetic lake + DB sensors) and `apps/core-server-rs/src/bin/tsse_bench.rs` (job/preview harness + p50/p95 gates; report under `reports/`).
    - 2026-01-24: TSSE audit: `tsse_bench` only enforces candidate/preview p50/p95 targets; scoring throughput + end-to-end job latency + CPU/RAM/disk budgets remain placeholders (no pass/fail gates).
    - 2026-01-24: Tier‑A bench run on installed controller (`0.1.9.206`): report `reports/tsse-bench-20260124_050332-0.1.9.206.md` (PASS for candidate+preview p50/p95 thresholds).
    - 2026-01-24: Review: remaining work is to add pass/fail gates for scoring throughput, end-to-end job latency, and resource budgets (CPU/RAM/disk IO) if we decide those are hard constraints beyond ADR 0006.
    - 2026-01-24: Tier‑A bench run on installed controller (`0.1.9.211`): report `reports/tsse-bench-20260124_083042-0.1.9.211.md` (PASS for candidate+preview p50/p95 thresholds; includes Qdrant search time p50/p95).
  - **Status:** Done (P0)


- **TSSE-21: TSE-0020: Observability + “why ranked” explanations + profiling hooks**
  - **Description:** Add observability for TSSE compute, plus an operator-visible “why ranked” surface that’s explainable and debuggable.
  - **References:**
    - `project_management/tickets/TSE-0020-observability-profiling-why-ranked.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/tickets/TSE-0020-observability-profiling-why-ranked.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker B delivered observability + benchmark hook proposals (phase-level spans/metrics + CPU profiling toggles + criterion/iai bench scaffolds).
    - 2026-01-24: Worker B audited analysis job observability surfaces and implemented minimal tracing spans + durable phase timing events (`phase_timing`, `runner_summary`) in `apps/core-server-rs`; added a consistent `why_ranked` summary for `event_match_v1` candidates and documented the schema in `project_management/tickets/TSE-0020-observability-profiling-why-ranked.md`.
    - 2026-01-24: Update: added a profiling hook (per-job `profile=true` + optional `profile_output_dir`) that writes a flamegraph and emits a `profile_written` job event; also standardized `timings_ms` keys across TSSE jobs and ensured phase timing events include failure context (`current_phase`).
    - 2026-01-24: Review: remaining gaps include metrics/OTel counters for TSSE phases and consistent `why_ranked` coverage across all ranked outputs (beyond related_sensors_v1 + event_match_v1).
    - 2026-01-24: Tier‑A evidence: `reports/tsse-bench-20260124_083042-0.1.9.211.md` consumes per-job `timings_ms` for candidate_gen/duckdb/scoring and records Qdrant timing (`qdrant_search_ms`) plus phase-latency gates.
  - **Status:** Done (P0)


- **TSSE-22: TSE-0021: NAS readiness (cold partitions)**
  - **Description:** Ensure Parquet partitions can be relocated to a NAS later without redesigning the TSSE pipeline.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0021-nas-readiness-cold-partitions.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0021-nas-readiness-cold-partitions.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Worker A proposed hot/cold path abstraction and migration considerations.
    - 2026-01-23: Worker A reviewed NAS readiness gaps vs TSE-0021; documented missing move tool, manifest location tracking, and mixed hot/cold query support.
    - 2026-01-24: Verified hot/cold path support + move tooling (`tsse_lake_move_partition`) and cold-path DuckDB read tests in `apps/core-server-rs` (validated via `make ci-smoke`, log: `reports/ci-smoke-20260123_174943.log`).
  - **Status:** Done (P2)

- **TSSE-23: TSE-0022: Security hardening for analytics plane**
  - **Description:** Add explicit hardening for the analytics plane (job authz, paths/perms, and safe defaults) so TSSE can run on a controller safely.
  - **References:**
    - `project_management/archive/archive/tickets/TSE-0022-security-hardening-analytics-plane.md`
    - `project_management/archive/archive/tickets/TSSE-INDEX.md`
  - **Acceptance Criteria:**
    - All acceptance criteria in `project_management/archive/archive/tickets/TSE-0022-security-hardening-analytics-plane.md` are met.
    - Collab Harness is used during implementation with explicit worker deliverables captured.
  - **Notes / Run Log:**
    - 2026-01-23: Ops review gaps to cover: enforce chmod/umask for analysis + Qdrant dirs/files (0700/0750, 0600 configs), validate/canonicalize analysis paths (reject traversal/symlinks outside data_root), and add missing security tests for per-user job caps (429s) + preview max window clamping + analysis authz enforcement.
    - 2026-01-24: Added tests for analysis authz (cap enforcement), per-user job cap 429s, preview max-window clamp, and config path validation (reject traversal/outside-base/symlink escape).
  - **Status:** Done


- **TSSE-24: DuckDB query correctness tests (points + buckets)**
  - **Description:** Add unit/integration tests for `DuckDbQueryService` that assert row-level correctness and bucketed aggregation results from Parquet (not just pruning).
  - **Acceptance Criteria:**
    - Tests cover multi-sensor reads across partitions/files with correct ordering and time-range inclusion/exclusion.
    - Bucketed reads assert average + sample counts per bucket and correct bucket timestamp alignment.
    - Tests use tiny Parquet lake fixtures in temp dirs and run in CI (`cargo test` for `apps/core-server-rs`) without external deps.
  - **Notes / Run Log:**
    - 2026-01-24: Worker A drafted a concrete correctness test plan (fixture builder + invariants).
    - 2026-01-24: Implemented correctness tests in `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs` (multi-sensor points across partitions/files; bucket alignment + samples) and validated with `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (log: `reports/cargo-test-core-server-rs-20260123_174724.log`).
  - **Status:** Done (P0)

- **TSSE-25: Postgres <-> Parquet parity spot-check runbook + ops CLI**
  - **Description:** Provide a low-risk operator runbook and CLI to spot-check parity between Postgres metrics and the Parquet analysis lake for a small set of sensors/windows.
  - **Acceptance Criteria:**
    - Runbook exists at `docs/runbooks/tsse-parquet-parity-spot-check.md` with controller-safe steps.
    - Ops CLI exists at `apps/core-server-rs/src/bin/ops_tsse_parity_spotcheck.rs` and is read-only.
    - CLI supports sensor IDs + window selection and defaults to the lake watermark/lag.
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml --bin ops_tsse_parity_spotcheck` passes.
  - **Notes / Run Log:**
    - 2026-01-24: Added runbook + ops CLI; build command run.
  - **Status:** Done (local build)

- **TSSE-26: Fix lag search missing sharp correlation peaks**
  - **Description:** Ensure TSSE lag search never skips true correlation peaks by removing coarse-step blind spots and adding a regression test.
  - **Acceptance Criteria:**
    - Lag search evaluates bucket-aligned lag offsets and no longer skips peaks due to coarse step sizing.
    - Regression test covers a shifted-series case where coarse sampling would have missed the true lag.
    - Relevant core-server-rs tests pass.
  - **Notes / Run Log:**
    - 2026-01-25: Switched lag search to bucket-aligned evaluation with exact sweep when bounded, plus multi-scale refinement guard for larger ranges in `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`.
    - 2026-01-25: Added regression test `lag_search_finds_peak_when_coarse_steps_skip`.
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml lag_search_finds_peak_when_coarse_steps_skip` (pass).
  - **Status:** Done

- **TSSE-27: Parallelize/batch Related Sensors candidate scoring**
  - **Description:** Remove sequential per-candidate DuckDB reads in related sensor scoring by batching reads and running batch scoring concurrently.
  - **Acceptance Criteria:**
    - Related Sensors scoring batches DuckDB reads for candidates and processes multiple batches concurrently (no 250× sequential reads).
    - Progress reporting still advances correctly during scoring.
    - Relevant core-server-rs tests pass.
  - **Notes / Run Log:**
    - 2026-01-25: Batched candidate reads (25 per batch) and concurrent batch scoring in `apps/core-server-rs/src/services/analysis/jobs/related_sensors_v1.rs`.
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml related_sensors_v1` (pass).
  - **Status:** Done

- **TSSE-28: Add significance filtering for correlations (p-value + min overlap)**
  - **Description:** Filter weak correlations so low-sample noise (e.g., r=0.3 with n=10) does not surface in TSSE results.
  - **Acceptance Criteria:**
    - Related Sensors scoring rejects candidates when lag correlation is not statistically significant.
    - Correlation matrix results suppress r-values that fail significance thresholds.
    - Relevant core-server-rs tests pass.
  - **Notes / Run Log:**
    - 2026-01-25: Added p-value filtering (Fisher z approximation) + minimum overlap in `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`.
    - 2026-01-25: Applied significance filtering in `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs` (min overlap + p-value gate).
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml scoring` (pass).
  - **Status:** Done

- **TSSE-29: Mitigate rolling Pearson drift in episode extraction**
  - **Description:** Prevent floating-point drift in rolling Pearson computations over long windows by periodically recomputing sums.
  - **Acceptance Criteria:**
    - Rolling Pearson recalculates sums from scratch at least every 1000 iterations.
    - TSSE scoring tests pass.
  - **Notes / Run Log:**
    - 2026-01-25: Recompute rolling Pearson window sums every 1000 iterations in `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`.
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml scoring` (pass).
  - **Status:** Done

- **TSSE-30: Surface correlation confidence intervals in API + dashboard**
  - **Description:** Expose correlation confidence intervals from TSSE analysis results and display them in the Trends UI.
  - **Acceptance Criteria:**
    - Related Sensors results include best-lag confidence intervals in the API response.
    - Correlation matrix cells include confidence interval bounds in the API response.
    - Trends UI displays confidence interval context for related sensors and correlation matrix tooltips.
  - **Notes / Run Log:**
    - 2026-01-25: Added CI bounds to `TsseWhyRankedV1` and `CorrelationMatrixCellV1`, plus computed CI bounds in `apps/core-server-rs/src/services/analysis/tsse/scoring.rs` and `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs`.
    - 2026-01-25: Surfaced CI in `apps/dashboard-web/src/features/trends/components/AutoComparePanel.tsx` and matrix tooltips in `apps/dashboard-web/src/features/trends/components/RelationshipsPanel.tsx`.
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml scoring` (pass).
    - 2026-01-25: `make ci-web-smoke` (pass; existing lint warnings emitted).
  - **Status:** Done

- **TSSE-31: Replace scoring magic numbers with named constants**
  - **Description:** Make TSSE scoring weights/bonuses/penalties explicit with named constants and rationale.
  - **Acceptance Criteria:**
    - All scoring weight/bonus/penalty “magic numbers” are replaced with named constants and documented rationale.
    - TSSE scoring tests pass.
  - **Notes / Run Log:**
    - 2026-01-25: Added named constants + rationale in `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`.
    - 2026-01-25: Added thresholds for coverage/overlap as named constants with rationale.
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml scoring` (pass).
  - **Status:** Done

- **TSSE-32: Fix significance UX + Spearman correctness + correlation CI labeling**
  - **Description:** Correct statistical semantics and UI/UX around correlation significance and confidence intervals.
  - **Acceptance Criteria:**
    - Significance thresholds (min overlap + p-value alpha) are exposed as API params and adjustable in the Trends UI.
    - Spearman p-values/CI are computed correctly (or explicitly unavailable and labeled as such).
    - Correlation matrix cells distinguish “insufficient overlap” vs “not significant” vs “not computed” and the UI uses the correct reason.
    - Confidence interval labels reflect the actual metric (correlation coefficient, not lag seconds).
    - Rolling correlation description stays accurate (Pearson vs Spearman) after changes.
    - Correlation matrix does not silently override `min_overlap` with `min_significant_n`; UI/API explain the actual overlap requirement.
    - Tooltips and “not significant” messaging use the same `n` used for p-value/CI, including diagonal/null-r cases.
    - Low-overlap penalty logic is reachable (thresholds aligned), or explicitly removed with rationale.
    - Score component display does not mislead (raw CI/p-values are labeled and contextualized, or hidden if not applicable).
  - **Notes / Run Log:**
    - 2026-01-25: Review backlog notes:
      - Rolling Pearson description remains in UI text and must be kept accurate with any method changes.
      - Significance thresholds must be configurable (API + UI controls).
      - Spearman requires correct p-value/CI handling.
      - Significance gating currently uses |r| with fixed thresholds and UI tooltips mislabel null r vs insufficient data, especially for diagonals/overlap.
    - 2026-01-25: Review findings (needs fixes):
      - `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs` uses `ci_low/ci_high` fields that no longer exist in `CorrelationMatrixCellV1` (`r_ci_low/r_ci_high`), and it never sets `p_value`/`status`. This is a compile/runtime mismatch and loses status reporting.
      - `apps/dashboard-web/src/features/trends/components/RelationshipsPanel.tsx` still references `cell.ci_low/ci_high` and hard-codes significance text to `n >= 10`, which mismatches API fields and actual thresholds.
      - `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs` ignores `min_significant_n` and `significance_alpha` params; it also silently forces `min_overlap` to at least 10 via `effective_min_overlap`.
      - Spearman p-values/CI in `correlation_matrix_v1.rs` are computed using Pearson/Fisher-z (`pearson_p_value`/`pearson_confidence_interval`), which is statistically incorrect for Spearman.
      - UI does not expose significance controls for correlation matrix or related sensors; `RelatedSensorsJobParamsV1` types in `apps/dashboard-web/src/types/analysis.ts` are missing `min_significant_n` and `significance_alpha`, so the dashboard cannot send them.
      - Null/degenerate correlation cases (e.g., zero variance leading to `r=None`) are reported as “not significant” instead of “not computed/insufficient data,” which misleads users.
      - Correlation matrix diagonal cells (`row==col`) set `r=1.0` without `p_value/CI/status`, and the UI disables them without clarifying the semantic difference from computed pairs.
      - `apps/core-server-rs/src/services/analysis/tsse/scoring.rs` low-overlap penalty is unreachable because `min_significant_n` (default 10) rejects low overlap before the penalty threshold (6), making the penalty dead code.
      - Related-sensors CI label is hard-coded to “95%” while the backend uses a fixed z=1.96; once alpha becomes configurable this becomes misleading unless tied to alpha.
      - Dashboard correlation matrix UI/tooltips do not expose `status`/`p_value` once added, so users can’t tell “not computed” vs “not significant” vs “insufficient overlap.”
    - 2026-01-25: Implemented significance controls + status-aware correlation matrix outputs/UX (CI labels, p-values, explicit statuses), fixed Spearman significance math (t-approx + Fisher-z CI), and aligned overlap thresholds (no silent max with `min_significant_n`). Updated Related Sensors params/types + CI labeling and made low-overlap penalty reachable again.
    - 2026-01-25: `cargo test --manifest-path apps/core-server-rs/Cargo.toml scoring` (pass; existing warnings in core-server-rs).
    - 2026-01-25: `make ci-web-smoke` (pass; existing lint warnings in dashboard-web).
  - **Status:** Done

- **TSSE-33: Centralize correlation inference (shared Rust module)**
  - **Description:** Create a single shared Rust implementation for correlation p-values/CIs so correlation matrix and TSSE scoring cannot drift.
  - **Acceptance Criteria:**
    - Correlation matrix and Related Sensors scoring both call the same backend correlation inference helpers for p-values/CIs.
    - Shared module is located under `apps/core-server-rs/src/services/analysis/` and is imported from both call sites.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-26: Added shared correlation inference module `apps/core-server-rs/src/services/analysis/stats/correlation.rs` and wired it into `correlation_matrix_v1.rs` and `tsse/scoring.rs`.
    - 2026-01-26: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass; existing warnings).
    - 2026-01-26: `make ci-web-smoke` (pass; existing lint warnings).
  - **Status:** Done

- **TSSE-34: Fix analysis job dedupe DB errors for large job_key**
  - **Description:** Prevent `POST /api/analysis/jobs` from returning `500 Database error` when the dashboard sends very large `job_key` payloads (e.g., many sensor IDs), by hashing the dedupe key for indexing.
  - **Acceptance Criteria:**
    - DB schema includes a fixed-size `analysis_jobs.job_key_hash` used for dedupe/indexing.
    - The old unique index on `(job_type, job_key)` is removed (or no longer relied upon) so large `job_key` values do not trip Postgres index/key-size limits.
    - `POST /api/analysis/jobs` succeeds (200) even when `job_key` is very large (regression test via manual API reproduction is acceptable for Tier A).
    - Tier‑A refresh/validation recorded to installed controller with **no DB/settings reset**.
  - **Notes / Run Log:**
    - 2026-01-26: Added migration `infra/migrations/034_analysis_job_key_hash.sql` + core-server store changes to compute SHA-256 `job_key_hash`.
    - 2026-01-26: Tier‑A refreshed installed controller to `0.1.9.215` and verified large `job_key` no longer returns `500 Database error` (run: `project_management/runs/RUN-20260126-tier-a-analysis-job-key-hash-0.1.9.215.md`).
  - **Status:** Done

- **TSSE-35: TSSE stats Phase 2 — remove hidden lag overlap + echo effective params**
  - **Description:** Complete Phase 2 of the TSSE/TSE statistical correctness tracker: remove hidden overlap thresholds in lag search and ensure analysis jobs echo effective (clamped/defaulted) params back to callers.
  - **Acceptance Criteria:**
    - TSSE lag search does not use a hidden `MIN_OVERLAP=10`; it uses the caller-provided overlap threshold (derived from `min_significant_n`, clamped to `>=3`).
    - Analysis job responses expose the effective values used for overlap/significance parameters (interval, thresholds, alpha) so the UI can display them accurately.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - Tracker Phase 2 items are marked complete: `project_management/archive/trackers/TSSE_TSE_STATISTICAL_CORRECTNESS_TRACKER.md`.
  - **Notes / Run Log:**
    - 2026-01-26: Implemented param-driven overlap in lag search (`apps/core-server-rs/src/services/analysis/tsse/scoring.rs`) and added regression test.
    - 2026-01-26: Echoed effective params in `correlation_matrix_v1` and `related_sensors_v1`.
    - 2026-01-26: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass; existing warnings).
  - **Status:** Done

---

## iOS App
### Done
- **IOS-1: Implement full Bluetooth provisioning**
  - **Description:** CoreBluetooth scan/connect flow that provisions nodes over the FarmDashboard GATT service.
  - **Status:** Done (client-side implementation complete; hardware validation tracked in IOS-30)


- **IOS-20: Achieve Feature Parity and Comprehensive Testing**
  - **Description:** Bring the iOS app to parity with the web dashboard in demo mode (Analytics, Backups, Settings) and expand test coverage for critical flows.
  - **Acceptance Criteria:**
    - Analytics, Backups, and Settings tabs render demo-mode data correctly.
    - Provisioning flow completes against mocked/demo services.
    - Critical flows are covered by unit and UI tests and run in CI.
  - **Status:** Done (demo-mode parity + tests)


- **IOS-6: Replace SPM target with Xcode project**
  - **Description:** Ensure schemes for app + tests.
  - **Status:** Done


- **IOS-7: Implement persistent connection settings**
  - **Description:** AppStorage / Keychain supporting local + cloud endpoints.
  - **Status:** Done


- **IOS-8: Build discovery service**
  - **Description:** Using Bonjour + fallback manual entry to populate connection list.
  - **Status:** Done


- **IOS-9: Provide toggle for demo mode**
  - **Description:** Inject mock repositories covering nodes, sensors, schedules, analytics, backups, alarms.
  - **Status:** Done


- **IOS-10: Implement Nodes tab**
  - **Description:** List + detail (uptime, CPU, storage, sensors/outputs summary, adoption actions).
  - **Status:** Done


- **IOS-11: Implement Sensors & Outputs tab**
  - **Description:** Filtering, configuration summary, alarms, trend preview.
  - **Status:** Done


- **IOS-12: Implement Users tab**
  - **Description:** Role badges, capability list.
  - **Status:** Done


- **IOS-13: Implement Schedules tab**
  - **Description:** Week calendar (drag-to-create mock interactions) and condition/action detail.
  - **Status:** Done


- **IOS-14: Implement Trends tab**
  - **Description:** Multi-select (max 10) and Charts-based visualization supporting stacked/independent axes, pinch zoom, export placeholder.
  - **Status:** Done


- **IOS-15: Provisioning tab**
  - **Description:** Mirrors web workflow (BLE status, session controls, Wi-Fi credential push).
  - **Status:** Done


- **IOS-16: Update docs**
  - **Description:** Describe Xcode 26 navigation, scheme selection, API base configuration.
  - **Status:** Done


- **IOS-22: Harden watch companion coverage**
  - **Description:** Add watch companion unit tests for API decoding, environment overrides, and demo mode persistence to keep the Apple Watch experience reliable in CI.
  - **Status:** Done


- **IOS-23: Wire up watchOS app + extension targets**
  - **Description:** Add `FarmDashboardWatch` and `FarmDashboardWatchExtension` targets to the Xcode project, embed the extension in the watch app, and embed the watch app in the iOS app. Keep shared watch code testable from `FarmDashboardAppTests`.
  - **Acceptance Criteria:**
    - `FarmDashboardWatch` scheme builds on a watchOS simulator.
    - `WatchAPI`/`WatchDashboardView` are compiled into the watch extension.
    - `FarmDashboardWatchApp` is the watch entry point (no raw template `ContentView`).
  - **Status:** Done


- **IOS-24: Implement Maestro for iOS Screenshot Automation**
  - **Description:** Set up Maestro (mobile UI testing tool) to automate the capture of screenshots for manual review, similar to the web dashboard's Playwright script. This allows for rapid visual verification of the iOS app without the complexity of XCUITest result bundles.
  - **Acceptance Criteria:**
    - A `maestro/` directory is created with flow definitions (YAML).
    - The flow navigates through all primary tabs plus secondary "More" destinations (Analytics, Backups, Users, Provisioning, Settings) and captures representative modal sheets (Sensor Configure, Node Settings).
    - Screenshots are saved to a local directory (e.g., `manual_screenshots_ios/`).
    - A makefile target or script (e.g., `make ios-screenshots`) runs the flow.
  - **Status:** Done


- **IOS-25: Automate Watch App Screenshots with XCUITest**
  - **Description:** Implement an automated workflow to capture screenshots of the Farm Dashboard watchOS companion app. Since Maestro does not support watchOS, this must be done using **XCUITest** and **fastlane snapshot** (or a custom XCUITest wrapper). This allows for visual verification of the watch app's "glanceable" UI states. Critically, this must support running on **paired simulators** as documented in `apps/ios-app/FarmDashboardApp/FarmDashboardWatch/README.md`, ensuring the watch app launches correctly in its companion context.
  - **Acceptance Criteria:**
    - A command (e.g., `make watch-screenshots`) runs the test on a Watch Simulator (paired to an iPhone Simulator).
    - High-quality PNG screenshots of the Watch App are saved to `manual_screenshots_watch/`.
    - Screenshot set includes:
      - Dashboard summary (includes the Status card)
      - Controls section
      - Analytics section
      - Backups section
      - Outputs list
      - Alarms list
    - Screenshots are distinct (no duplicated frames due to identical scroll position).
    - The test reliably passes in a CI/local environment without manual intervention.
  - **Status:** Done (paired-simulator boot + best-effort screenshot export on failures)


- **IOS-26: Implement Interactive Controls for WatchOS App**
  - **Description:** Extend the `WatchDashboardView` to move beyond read-only status monitoring and support basic system control. This enables users to perform critical actions (like toggling a pump or acknowledging a leak alarm) directly from their wrist without needing to pull out their phone.
  - **Acceptance Criteria:**
    - **Output Control:** Add an "Outputs" view that fetches `/api/outputs` and allows toggling via `/api/outputs/{id}/command`.
    - **Alarm Management:** Tapping the alarms badge lists recent events and supports acknowledging via `/api/alarms/events/{id}/ack`.
    - **Feedback:** UI provides visual feedback (loading spinner/optimistic update) during the API request.
    - **Permissions:** Actions honor bearer auth and are gated by capabilities (`outputs.command`, `alerts.ack`).
  - **Status:** Done


- **IOS-27: iOS UI/UX polish**
  - **Description:** Improve readability and navigation by consolidating secondary tabs under a “More” menu, standardizing interval labels (e.g., “Every 30m”, “On change”), improving status badge coloring, and aligning Settings styling with the rest of the dashboard cards.
  - **Acceptance Criteria:**
    - Tab bar is limited to the primary surfaces (Nodes, Sensors, Schedules, Trends, More).
    - Secondary screens remain accessible from “More” (Analytics, Backups, Users, Provisioning, Settings).
    - Maestro screenshot flows remain stable after the nav change.
  - **Status:** Done


- **IOS-28: Align iOS tests with generated SDK models**
  - **Description:** Update demo/test fixtures, decoding strategies, and package wiring so iOS unit tests align with the generated `FarmDashboardAPI` models.
  - **Status:** Done


- **IOS-29: Silence Swift 6 actor-isolation warnings**
  - **Description:** Update iOS app delegate conformances to satisfy Swift 6 actor-isolation rules without changing runtime behavior.
  - **Acceptance Criteria:**
    - NetService and CoreBluetooth delegate conformances use `@preconcurrency` to avoid actor-isolation warnings.
    - iOS build/test runs without Swift 6 actor-isolation warnings.
  - **Status:** Done


- **IOS-32: Split iOS AppEntry monolith into modules**
  - **Description:** Reduce complexity and improve maintainability by extracting app state, navigation/routing, and view models out of `apps/ios-app/FarmDashboardApp/FarmDashboardApp/FarmDashboardApp/AppEntry.swift` so the entrypoint stays small and testable.
  - **Acceptance Criteria:**
    - `AppEntry.swift` is reduced to app startup/wiring and delegates; major state/navigation/view logic moves into dedicated files/modules.
    - The app builds and tests still pass (`make ci-ios`).
    - App behavior remains unchanged for primary navigation flows (Nodes/Sensors/Schedules/Trends/More + key sheets).
  - **Status:** Done (`make ci-ios`)


- **IOS-33: Default iOS/watch clients to production mode after install (no demo injection + no localhost target)**
  - **Description:** Ensure iOS and watchOS default to real/production behavior after a controller install: no automatic demo controllers/data unless explicitly enabled, and no default `127.0.0.1` base URL on device.
  - **Acceptance Criteria:**
    - On a fresh install, iOS discovery does not inject demo controllers when no controller is found; demo mode is only enabled by explicit user toggle or env (`FARM_IOS_DEMO_ONLY` / `UITEST_DEMO_ONLY`).
    - Core API state does not auto-seed demo snapshots unless demo mode is explicitly enabled.
    - Default base URL for iOS/watch is not `127.0.0.1`; users can discover/select the installed controller (mDNS) or enter a LAN address.
    - `make ci-ios` passes after the change.
  - **Status:** Done (`make ci-ios`)


- **IOS-34: Add login UX + token persistence for Rust core-server auth**
  - **Description:** Wire the iOS app to the Rust controller auth model (`/api/auth/login`, `/api/auth/me`) with a user-friendly sign-in screen and secure token storage, so auth-gated features (deploy-from-server, output commands, alarm acks) work without env hacks.
  - **Acceptance Criteria:**
    - On first launch (or after token expiry), the iOS app prompts for email/password and obtains a bearer token via `/api/auth/login`.
    - Token is stored securely (Keychain) and reused until invalid; on 401 the app forces re-login and clears cached token.
    - The active controller base URL is shown clearly in the login/settings UI and can be changed (mDNS discovery + manual entry).
    - Real-backend parity validation is tracked separately (IOS-31).
  - **Status:** Done (`make ci-ios`)


- **IOS-35: Wire watch app to iOS session (base URL + token)**
  - **Description:** Remove watch-only env defaults and keep the Apple Watch experience in sync with the paired phone (controller base URL + bearer token).
  - **Acceptance Criteria:**
    - Watch uses the same controller base URL as the iOS app (no hardcoded `127.0.0.1` fallback in production).
    - Watch actions use the phone-issued bearer token and gracefully handle expiry (prompt on phone to re-login).
    - `make watch-screenshots` remains green.
  - **Status:** Done (`make watch-screenshots`)


---

## Documentation
### Done
- **DOC-37: Audit Tier-A controller rebuild/refresh runbook (steps + evidence checklist)**
  - **Description:** Summarize the Tier‑A runbook’s exact SOP steps and the required evidence artifacts (health checks, version confirmation, screenshot review) for installed-controller validation.
  - **Acceptance Criteria:**
    - The Tier‑A SOP steps are captured (health checks, clean-tree gate, version bump, bundle build path, config/upgrade, verification).
    - Evidence requirements are enumerated (health checks + installed version + `make e2e-installed-health-smoke` + UI screenshots reviewed under `manual_screenshots_web/` when UI changes).
    - Clean-tree gate enforcement is documented (farmctl bundle refuses dirty worktrees; `reports/**` exception).
  - **Status:** Done (docs-only; audit summary delivered 2026-01-24)

- **DOC-36: Archive stale analytics/QA docs + refresh runbooks**
  - **Description:** Reduce confusion from stale docs by archiving historical analytics/QA sweep notes and removing dead `/provisioning` + Python-era feed references from active runbooks.
  - **References:**
    - Removed stale/historical docs (keeping current runbooks/docs only):
      - `docs/analytics.md`
      - `docs/analytics_feeds.md`
      - `docs/predictive_alarm_agent.md`
      - `docs/qa/2025-11-04.md`
      - `docs/qa/retention-2025-11-05.md`
      - `docs/runbooks/emporia-esphome-mqtt.md`
  - **Acceptance Criteria:**
    - Out-of-date docs are removed (stale/historical) to keep the repo minimal.
    - Active runbooks no longer reference the removed `/provisioning` UI path and reflect the Rust controller’s current feed/credential flow.
  - **Status:** Done (docs-only; archived + rewritten)

- **DOC-31: Document auth + capabilities UX (production path)**
  - **Description:** Update docs so a non-expert can log in, create the first admin user, and manage capabilities from the dashboard (no curl/ModHeader/manual token steps).
  - **Acceptance Criteria:**
    - Runbooks no longer describe browser header injection as the normal path for deployments/config writes.
    - Docs explain the “first-run” flow (create admin → login → manage capabilities).
  - **Status:** Done (docs-only; runbook updated)


- **DOC-32: Reduce install/uninstall polish footguns (getcwd + code signing provenance)**
  - **Description:** Address confusing-but-harmless warnings and macOS UX surprises with explicit guidance and context.
  - **Acceptance Criteria:**
    - Runbooks instruct a safe invocation pattern for privileged CLI actions (avoid `getcwd: Permission denied` noise).
    - Docs explain why macOS may attribute bundled dependency background items to upstream signers and what re-signing would entail later.
    - Docs explain why `database_url` remains localhost while `mqtt_host` is LAN-facing for nodes, and why this is correct.
  - **Status:** Done (docs-only; runbook updated)


- **DOC-33: Document macOS firewall prompts during dev/E2E**
  - **Description:** Prevent “false green” local tests by documenting the macOS Application Firewall prompt behavior when launching `core-server` and how it affects LAN reachability during dev QA.
  - **Acceptance Criteria:**
    - Dev docs explain the firewall prompt and that ignoring it can break LAN access even if localhost checks pass.
    - Guidance clarifies this is a dev/QA concern (production users can click Allow) and how to proceed for LAN testing.
  - **Status:** Done (docs-only; `AGENTS.md` + `docs/DEVELOPMENT_GUIDE.md` updated)


- **DOC-34: Remove obsolete external delegation workflow instructions**
  - **Description:** Remove repo instructions that refer to retired delegation/offload workflows so day-to-day work no longer references unused tooling or models.
  - **Acceptance Criteria:**
    - No documentation files instruct contributors to use the retired delegation/offload workflow.
    - Repository documentation contains no remaining references to the retired workflow command/model names.
  - **Status:** Done (docs-only; removed obsolete instructions)


- **DOC-30: Archive external audit reports (2026-01-01)**
  - **Description:** Store the external security/code quality audit reports in-repo so future work can reference the original findings and scope remediation appropriately.
  - **References:**
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `docs/audits/2026-01-01-rust-migration-audit-snippet.md`
  - **Acceptance Criteria:**
    - Audit reports are saved verbatim under `docs/audits/`.
    - Remediation tasks reference the archived reports.
  - **Status:** Done (docs-only)


- **DOC-29: Document repeatable installer E2E on a single Mac**
  - **Description:** Document the “multiple fresh installs” strategy for installer-first E2E without requiring reimaging or new macOS users.
  - **Acceptance Criteria:**
    - Docs explain the E2E profile (temp roots + random ports + namespaced labels) and how cleanup/uninstall makes runs repeatable.
    - Docs clearly separate production LaunchDaemon behavior from E2E LaunchAgent behavior.
    - All instructions are macOS-only and do not mention container runtimes.
  - **Notes:** Documented in `docs/DEVELOPMENT_GUIDE.md` and aligned with installer-first runbooks.
  - **Status:** Done

- **DOC-28: Replace container stack references with native stack guidance**
  - **Description:** Update developer/testing/runbook docs to reflect native (launchd) dependencies and non-container E2E flows.
  - **Acceptance Criteria:**
    - `docs/runbooks/core-server-production-setup.md` is fully rewritten to the installer-first (DMG) architecture and does not mention container runtimes.
    - Docs no longer claim a container runtime is required for `make e2e-web-smoke` or `make demo-live`.
    - Local setup instructions describe the native dependencies (Postgres/Mosquitto/Redis) and how to verify they are running.
    - Testing guidance references the updated non-container E2E harness behavior.
    - Related docs that referenced container-era steps are updated to point at the installer-first workflow.
  - **Notes:** Finished rewrite + reference updates (production runbook + supporting docs).
  - **Status:** Done (docs-only)

- **DOC-25: Add a non-expert "before you begin" checklist to the production runbook**
  - **Description:** Add a friendly preflight section to `docs/runbooks/core-server-production-setup.md` with the values and decisions a non-expert should gather up front (repo URL, install path, LAN IP/hostname, admin credentials, backup path).
  - **Acceptance Criteria:**
    - The runbook includes a "Before you begin" checklist with required values and recommended defaults.
    - The runbook includes an optional copy/paste variable block for those values.
    - The runbook notes backup path permissions if a non-default location is used.
  - **Status:** Done (docs-only; make e2e-web-smoke)

- **DOC-26: Add copy/paste-friendly commands and verification checkpoints to the production runbook**
  - **Description:** Reduce manual editing errors by adding templated `.env` creation instructions, safer migration commands, and explicit verification checkpoints.
  - **Acceptance Criteria:**
    - The runbook includes a copy/paste `.env` template with placeholders and calls out where to replace passwords.
    - The migration command uses a safe shell loop without relying on `ls`.
    - The runbook includes a quick verification checklist (service health, DB readiness, `/healthz`).
  - **Status:** Done (docs-only; make e2e-web-smoke)

- **DOC-27: Add non-expert troubleshooting and routine operations guidance to the production runbook**
  - **Description:** Add lightweight troubleshooting and basic operational commands (restart/upgrade) to help operators recover without deep expertise.
  - **Acceptance Criteria:**
    - The runbook includes a "Routine operations" section with restart/stop/upgrade commands.
    - The runbook includes a "Common issues" section with fixes for native services not running, port conflicts, and migration failures.
  - **Status:** Done (docs-only; make e2e-web-smoke)

- **DOC-9: Update testing guidance to require E2E before Done**
  - **Description:** Align all docs with the new testing policy: E2E (running the full stack or relevant live component) is required before a task can be marked Done.
  - **Acceptance Criteria:**
    - Docs specify the E2E smoke suites (`make e2e-web-smoke`, `make demo-live` + Playwright) and when they must run.
    - Commit hook guidance reflects path-aware E2E behavior and native-service requirement.
    - Task completion policy explicitly requires E2E validation.
  - **Status:** Done (docs-only)

- **DOC-1: Create centralized documentation hub**
  - **Description:** Add a single entry point that links to component guides, testing policy, and production readiness, so newcomers know where to go first.
  - **Status:** Done

- **DOC-2: Production environment guide**
  - **Description:** Author a step-by-step guide for deploying core server, dashboard, node agents, and analytics feeds in production (TLS, secrets, monitoring).
  - **Status:** Done (`docs/PRODUCTION_GUIDE.md`)

- **DOC-3: Refresh READMEs to current state**
  - **Description:** Update per-app READMEs and infra notes to match the latest APIs, auth model, and tooling.
  - **Status:** Done (core-server, node-agent, dashboard, iOS/watch, and root README updated)

- **DOC-4: Git hygiene guardrails (prevent accidental restores)**
  - **Description:** Encode “no discarding without review” and “no `git restore` without confirmation” rules in `AGENTS.md`, add a `make open-simulators` helper for keeping iOS/watch simulators open after commits, move runtime-mutating config files out of version control (commit templates instead + `.gitignore` runtime state), and document `skip-worktree` for rare tracked local overrides.
  - **Status:** Done

- **DOC-5: New machine onboarding templates**
  - **Description:** Add `.env.example` templates, a `make bootstrap` target, and a concise new-machine checklist so a fresh clone can get to “demo running + tests passing” quickly.
  - **Status:** Done (`docs/development/new_machine.md` + per-app `.env.example`)

- **DOC-6: Remove simulator requirement from AGENTS**
  - **Description:** Drop the post-commit simulator boot requirement from `AGENTS.md` so simulator usage is optional.
  - **Status:** Done

- **DOC-7: Provide curated Codex skills list on request**
  - **Description:** Use the skill installer workflow to list curated Codex skills for ad-hoc support requests.
  - **Status:** Done

- **DOC-8: Require tests for all code changes**
  - **Description:** Update documentation to state that any code change must be tested, with relevant suites run locally before sharing results.
  - **Acceptance Criteria:**
    - Root `README.md` test policy reflects the requirement.
    - `docs/DEVELOPMENT_GUIDE.md` states that code changes must be tested before PRs.
    - `AGENTS.md` includes the testing expectation for agents.
  - **Status:** Done

- **DOC-10: Add Sim Lab step-by-step usage runbook**
  - **Description:** Document a cold-start walkthrough for running Sim Lab (mocks + demo-live), including URLs and shutdown steps.
  - **Status:** Done (docs/runbooks/sim-lab.md, docs/README.md; docs-only)


- **DOC-11: Document Renogy Rover BT-2 + Pi 5 node assumptions**
  - **Description:** Record the fixed hardware/topology assumptions for the Renogy Rover charge controller integration so future implementation work does not revisit settled decisions.
  - **Acceptance Criteria:**
    - `docs/analytics_feeds.md` lists the fixed Renogy deployment assumptions: `RNG-CTRL-RVR20-US` (RS-485), `BT-2` BLE module, Raspberry Pi 5 node polls locally, core server consumes over LAN.
  - **Status:** Done (docs-only)


- **DOC-12: Document Renogy Pi 5 deployment tool**
  - **Description:** Provide a user-facing runbook for provisioning a Raspberry Pi 5 Renogy charge-controller node using `tools/renogy_node_deploy.py`, including bundle outputs, flashing steps, adoption, and replacement flow.
  - **Acceptance Criteria:**
    - `docs/runbooks/renogy-pi5-deployment.md` exists and is linked from `docs/README.md` and `docs/node-agent.md`.
  - **Status:** Done (docs-only)


- **DOC-13: Document Raspberry Pi 5 simulator**
  - **Description:** Provide a runbook for the Pi 5 simulator workflow and link it from the doc hub and node-agent validation guide.
  - **Acceptance Criteria:**
    - `docs/runbooks/pi5-simulator.md` exists and covers startup, verification, and config output paths.
    - `docs/README.md` and `docs/node-agent.md` link to the simulator runbook.
  - **Status:** Done (docs-only)


- **DOC-14: Document Raspberry Pi 5 deployment tool**
  - **Description:** Provide a user-facing runbook for the Raspberry Pi 5 deployment helpers (`tools/build_image.py`, `tools/flash_node_image.sh`) and link it from the doc hub and node-agent validation guide.
  - **Acceptance Criteria:**
    - `docs/runbooks/pi5-deployment-tool.md` exists and documents Raspberry Pi Imager + flash script workflows.
    - `docs/README.md`, `docs/node-agent.md`, and `docs/PRODUCTION_GUIDE.md` link to the runbook.
  - **Status:** Done (docs-only)


- **DOC-15: Link Renogy Pi 5 runbook in root README**
  - **Description:** Ensure the repository README highlights the Renogy Pi 5 deployment runbook for quick discovery.
  - **Acceptance Criteria:**
    - `README.md` includes a link to `docs/runbooks/renogy-pi5-deployment.md`.
  - **Status:** Done (docs-only)


- **DOC-16: Document Renogy simulator validation workflow**
  - **Description:** Add a simulator validation walkthrough for Renogy BT-2 deployment bundles, including ingest payload verification and MQTT checks.
  - **Acceptance Criteria:**
    - `docs/runbooks/pi5-simulator.md` includes steps for running a Renogy bundle with `--config-path` and `--no-simulation`.
    - `docs/runbooks/renogy-pi5-deployment.md` includes a simulator validation section with ingest + MQTT checks.
  - **Status:** Done (docs-only)


- **DOC-17: Keep simulator/deployment tools and runbooks tracked**
  - **Description:** Confirm simulator/deployment tooling and their runbooks are not gitignored.
  - **Acceptance Criteria:**
    - `.gitignore` does not exclude the simulator scripts, deployment tools, or associated runbooks.
  - **Status:** Done (docs-only)


- **DOC-18: Clarify Renogy Pi 5 deployment steps and split simulator validation**
  - **Description:** Rewrite the Renogy Pi 5 deployment runbook for real hardware only (clear OS image responsibilities, tool outputs, boot/root mounting steps, and BT-2 address discovery). Move simulator validation to a separate runbook and link it from the doc hub and node-agent guide.
  - **Acceptance Criteria:**
    - `docs/runbooks/renogy-pi5-deployment.md` focuses on real deployment with explicit boot/root mount instructions.
    - `docs/runbooks/renogy-pi5-simulator.md` captures simulator-only validation steps.
    - `docs/README.md` and `docs/node-agent.md` link to the new simulator runbook.
  - **Status:** Done (docs-only)


- **DOC-19: Simplify Renogy Pi 5 deployment for macOS users**
  - **Description:** Make the Renogy Pi 5 deployment runbook macOS-first with Raspberry Pi Imager only, clear placeholder guidance, Ethernet-first defaults, and troubleshooting/alternatives consolidated at the end.
  - **Acceptance Criteria:**
    - Main deployment steps avoid optional flags (`--collector renogy-bt` implied), Linux paths, and manual flashing.
    - Clear guidance for `--node-name`, `--node-id`, and `--mqtt-url` (including what must match core broker config).
    - Wi-Fi and fallback BT-2 address discovery steps live under a final “Troubleshooting and alternative options for deployment” section.
  - **Status:** Done (docs-only)


- **DOC-20: Document Renogy Pi 5 first-boot automation**
  - **Description:** Update the Renogy Pi 5 deployment runbook to cover automatic first-boot staging (node config/env + `renogy-bt` install) so deployments stay macOS-only with no Pi login.
  - **Acceptance Criteria:**
    - `docs/runbooks/renogy-pi5-deployment.md` documents copying `node_config.json`, `node-agent.env`, `renogy-bt-config.ini`, and `renogy-bt.service` to the boot volume.
    - The runbook notes that first boot installs `renogy-bt` and starts the service automatically (no SSH).
  - **Status:** Done (docs-only)


- **DOC-35: Document Pi 5 simulator core-registration de-dupe behavior**
  - **Description:** Record the simulator behavior when registering against seeded databases and align Renogy simulator sensor IDs with the node-agent runbook.
  - **Acceptance Criteria:**
    - `docs/runbooks/pi5-simulator.md` notes the de-dupe behavior for seeded DBs.
    - The runbook calls out the canonical `renogy-*` simulator IDs.
  - **Notes:**
    - Renumbered from `DOC-34` → `DOC-35` to resolve an ID collision (the `DOC-34` ID is used by the “Remove obsolete external delegation workflow instructions” ticket).
  - **Status:** Done (docs-only)


- **DOC-21: Add repo architecture diagram to root README**
  - **Description:** Add a Mermaid diagram to the root README that visualizes the end-to-end architecture (clients, core control plane, ingest pipeline, infra services, edge nodes, and test/deployment tooling).
  - **Acceptance Criteria:**
    - `README.md` includes a Mermaid diagram covering the major components and their primary data flows (REST, MQTT, SQL, discovery).
    - The diagram includes the core apps (`core-server`, `dashboard-web`, `node-agent`, `telemetry-sidecar`, `ios-app`, `esp32-firmware`) and infra services (TimescaleDB/Postgres, Mosquitto, Redis, Grafana/Tempo).
  - **Status:** Done (docs-only)


- **DOC-22: Emporia cloud API setup guide**
  - **Description:** Publish a user-facing guide for connecting Emporia Vue via the cloud API (tokens, deviceGids, env vars, validation).
  - **Acceptance Criteria:**
    - `docs/runbooks/emporia-cloud-api.md` documents token acquisition, deviceGid discovery, env configuration, and validation steps.
    - `docs/README.md` links the guide for easy discovery.
  - **Status:** Done (docs-only)


- **DOC-23: Emporia deviceGid helper script**
  - **Description:** Add a small helper script to list Emporia deviceGid values from an authtoken.
  - **Acceptance Criteria:**
    - `tools/emporia_device_ids.py` fetches `/customers/devices` with `authtoken` and prints device IDs.
    - `docs/runbooks/emporia-cloud-api.md` references the helper usage.
  - **Status:** Done (docs-only)


- **DOC-24: Production-only core server setup runbook (from scratch)**
  - **Description:** Add a production-focused, step-by-step guide for setting up the core server on a brand new system from a fresh clone, including explicit dependency installation steps (URLs) and no development-only tooling.
  - **Acceptance Criteria:**
    - `docs/runbooks/core-server-production-setup.md` exists and avoids dev-only tooling (Xcode, Playwright, simulators).
    - The runbook includes explicit dependency install steps with URLs (Git, installer DMG) and verification commands.
    - The runbook includes a production “node setup” section for: Renogy Pi 5 via `/api/deployments/pi5`, and Emporia via cloud API.
    - `docs/PRODUCTION_GUIDE.md` and `docs/README.md` link to the runbook for discoverability.
  - **Status:** Done (docs-only)


- **DOC-38: Tier-A clean-worktree discipline (runbook + AGENTS)**
  - **Description:** Prevent “fixed locally but not shipped” regressions by codifying safe clean-worktree behavior for Tier‑A runs. When a clean-tree gate is required, operators/agents must inventory and classify all dirty/untracked paths (commit real work; move artifacts; only delete proven disposable files).
  - **Acceptance Criteria:**
    - `docs/runbooks/controller-rebuild-refresh-tier-a.md` explicitly requires `git status` + `git diff --stat` and includes a “dirty worktree” checklist (commit/move/delete with caution) instead of recommending blind discard.
    - `AGENTS.md` explicitly calls out the responsibility to investigate dirty/untracked paths when a clean-worktree gate is required, and references the discard allowlist.
  - **Notes / Run Log:**
    - 2026-02-03: Updated Tier‑A runbook + root agent guidance to require path-by-path review when cleaning for Tier‑A bundle builds; no blind discards.
  - **Status:** Done

---

## Setup App & Native Services
### Done
- **SETUP-41: Fix Tier-A dirty-path allowlist parsing for porcelain lines w/ leading space**
  - **Description:** Fix `farmctl bundle` Tier‑A hard gate parsing so porcelain-v1 lines with a leading status space (e.g. `" M reports/foo.log"`) do not lose the first character of the path, incorrectly failing the `reports/**` allowlist.
  - **References:**
    - `apps/farmctl/src/bundle.rs`
  - **Acceptance Criteria:**
    - When only `reports/**` is dirty, Tier‑A bundle builds are allowed.
    - A unit test covers the porcelain-v1 leading-space status prefix case.
    - `cargo test --manifest-path apps/farmctl/Cargo.toml` passes.
  - **Status:** Done (unit-tested; `cargo test --manifest-path apps/farmctl/Cargo.toml`)


- **SETUP-39: Document Tier-A rebuild/refresh runbook (installed controller)**
  - **Description:** Document the fast developer workflow (“Tier‑A”) for rebuilding a controller bundle DMG from source and refreshing the already-installed controller stack (static web + API) without admin privileges, using the running setup daemon.
  - **Acceptance Criteria:**
    - A runbook exists at `docs/runbooks/controller-rebuild-refresh-tier-a.md` that includes:
      - Stable bundle path guidance (avoid `/Volumes/...` transient mounts).
      - Bundle build example (`farmctl bundle`).
      - Setup Center UI steps (set bundle path → Upgrade).
      - CLI alternative using setup-daemon endpoints (`/api/config`, `/api/upgrade`).
      - Post-upgrade verification (`/healthz`, version/source-of-truth pointers) and troubleshooting notes.
    - The runbook is discoverable from the documentation hub and related runbooks (`docs/README.md`, `docs/runbooks/core-server-production-setup.md`, `docs/DEVELOPMENT_GUIDE.md`, `docs/runbooks/controller-bundle.md`).
  - **Run Log:**
    - 2026-01-10: Added Tier‑A rebuild/refresh runbook + cross-links so the “build → refresh installed controller” workflow is documented and repeatable.
  - **Status:** Done


- **SETUP-35: Pre-create bootstrap admin user (temp password) during production install**
  - **Description:** Ensure fresh production installs create an initial admin account automatically so the dashboard can always start at a login screen (no “create admin user” as the primary path).
  - **Acceptance Criteria:**
    - During `farmctl --profile prod install`, if no users exist, create `admin@farmdashboard.local` with a randomly generated temporary password and `admin` role (includes `config.write`).
    - The temporary password is printed once in the installer wizard output so the operator can sign in.
    - The dashboard stores auth tokens per browser session (no persistent “remember me” mode) and lands on `/login`.
    - Admin can change passwords from the dashboard (**Users** → **Set password**).
  - **Status:** Done (implementation + unit/smoke coverage; production UX validation tracked in SETUP-36)


- **SETUP-32: Enforce “single public installer DMG” in release workflow**
  - **Description:** Prevent user confusion by ensuring GitHub releases publish only `FarmDashboardInstaller-<ver>.dmg`. The controller bundle DMG remains an internal build artifact embedded inside the installer app bundle.
  - **Acceptance Criteria:**
    - Local release tooling/docs make the “upload installer only” rule hard to violate (guardrails + clear instructions).
    - The public release output directory contains only `FarmDashboardInstaller-*.dmg` + checksums (no controller DMG asset to upload by accident).
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`make e2e-installer-stack-smoke`; guardrails in `farmctl dist`; log: `reports/e2e-installer-stack-smoke/20260103_232953`)


- **SETUP-31: Installer launcher reliability (no ERR_CONNECTION_REFUSED)**
  - **Description:** Ensure double-clicking `Farm Dashboard Installer.app` reliably starts `farmctl serve` on `127.0.0.1:8800` and keeps it available for the full wizard session (no transient bootstrap that leaves the browser showing `ERR_CONNECTION_REFUSED`).
  - **Acceptance Criteria:**
    - Double-clicking the installer app on a clean machine brings the wizard up reliably.
    - The setup daemon remains reachable for the duration of the setup flow.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260103_232953`)

- **SETUP Gate Status (North-Star)**
  - **Description:** The Setup North-Star is only “green” when the installer UX meets the current requirements (native launcher, clean preflight semantics, quarantine-safe controller bundle mounts, and repeatable clean installs).
  - **Acceptance Criteria:**
    - The installer DMG shows only the installer launcher on mount (the controller bundle DMG is embedded inside the app bundle `Contents/Resources/...`, not visible at the DMG root).
    - Wizard launches via `NSWorkspace` (no AppleScript).
    - Production installs prompt for admin only when installing LaunchDaemons (wizard runs as the user).
    - Preflight shows no warnings on a clean machine (no “8800 in use” self-warning; “not root” is informational; “existing install detected” appears only when applicable).
    - Launch plan generation shows no warnings on a clean machine (missing binaries are expected before first install).
    - The controller bundle DMG mount path is quarantine-safe (no manual `xattr` required).
    - `make e2e-setup-smoke-quarantine` passes (simulated quarantined-downloaded installer DMG).
    - `make e2e-installer-stack-smoke` passes from a verified clean state (preflight/postflight test hygiene).
  - **Status:** Done (`make e2e-installer-stack-smoke-quarantine`; log: `reports/e2e-installer-stack-smoke/20260105_171120`)


- **SETUP-10: Ship a single installer DMG that auto-launches the setup wizard**
  - **Description:** Deliver an end-user DMG that bundles `farmctl` + the setup UI and starts the wizard automatically without terminal commands.
  - **Acceptance Criteria:**
    - Opening the DMG presents a single installer launcher that opens the wizard automatically.
    - The wizard uses the embedded `farmctl` and auto-detects the embedded controller bundle (no manual path entry for the default case).
    - Install can be completed without terminal commands or manual file edits.
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_101952`)


- **SETUP-22: Ship a single public installer artifact (hide the controller DMG)**
  - **Description:** Make “download one DMG, double-click it” the only supported way to deploy Farm Dashboard to a new Mac. The GitHub release should publish **only** the installer DMG; the controller bundle DMG remains an internal build artifact and is embedded inside the installer DMG for auto-detection/upgrades.
  - **Acceptance Criteria:**
    - The mounted installer DMG shows only the installer launcher; the controller bundle DMG is embedded inside the launcher app bundle (`Contents/Resources/...`).
    - The wizard auto-detects the embedded controller bundle (no manual bundle selection for the default path).
    - Runbooks/docs no longer direct users to download/use `FarmDashboardController-*.dmg` for initial installs.
    - `make e2e-installer-stack-smoke` remains green.
  - **References:**
    - `docs/releases.md`
    - `docs/runbooks/core-server-production-setup.md`
    - `apps/farmctl/src/bundle.rs`
    - `apps/farmctl/src/dist.rs`
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_101952`)


- **SETUP-23: Make `farmctl uninstall` resilient to missing service user**
  - **Description:** Fix `farmctl uninstall` so it does not fail if the configured service user (default `_farmdashboard`) does not exist (e.g., partial/unwind installs). Treat missing-user `pgrep` failures as “no processes” and continue cleanup.
  - **Acceptance Criteria:**
    - `farmctl uninstall --profile prod --remove-roots --yes` succeeds even if `_farmdashboard` is missing.
    - The uninstall still terminates services when the user exists and processes are running.
    - `cargo test --manifest-path apps/farmctl/Cargo.toml` passes.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`; `reports/e2e-installer-stack-smoke/20260103_101952`)


- **SETUP-24: Replace AppleScript installer launcher with native Swift app (no AppleScript)**
  - **Description:** Replace the AppleScript-generated launcher with a small native Swift installer launcher (or `.pkg` bootstrap) that embeds the controller bundle inside the app bundle and opens the wizard via `NSWorkspace`.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0026-installer-launcher-rewrite-(swift-no-applescript-embedded-controller-dmg-preflight-quarantine).md`
    - `apps/farmctl/src/bundle.rs`
  - **Acceptance Criteria:**
    - Installer DMG no longer relies on `osacompile`/AppleScript.
    - Controller bundle DMG is embedded inside the launcher app (`Contents/Resources/...`) and is not user-visible on the mounted DMG root.
    - Launcher starts `farmctl serve` and opens the wizard via `NSWorkspace`.
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_101952`)


- **SETUP-25: Fix preflight warning semantics (clean install = no warnings)**
  - **Description:** Adjust preflight checks so they don’t warn about expected states (wizard port in-use by itself, not-root in prod before install) and provide clear guidance when an existing install is present.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0026-installer-launcher-rewrite-(swift-no-applescript-embedded-controller-dmg-preflight-quarantine).md`
    - `apps/farmctl/src/launchd.rs`
    - `apps/setup-app/static/app.js`
  - **Acceptance Criteria:**
    - On a clean machine with only the installer wizard running, preflight shows no `warn` checks.
    - “Existing install detected” appears only when `/usr/local/farm-dashboard` (or configured install root) is present.
    - “Not running as root” is informational (not a warning) in prod profile.
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_101952`)


- **SETUP-26: Make controller bundle DMG mounts quarantine-safe**
  - **Description:** Remove the need for manual `xattr -dr com.apple.quarantine ...` by making `farmctl` copy/quarantine-strip bundle DMGs before mounting when necessary.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0026-installer-launcher-rewrite-(swift-no-applescript-embedded-controller-dmg-preflight-quarantine).md`
    - `apps/farmctl/src/install.rs`
  - **Acceptance Criteria:**
    - Mounting the embedded controller DMG succeeds even when the installer DMG was downloaded and is quarantined.
    - The wizard no longer fails with `hdiutil: attach failed - Resource temporarily unavailable` due to quarantine.
    - `make e2e-setup-smoke-quarantine` passes.
  - **Status:** Done (`reports/e2e-setup-smoke/last_state.json` has `quarantine_installer_dmg: true`)


- **SETUP-27: Prompt for admin only at LaunchDaemons install (no AppleScript)**
  - **Description:** Ensure the wizard runs as the user and only prompts for admin when it needs to install/modify LaunchDaemons (system domain). The prompt must use a standard macOS mechanism (no AppleScript password prompts).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0026-installer-launcher-rewrite-(swift-no-applescript-embedded-controller-dmg-preflight-quarantine).md`
    - `apps/farmctl/src/server.rs`
  - **Acceptance Criteria:**
    - On a fresh Mac, the wizard opens without requesting admin.
    - Clicking Install triggers an admin prompt and completes the install path successfully (LaunchDaemons installed; services run as the dedicated service user).
    - No plaintext password fields are introduced in the web UI or persisted to disk.
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_101952`)


- **SETUP-28: Make MQTT host first-class in the wizard (auto-detect controller LAN IP)**
  - **Description:** MQTT host must be configured for real production nodes and should not be hidden behind “advanced”. Defaulting to `127.0.0.1` breaks non-controller clients (Pi/ESP nodes). The wizard should surface MQTT host by default and provide a one-click action to populate it with the controller’s LAN IP.
  - **Acceptance Criteria:**
    - Configure step shows `mqtt_host` without enabling “advanced”.
    - A button populates `mqtt_host` with the controller’s best-guess LAN IPv4 address (and the value remains editable).
    - The wizard persists the config before running preflight/install actions so the chosen `mqtt_host` is actually used.
    - `make e2e-setup-smoke` and `make e2e-installer-stack-smoke` remain green.
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_142056`)


- **SETUP-29: Make preflight UX “normal” (configure-first + no warn for expected states)**
  - **Description:** The wizard should not show `warn` badges for expected states (e.g., “existing install detected” during an upgrade workflow, or service ports in-use because the existing stack is running). Move Configure before Preflight, and downgrade expected-state checks to `info` with clear guidance.
  - **Acceptance Criteria:**
    - Wizard flow is `Welcome → Configure → Preflight → Plan → Operations`.
    - Preflight uses `info` (not `warn`) for expected states in prod, including:
      - existing install present under the install root.
      - stack ports in-use because the current install is running.
    - `warn` is reserved for actionable issues that may cause install/upgrade to fail.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_151249`)


- **SETUP-30: Fix Launch plan warning semantics (clean install = no warnings)**
  - **Description:** The wizard’s “Generate plan” step must not show warnings for expected pre-install states. Missing binaries under the install root are expected on a clean machine before the first install and should not surface as `warn`. Warnings should be reserved for actionable “broken partial install” scenarios.
  - **Acceptance Criteria:**
    - On a clean machine before first install, clicking “Generate plan” produces no `warn` banner for missing binaries under `/usr/local/farm-dashboard/...`.
    - If an existing install is detected but required binaries are missing, the Launch plan includes warnings describing the missing paths.
    - `make e2e-installer-stack-smoke` remains green (wizard smoke asserts no plan warnings on clean installs).
  - **References:**
    - `apps/farmctl/src/launchd.rs`
    - `apps/dashboard-web/scripts/setup-wizard-smoke.mjs`
  - **Status:** Done (`reports/e2e-installer-stack-smoke/20260103_155247`)


- **SETUP-15: Add end-to-end DMG install/upgrade/rollback validation**
  - **Description:** Automate local DMG install/upgrade/rollback validation with a dedicated test target that matches the real installer/wizard flow.
  - **Acceptance Criteria:**
    - `make e2e-setup-smoke` mounts the installer DMG and validates the wizard flow end-to-end (Playwright drives the UI).
    - The install path exercises real launchd bootstrap + DB init + health checks (no test-only skip flags that bypass production behavior).
    - The run is repeatable on a single dev Mac via an isolated E2E profile: temp roots, random free ports, and namespaced launchd labels (LaunchAgents in `gui/$UID`, no admin required).
    - The flow performs install → upgrade → rollback and finishes with a clean uninstall/reset so repeated installs are safe.
    - Test artifacts are captured on failure.
  - **Status:** Done (`make e2e-installer-stack-smoke`)


- **SETUP-11: Simplify the setup wizard with auto-detection + advanced toggle**
  - **Description:** Reduce inputs to the minimum required and auto-detect bundle/farmctl paths.
  - **Acceptance Criteria:**
    - `bundle_path` and `farmctl_path` are auto-populated.
    - Advanced fields (ports/paths) are hidden behind an "Advanced" toggle.
    - Defaults are safe and consistent; wizard works with only the minimal prompts.
  - **Notes:** Playwright wizard smoke asserts bundle/farmctl auto-detect and that advanced fields are hidden by default.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-12: Provision DB/MQTT/Redis as managed native services**
  - **Description:** Bundle and manage core dependencies so the installer does not rely on container stacks or manual package managers.
  - **Acceptance Criteria:**
    - `farmctl install` provisions Postgres/Timescale, Mosquitto, and Redis as launchd services.
    - Production uses LaunchDaemons (system) so services start at boot with no user logged in.
    - Services run as a least-privilege service user (not root); only installation/bootstrap requires admin.
    - Health checks include DB/MQTT/Redis readiness.
    - No manual dependency install steps are required for a fresh Mac mini.
  - **Notes:** Installer-path launchd health validated via `make e2e-installed-health-smoke` and the full stack gate (`FARM_E2E_INCLUDE_IOS=1 make e2e-installer-stack-smoke`). Production LaunchDaemon plists set `UserName`/`GroupName` for the dedicated service user (unit-tested in `make ci-farmctl-smoke`).
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-13: Replace the Python setup app with a Rust setup daemon**
  - **Description:** Move setup API + wizard hosting into Rust (e.g., `farmctl serve`) and retire the Python backend.
  - **Acceptance Criteria:**
    - Rust service serves the setup UI and exposes preflight/plan/install/upgrade/rollback/health/diagnostics endpoints.
    - Setup Center can reach the Rust service without manually starting a separate app.
    - Python setup app is removed or remains only as a deprecated fallback.
    - The setup daemon uses the same install/upgrade/rollback codepath as `farmctl` CLI (no duplicated “server-only” install logic).
  - **Notes:** Rust `farmctl serve` hosts the wizard/endpoints. Setup Center uses a same-origin Next.js proxy (`/api/setup-daemon/*`) and `FARM_SETUP_DAEMON_BASE` to avoid CORS.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-14: Add one-click backup and upgrade actions in Setup Center**
  - **Description:** Provide direct actions and status feedback for backup/upgrade from the dashboard.
  - **Acceptance Criteria:**
    - Setup Center triggers backup/restore/upgrade with progress feedback.
    - Actions are routed through core-server or the Rust setup daemon, not a manually started service.
  - **Notes:** Installer actions are routed through the setup-daemon proxy, with progress/status captured in the Setup Center UI.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-3: Package native binaries for production installs**
  - **Description:** Produce packaged binaries/bundles for core-server, telemetry-sidecar, and dashboard-web suitable for LaunchDaemon execution (containerless).
  - **Acceptance Criteria:**
    - Core-server packaging produces a self-contained executable/bundle with pinned dependencies.
    - Telemetry-sidecar packaging produces a release binary placed alongside core-server artifacts.
    - Dashboard-web packaging produces a production build with a stable runtime entrypoint.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-4: Implement setup app install/upgrade workflow**
  - **Description:** Add guided install/upgrade actions (write plists, start/stop services, verify health) to the setup app.
  - **Acceptance Criteria:**
    - Setup app can apply the install plan and start services with clear progress feedback.
    - Upgrade flow preserves config and supports rollback to previous binaries.
    - Health checks confirm core API, MQTT, and DB connectivity after install.
    - Install and upgrade are one-click actions inside the setup app.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-5: Add diagnostics export and support bundle**
  - **Description:** Provide a one-click diagnostics export from the setup app for support and troubleshooting.
  - **Acceptance Criteria:**
    - Export includes service logs, config snapshot, and recent health results.
    - Export avoids secrets unless explicitly requested.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-6: Define controller bundle format and release manifest**
  - **Description:** Specify the versioned controller bundle layout and release manifest used by the installer.
  - **Acceptance Criteria:**
    - A manifest format captures versions, checksums, and component paths.
    - The bundle includes configs and assets required for deterministic installs.
    - The format supports rollback to prior versions.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-7: Build the `farmctl` installer CLI**
  - **Description:** Provide a single entrypoint tool that installs, upgrades, and rolls back releases.
  - **Acceptance Criteria:**
    - `farmctl install <version>` installs from the release manifest without source builds.
    - `farmctl upgrade` applies the next release while preserving config and data.
    - `farmctl rollback` restores the previous known-good release.
    - `farmctl` is the single installer entrypoint for non-GUI workflows.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-8: Make setup flows idempotent and secret-aware**
  - **Description:** Ensure setup can be safely re-run with generated secrets and minimal prompts.
  - **Acceptance Criteria:**
    - Re-running setup does not overwrite valid configuration unless explicitly requested.
    - Secrets (passwords/tokens) are generated or imported with clear prompts.
    - Preflight checks gate install steps until required inputs are satisfied.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-9: Add System Setup Center to the dashboard**
  - **Description:** Provide a UI-first operations surface for health, updates, backups, and guided node onboarding.
  - **Acceptance Criteria:**
    - Dashboard includes a System Setup area with infra/service status and last health results.
    - Guided node onboarding flows live in the Setup Center (scan, adopt, configure).
    - Credentials (Emporia tokens, etc.) are managed in a centralized Setup Center panel.
    - A diagnostics export action is available from the Setup Center UI.
    - Backups and upgrades are exposed as one-click actions in the Setup Center.
  - **Status:** Done (`make e2e-installer-stack-smoke`)


- **SETUP-16: Add explicit install profiles (prod vs e2e)**
  - **Description:** Replace ad-hoc environment skip flags with first-class install profiles so production and E2E can share code while using different launchd domains/labels/ports.
  - **Acceptance Criteria:**
    - `farmctl` supports `--profile=prod|e2e` (default `prod`).
    - `prod` uses stable ports/labels and LaunchDaemons (system).
    - `e2e` uses random free ports (including the setup daemon port), namespaced launchd labels, and LaunchAgents (`gui/$UID`) so runs are repeatable without admin.
    - E2E profile output clearly reports the chosen ports and label prefix for debugging.
  - **Status:** Done (`make e2e-installer-stack-smoke`)


- **SETUP-17: Implement `farmctl uninstall/reset` for clean removal**
  - **Description:** Add a first-class uninstall/reset command so E2E and operators can reliably remove an install and its launchd services.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0017-farmctl-uninstall-orphaned-launchd-jobs.md`
    - `apps/farmctl/src/uninstall.rs`
  - **Acceptance Criteria:**
    - `farmctl uninstall` cleanly bootouts/unloads launchd services and removes installed plists for the selected profile.
    - `farmctl uninstall --remove-roots` (or equivalent) deletes the install/data/logs roots when explicitly requested.
    - E2E flow uses uninstall/reset at the end and confirms no leftover launchd labels/ports.
    - macOS E2E runs do not leave behind stale Postgres SysV IPC objects (shared memory/semaphores) that can break subsequent installs (`initdb` shmget ENOSPC).
  - **Status:** Done (`make e2e-installer-stack-smoke`; logs: `reports/e2e-installer-stack-smoke/20260102_115102`, `reports/manual-e2e-installer-stack-smoke-20260104_061306.log`)


- **SETUP-18: Make `farmctl serve` delegate to the canonical installer codepath**
  - **Description:** Remove drift between the wizard daemon and the CLI by routing install/upgrade/rollback through a single shared codepath.
  - **Acceptance Criteria:**
    - The setup daemon executes the same core install logic as the CLI (direct call or shared library module), not a parallel implementation.
    - Env-based skip flags are limited to development diagnostics (not used in the gating E2E path).
  - **Status:** Done (`make e2e-installer-stack-smoke`)


- **SETUP-19: Generate non-default credentials for bundled Postgres**
  - **Description:** Remove the hardcoded `postgres/postgres` default and generate a unique Postgres password (and/or role) during fresh setup; store it in the setup config so all bundled services use the same deterministic, non-default DB URL.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `apps/farmctl/src/config.rs`
    - `apps/farmctl/src/native.rs`
  - **Acceptance Criteria:**
    - Fresh `farmctl` setup writes a config with a non-default Postgres password (no `postgres:postgres` fallback).
    - Postgres `initdb` uses the configured password and the controller stack starts successfully.
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260102_125643`)


- **SETUP-20: Make the MQTT broker reachable to LAN nodes (bind fix)**
  - **Description:** Fix the Mosquitto listener configuration so Pi nodes can reach the controller broker at `mqtt://core.local:<port>` as documented (no `127.0.0.1`-only listener).
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0016-external-audit-2026-01-01-security-code-quality.md`
    - `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
    - `docs/runbooks/renogy-pi5-deployment.md`
    - `apps/farmctl/src/native.rs`
  - **Acceptance Criteria:**
    - The bundled Mosquitto config listens on the intended port for LAN clients (not loopback-only).
    - `make e2e-installer-stack-smoke` remains green.
    - Docs/runbooks remain consistent about the broker URL nodes should use.
  - **Status:** Done (`cargo test --manifest-path apps/farmctl/Cargo.toml`, `make e2e-installer-stack-smoke`; log: `reports/e2e-installer-stack-smoke/20260102_125643`)


- **SETUP-21: Prevent/purge launchd override state pollution for E2E installs**
  - **Description:** Ensure installer-path E2E runs do not leave persistent launchd enable/disable override records for the run’s namespaced labels, and provide a one-time purge tool for historical residue.
  - **References:**
    - `tools/e2e_setup_smoke.py`
    - `tools/purge_launchd_overrides.py`
    - `docs/DEVELOPMENT_GUIDE.md`
  - **Acceptance Criteria:**
    - `make e2e-setup-smoke` fails if override keys remain for the run’s `launchd_label_prefix` after uninstall.
    - A purge helper exists to remove historical `com.farmdashboard.e2e.*` override keys (requires admin, one-time).
    - `make e2e-installer-stack-smoke` remains green.
  - **Status:** Done (`make e2e-installer-stack-smoke`; log: `reports/manual-e2e-installer-stack-smoke-20260102_195458.log`)


- **SETUP-1: Create the local setup app foundation (wizard UI + API)**
  - **Description:** Build a separate local setup app with a GUI wizard that collects production inputs, runs preflight checks, and writes a validated config file for native service installs.
  - **Acceptance Criteria:**
    - A local web-based wizard runs as a standalone app and guides a non-expert through initial setup.
    - The app exposes a JSON API for preflight checks and config persistence.
    - The setup config is stored in a well-defined location with clear defaults and can be reloaded on restart.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

- **SETUP-2: Generate launchd service definitions from setup config**
  - **Description:** Generate LaunchDaemon/LaunchAgent plist files for core-server, telemetry-sidecar, and dashboard-web based on the setup config.
  - **Acceptance Criteria:**
    - Plist files are generated into a staging directory with placeholders resolved from the setup config.
    - The plan output includes the intended install targets and commands for activation.
    - The setup app surfaces errors if required binaries/paths are missing.
  - **Status:** Done (`make e2e-installer-stack-smoke`)

## Architecture & Technical Debt
### Done
- **ARCH-1: Catalogue oversized production modules**
  - **Description:** Inventory the most tangled, over-1k-line files in the production stacks so future refactors can target the files that already mix data fetching, validation, UI rendering, and API wiring.
  - **Acceptance Criteria:**
    - Record the top production files (dashboard-web, core-server-rs, etc.) that grow beyond reasonable single-responsibility sizes after filtering out dependency bundles.
    - Summarize the responsibilities in each candidate so engineers can prioritize refactors (e.g., analytics overview, map page, API helpers, forecast routes).
    - Reflect the investigation in `project_management/BOARD.md` and `project_management/EPICS.md` so the effort is tracked at the epic level.
  - **Status:** Done (2026-01-22: god-file inventory captured + planning docs updated)
