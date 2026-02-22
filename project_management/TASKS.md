# Infrastructure Dashboard Tasks

This document provides a detailed list of all the tasks for the Infrastructure Dashboard project.

---

## Ticket Workflow

- `project_management/TASKS.md` is the single source of truth for work items (task IDs like `CS-1`, `AN-11`, etc.).
- `project_management/BOARD.md` is a high-level rollup; `project_management/EPICS.md` defines epic scope/goals.
- Completed tickets live in `project_management/TASKS_DONE_2026.md`.
- Indefinitely deferred/deprecated tickets live in `project_management/TASKS_DEFERRED_INDEFINITE.md`.
- Long-form requirement briefs for active work live in `project_management/tickets/` (create a stub with `make ticket t="..."`); archived/completed briefs live in `project_management/archive/tickets/`.
- Architecture decisions live in `docs/ADRs/` (create a stub with `make adr t="..."`).
- Keep repo instructions free of external agent/model delegation workflows (obsolete as of 2026-01-14).
- Each `XX-N` entry is a work item. Keep IDs stable; add new IDs sequentially per prefix (do not renumber).
- Status should start with one of: `To Do`, `In Progress`, `Blocked: <reason>`, or `Done` (you can append qualifiers in parentheses).
- If a feature requires later hardware validation, **split it into two tasks**: **Implement …** and **Validate … on hardware**. Mark the validation task `Blocked: hardware validation (...)` until hardware is available; do not leave hardware-waiting work as `In Progress`.
- Any `To Do`/`In Progress` ticket should include clear acceptance criteria.
- Dashboard-web UI changes (`apps/dashboard-web/src/**`): acceptance criteria must state compliance with `apps/dashboard-web/AGENTS.md` UI/UX guardrails (page pattern + token set, templates/slots, visual hierarchy, component variants, UI debt tracking). If a deliberate one-off violates guardrails, add a follow-up `DW-*` debt ticket with owner + measurable exit criteria before merge.
- When starting work, move the ticket to `In Progress`, create a branch/PR that references the ticket ID, and update the ticket to `Done` when merged.
- **Two-tier validation policy (production):**
  - **Tier A = “Validated on installed controller”** (production smoke; **no DB/settings reset**). Evidence must include the deployed controller bundle version + health checks; if the ticket touches the dashboard UI, Tier A must also include **at least one screenshot that was captured and viewed** (store under `manual_screenshots_web/` and reference the file path in the ticket).
  - **Tier B = “Validated on clean host via E2E”** (clean-state pre/postflight enforced).
  - **Implementation tickets may be marked Done after Tier A** only if they reference a Tier B validation **cluster ticket** when clean-host E2E is deferred.
  - **Standard status strings:**
    - Implementation ticket: `Done (validated on installed controller; clean-host E2E deferred to <CLUSTER-ID>)`
    - Validation cluster ticket: `Blocked: clean-host E2E validation (prod host cannot be stopped/reset)`
- When a `project_management/tickets/TICKET-####-*.md` file is added, create a corresponding work item here and link it.
- 2026-01-24: Tier-A runbook audit summary recorded as DOC-37 in `project_management/TASKS_DONE_2026.md`.

### New TASKS Entry Template

- **XX-N: <title>**
  - **Description:** <what/why>
  - **Acceptance Criteria:**
    - <verifiable outcome>
  - **Status:** To Do

---

## Operations

### To Do
- **CS-103: Standardize “site time” (controller-local time) across API + UI**
  - **Description:** Standardize timestamp rendering across core-server and all clients (dashboard-web, future iOS) so operators always see a consistent “site time” (controller-local timezone), not the viewer’s browser timezone. This avoids confusion when operators access the controller remotely.
  - **Acceptance Criteria:**
    - Core-server exposes the controller/site timezone explicitly (e.g., via a dedicated endpoint or a `site_timezone` field in an existing config/status payload).
    - Dashboard-web uses controller/site timezone for chart x-axes, timestamps, and time labels (not browser-local time) wherever timestamps are shown.
    - All charting/date formatting uses a single shared helper so behavior is consistent across tabs.
    - Existing tests remain green.
  - **Notes / Run Log:**
    - 2026-01-20: In progress: start by standardizing Analytics Overview + Trends + Power on controller-local time (and expose timezone via an API payload), then expand to the remaining tabs as follow-up work.
  - **Status:** In Progress

## Core Server

### Blocked
- **CS-69: Validate Power/Analytics composition cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm power/analytics composition is correct end-to-end (Renogy + Emporia ingest, series bucketing, totals composition, and dashboard rendering) without production-host constraints.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - `make e2e-web-smoke` passes and the power/analytics UI renders with real data (no fabricated zeros, correct units, correct bucketing).
    - Evidence is recorded (commands + artifact path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)

- **CS-81: Validate Pi 5 deploy-over-SSH SPI bootstrap on real hardware**
  - **Description:** Validate that the dashboard “Deploy over SSH” job enables SPI0 automatically on a clean Raspberry Pi OS Lite image so optional ADS1263 ADC HAT sensors can be added without manual SSH edits.
  - **Acceptance Criteria:**
    - On a clean Pi 5 OS image (SPI disabled by default), running Deployment → Remote Pi 5 Deployment completes successfully.
    - The deploy job enables `dtparam=spi=on` and reboots once if required, then reconnects and continues automatically.
    - After deployment, `/dev/spidev0.0` exists and enabling ADS1263 in the dashboard reports `analog_health.ok=true`.
  - **Status:** Blocked: hardware validation (Pi 5 node with clean OS image)

- **CS-105: Validate DHCP churn does not break node config/sensors (Tier A)**
  - **Description:** Validate that when a Pi node receives a new IPv4 address via DHCP, the controller can still locate the node-agent and push sensor/config changes without manual IP edits.
  - **Acceptance Criteria:**
    - After forcing a DHCP renewal (node gets a new IP), the node remains the same logical node in the dashboard (identity is stable; existing sensors/history remain attached).
    - Core-server updates `nodes.ip_last` to the new IP within ≤2 heartbeats and preserves a stable node-agent hostname (`<agent_node_id>.local`) in node config.
    - Applying a sensor config update from the dashboard succeeds after the IP change (node-agent receives the update and continues publishing telemetry).
    - Record Tier‑A evidence (controller bundle version + health checks) in a run log under `project_management/runs/`.
  - **Notes / Run Log:**
    - 2026-02-08: Deployed updated node-agent overlay to existing Pi nodes (no adoption token issued; node names preserved).
      - Pi5 Node 1 (`10.255.8.170`) job: `267aeb95f5a9a699` (status: success)
      - Pi5 Node 2 (`10.255.8.20`) job: `d826481984318dd9` (status: success)
  - **Status:** Blocked: hardware validation (Pi node on LAN + DHCP lease change)

- **CS-108: Validate battery SOC estimator + power runway on real Renogy system (Tier A)**
  - **Description:** Validate the controller-side battery SOC estimator + capacity/runway projection against a real Renogy Rover + BT‑2 system where the true load is measured via an ADS1263/ADC-hat power sensor (not Renogy `load_power_w`).
  - **Acceptance Criteria:**
    - On an installed controller refresh (Tier A; no DB/settings reset), configure:
      - Battery model (sticker capacity Ah + SOC cutoff + anchoring)
      - Power runway (load_sensor_ids = ADC-hat load power sensor; PV derate + projection days)
      - PV forecast enabled for the Renogy node (Forecast.Solar public).
    - Power tab shows:
      - Estimated SOC (%), Renogy SOC (%), sticker capacity (Ah), remaining (Ah), runway (hours/days).
    - Runway behaves conservatively beyond the PV forecast horizon (PV=0); alert thresholds can be built in Alarm Wizard on the runway sensor.
    - Create a warning alarm rule in Alarm Wizard (ex: runway < 72h) and confirm it evaluates without errors.
    - Record Tier‑A evidence in a run log under `project_management/runs/`:
      - controller bundle version
      - `/healthz` and `farmctl health`
      - at least one captured **and viewed** screenshot under `manual_screenshots_web/` showing the runway UI.
  - **Notes / Run Log:**
    - 2026-02-18: Tier A validated installed controller `0.1.9.274-battery-runway-fix` (run: `project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md`; screenshot gate: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md` PASS).
  - **Status:** Done


### In Progress
- **CS-77: WS-2902: expose status + rotate-token endpoints by node id**
  - **Description:** The dashboard can identify a weather station node by `node_id`, but the token rotation API is keyed by `integration_id` (not discoverable later). Add core-server endpoints to fetch WS-2902 integration status and rotate the token using the weather station `node_id` so the dashboard can offer a “Rotate token” entrypoint from the node detail UI.
  - **Acceptance Criteria:**
    - `GET /api/weather-stations/ws-2902/node/{node_id}` returns the same payload shape as `GET /api/weather-stations/ws-2902/{integration_id}` and requires `config.write`.
    - `POST /api/weather-stations/ws-2902/node/{node_id}/rotate-token` returns the same payload shape as `POST /api/weather-stations/ws-2902/{integration_id}/rotate-token` and requires `config.write`.
    - Unknown/invalid node IDs return `404 Integration not found` (no internal error leakage).
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` passes.
  - **Notes / Run Log:**
    - 2026-01-14: Tests intentionally not run (per operator request: “No tests”). Recommended follow-up validation: `make ci-core-smoke`.
  - **Status:** In Progress

- **CS-80: Pi 5 deploy-over-SSH enables SPI0 automatically (ADS1263)**
  - **Description:** Harden the Pi 5 “Deploy over SSH” workflow so optional ADS1263 ADC HAT deployments do not require manual OS configuration. If SPI0 is disabled, the deploy job must enable it and handle the reboot/reconnect automatically.
  - **Acceptance Criteria:**
    - When `/dev/spidev0.0` is missing, the deploy job enables SPI in the boot config (`/boot/firmware/config.txt` or `/boot/config.txt`), reboots, waits for SSH to return, reconnects, and verifies SPI0 is present.
    - When SPI0 is already enabled, the deploy job does not reboot and proceeds normally.
    - `make ci-core` remains green.
  - **Notes / Run Log:**
    - 2026-01-18: Added SPI0 bootstrap to the deploy job (`apps/core-server-rs/src/services/deployments/bootstrap.rs`) so clean Raspberry Pi OS Lite images do not require manual `dtparam=spi=on` edits for ADS1263.
  - **Status:** In Progress

- **CS-107: Battery model (estimated SOC) + power runway projection (Renogy + ADC-hat load)**
  - **Description:** Add controller-side battery intelligence: persistent SOC estimation (coulomb counting + resting anchoring) with capacity estimation and a conservative power runway projection that uses Forecast.Solar PV forecasts plus an hour-of-day load profile built from a selectable load power sensor (ADC-hat on node1).
  - **Acceptance Criteria:**
    - DB migration adds `battery_estimator_state` table to persist per-node SOC/capacity estimator state.
    - Core-server starts background services:
      - Battery estimator (SOC + capacity estimation)
      - Runway projector (Forecast.Solar + learned load profile; PV=0 beyond forecast horizon)
    - Core-server exposes authenticated config endpoints:
      - `GET/PUT /api/battery/config/{node_id}` (stores `battery_model` in `nodes.config`)
      - `GET/PUT /api/power/runway/config/{node_id}` (stores `power_runway` in `nodes.config`)
    - For configured nodes, core-server creates read-only virtual sensors and writes `metrics`:
      - `battery_soc_est_percent` (%)
      - `battery_remaining_ah` (Ah; uses sticker capacity)
      - `battery_capacity_est_ah` (Ah)
      - `power_runway_hours_conservative` (hr)
    - Runway uses the selected `load_sensor_ids` (W) rather than Renogy `load_power_w` and supports summing multiple load sensors.
    - Local validation passes:
      - `make ci-core-smoke`
  - **Owner:** Platform (Codex)
  - **Notes / Run Log:**
    - 2026-02-18: Implemented battery SOC estimator + runway projection services, migration `044_battery_estimator_state.sql`, config endpoints, and virtual sensors. Local validation: `make ci-core-smoke` (PASS). Hardware validation tracked as CS-108.
  - **Status:** Done

- **CS-106: Related sensors semantics for circular + cumulative-reset sensors (wind direction + daily rain)**
  - **Description:** Fix related-sensors analysis so circular wind direction (0–360 wrap) and daily cumulative rain totals (midnight reset) do not generate wrap/reset artifacts that suppress or distort related-sensor ranking for level-like signals such as reservoir depth.
  - **Acceptance Criteria:**
    - `BucketAggregationPreference::Auto` uses correct bucket aggregation semantics:
      - `wind_direction` → `Last`
      - `rain_rate` → `Avg`
      - `rain` (daily cumulative) → `Last` (not `Sum`)
      - Pulse/gauge rain sensors remain `Sum`.
    - WS-2902 integrations auto-create correlation-friendly derived sensors:
      - `wind_dir_sin` + `wind_dir_cos` (from `wind_direction`)
      - `rain_inc` (increment from daily cumulative `rain`)
    - Event detection in `event_match_v1` and `cooccurrence_v1` is semantics-aware:
      - circular-safe deltas for wind direction
      - reset-safe non-negative deltas for daily cumulative rain totals
    - Simple-mode `related_sensors_unified_v2` auto-enables Δ correlation weighting for level-like focus sensors (e.g., `water_level` / `*depth*`) so gradual relationships can surface without advanced toggles.
    - Local validation passes:
      - `make ci-core`
  - **Notes / Run Log:**
    - 2026-02-18: Started (implementing semantics-aware bucket aggregation + delta handling for WS-2902 rain/wind related-sensors ranking).
    - 2026-02-18: Implemented WS-2902 derived sensors (`wind_dir_sin`, `wind_dir_cos`, `rain_inc`) + semantics-aware deltas/bucketing for related sensors.
    - 2026-02-18: Local validation: `make ci-core` (PASS; 0 warnings).
    - 2026-02-17: Tier A validated installed controller `0.1.9.272-cs106-related-sensors` (run: `project_management/runs/RUN-20260217-tier-a-cs106-wind-rain-related-sensors-0.1.9.272-cs106-related-sensors.md`; screenshot gate: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-cs106-wind-rain-related-sensors-0.1.9.272-cs106-related-sensors.md` PASS).
  - **Status:** Done

- **CS-104: DHCP-safe node-agent addressing (mDNS hostname + MAC-matched heartbeat IP refresh)**
  - **Description:** Ensure node DHCP address changes do not break node configurations or sensors. Core-server must locate node-agent endpoints without relying on stale `nodes.ip_last`, and node identity must remain MAC-based (not IP-based).
  - **Acceptance Criteria:**
    - Core-server resolves a node-agent endpoint using (in order): `config.node_agent.host`, `config.agent_node_id` (`<id>.local`), mDNS discovery (`_iotnode._tcp.local.`) by matching TXT `mac_eth`/`mac_wifi`, then `nodes.ip_last` as a legacy fallback.
    - Node-agent heartbeats on `iot/<node_id>/status` include `ip` + `mac_eth`/`mac_wifi` so the controller can self-heal `nodes.ip_last` after DHCP changes.
    - Core-server ingests `iot/+/status` and refreshes `nodes.ip_last` by matching the node record via `mac_eth`/`mac_wifi` (no IP-based identity).
    - Node-agent HTTP call sites that push config (sensors, display profile, Renogy preset/settings, restore worker) use the resolver and retry on stale endpoints.
    - Local tests pass:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `cd apps/node-agent && poetry run python -m pytest`
  - **Notes / Run Log:**
    - 2026-02-08: Implemented resolver + MQTT status ingest + node-agent heartbeat IP/MAC fields. Tests: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass), `cd apps/node-agent && poetry run python -m pytest` (pass). Hardware DHCP churn validation is tracked as CS-105.
    - 2026-02-08: Tier‑A refresh validated installed controller `0.1.9.258-cs104-dhcp-safe` (run: `project_management/runs/RUN-20260208-tier-a-cs104-dhcp-safe-0.1.9.258-cs104-dhcp-safe.md`).
  - **Status:** Done (implementation complete; hardware validation tracked as CS-105)

### To Do
- **CS-109: External device integration framework (TCP/IP, catalog-driven)**
  - **Description:** Add core-server support for external commercial devices (Modbus TCP, SNMP, HTTP JSON, BACnet/IP) with catalog-driven point mapping and polling into metrics.
  - **References:**
    - `project_management/tickets/TICKET-0077-infrastructure-dashboard-commercial-integrations.md`
  - **Acceptance Criteria:**
    - Core-server can register external devices via `/api/integrations/devices` and list them.
    - Catalog points are auto-upserted into sensors with `source=external_device`.
    - Poller ingests points for Modbus TCP, SNMP, and HTTP JSON.
    - BACnet/IP driver stub exists with clear TODOs or is fully implemented.
    - `make ci-core-smoke` passes.
  - **Status:** To Do

- **CS-110: Device profile catalog for commercial integrations (last 10 years)**
  - **Description:** Build per-model point libraries for the required device families using publicly available documentation (TCP/IP protocols only).
  - **References:**
    - `project_management/tickets/TICKET-0077-infrastructure-dashboard-commercial-integrations.md`
  - **Acceptance Criteria:**
    - `shared/device_profiles/catalog.json` includes all models (last 10 years) for listed vendors.
    - Points are mapped with units, types, and protocol metadata per model.
    - Importer covers all available points for each model using public docs.
    - `make ci-presets-smoke` passes.
  - **Status:** To Do

---

## Rust Core Server Migration

**Intent:** After the installer gate is stable, migrate the production runtime from the Python core-server + Node-based dashboard server to a single Rust core-server binary that serves:
- `/api/*` (Rust API)
- `/` (static dashboard build assets)

**Constraints:**
- Contract-first via OpenAPI, with TS client generation from the canonical schema.
- DB schema parity and side-by-side behavioral comparison during the migration (run Python and Rust backends against the same DB and compare responses).
- Keep the dashboard as JS/TS static assets (avoid Rust/WASM rewrite); Rust SSR/HTMX is acceptable for the setup wizard surface only.

### To Do
- **RCS-16: OpenTelemetry tracing**
  - **Description:** Add OpenTelemetry distributed tracing to `apps/core-server-rs` consistent with telemetry-sidecar so cross-service issues can be debugged via trace spans.
  - **References:**
    - `project_management/tickets/TICKET-0021-add-opentelemetry-distributed-tracing.md`
  - **Acceptance Criteria:**
    - OpenTelemetry can be enabled/disabled via env (e.g., `CORE_OTEL_ENABLED`).
    - Key HTTP handlers emit spans and traces export to the configured OTLP endpoint.
  - **Status:** To Do


- **RCS-17: Refactor outputs.rs**
  - **Description:** Split `apps/core-server-rs/src/routes/outputs.rs` into smaller focused modules without behavior changes.
  - **References:**
    - `project_management/tickets/TICKET-0022-refactor-outputs-rs-into-smaller-modules.md`
  - **Acceptance Criteria:**
    - Route behavior remains unchanged and tests remain green after refactor.
  - **Status:** To Do


- **RCS-20: Controller no-Python runtime guardrails**
  - **Description:** Add explicit verification + guardrails that the production controller (Mac mini) runs no Python services as part of the core stack (core-server, telemetry-sidecar, setup-daemon). This prevents regressions and resolves ongoing confusion about whether core-server is still Python.
  - **References:**
    - `project_management/tickets/TICKET-0043-controller-no-python-runtime-guardrails.md`
  - **Acceptance Criteria:**
    - Production runbooks include a short, concrete “prove no Python services” checklist (launchd + process checks) and expected output.
    - Installer-path health checks (farmctl and/or E2E harness) detect and surface an explicit warning/error if any `com.farmdashboard.*` service is running under Python/uvicorn.
    - CI/E2E includes an assertion that the installed `core-server` launchd service is the Rust binary (not `python -m uvicorn`).
  - **Status:** To Do


- **RCS-21: Core-server Python tooling rename and prune**
  - **Description:** Remove the “core-server is still Python” repo-level ambiguity by renaming/pruning the legacy Python `apps/core-server` surface so it cannot be mistaken for a production runtime.
  - **References:**
    - `project_management/archive/tickets/TICKET-0044-core-server-python-tooling-rename-and-prune.md`
  - **Acceptance Criteria:**
    - The legacy Python `apps/core-server/` directory is removed (no “tooling-only core-server” ambiguity remains).
    - No onboarding or production docs imply that the controller runs a Python FastAPI server.
    - Repo scripts/CI are updated and remain green; DB migrations/seed workflows route through Rust-first tooling (`farmctl db ...`).
  - **Notes / Run Log:**
    - 2026-02-12: Completed as part of ARCH-6 pruning (Tier‑A evidence: `project_management/runs/RUN-20260212-tier-a-arch6-prune-0.1.9.268-arch6-prune.md`).
  - **Status:** Done


---

## Telemetry Ingest Sidecar

### In Progress
- **TS-9: Node health telemetry split (ICMP ping vs MQTT RTT) + core controller parity**
  - **Description:** Split node health latency into explicit ICMP ping metrics (`ping_ms`, `ping_jitter_ms`, `ping_p50_30m_ms`) and MQTT broker RTT diagnostics, persist both in `nodes`, and ensure node-health (“system”) sensors are created **and visible (not hidden)** for Pi5 nodes and the core controller node. Surface the required health stats on Nodes cards and Node detail, and make the sensors selectable on Trends.
  - **Acceptance Criteria:**
    - `telemetry-sidecar` parses/persists `ping_ms`, `ping_jitter_ms`, `ping_p50_30m_ms`, `mqtt_broker_rtt_ms`, and `mqtt_broker_rtt_jitter_ms` from node status payloads, while preserving backward compatibility with legacy `network_*` fields.
    - Node-agent heartbeat publishes ICMP ping metrics (including 30m p50) and keeps MQTT RTT as a separate diagnostic metric.
    - Core controller node emits the same health metrics (ping/jitter/p50/uptime24h/storage/ram) through sidecar-generated status heartbeats.
    - Dashboard Nodes cards and Node detail show ping, jitter, ping 30m, uptime (24h), storage used, and RAM used.
    - Node-health (“system”) sensors are **not hidden** and appear in `/api/sensors` (no `include_hidden=1` required), so they can be selected on **Sensors & Outputs** and **Trends**.
    - Tier‑A installed-controller validation evidence:
      - `make e2e-installed-health-smoke` passes after refreshing the installed controller.
      - At least one screenshot was captured **and viewed** showing ping (and/or jitter/p50) sensors selectable on Trends; store under `manual_screenshots_web/` and reference it in the run log.
    - Local validation passes:
      - `cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml`
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `PYTHONPATH=. poetry run pytest tests/test_latency_probe.py tests/test_validation_smoke.py` (from `apps/node-agent`)
      - `make ci-web-smoke-build`
  - **Status:** In Progress (code + local validation complete; Tier A installed-controller validation pending)

---

## Offline Telemetry Spool + Backfill Replay

> **Intent:** Make node telemetry durable across hard disconnects (48h+) with microSD-friendly sequential writes and a safe replay path that cannot cause controller liveness flaps.
>
> **Committed architecture:** Option C (append-only segment spool + Rust node-forwarder + controller ACK + receipt-time liveness). See ADR 0009 and `TICKET-0049`.

> **Tier A + hardware evidence:** `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`

### To Do
- [ ] No open items

### To Do
- **DW-259: Setup Center external device management UI**
  - **Description:** Add Setup Center UI to manage external commercial devices (catalog selection, connection settings, sync, and removal).
  - **References:**
    - `project_management/tickets/TICKET-0077-infrastructure-dashboard-commercial-integrations.md`
  - **Acceptance Criteria:**
    - Integrations → External devices card allows selecting vendor/model/protocol from the catalog.
    - Operators can add/sync/remove external devices via `/api/integrations/devices`.
    - UI follows `apps/dashboard-web/AGENTS.md` guardrails; no bespoke layout drift.
    - `make ci-web-smoke` passes.
  - **Status:** To Do

### Deferred / Optional
- **OT-14: Phase 2: HTTP bulk ingest for backfill (batch + compression + acked_seq response)**
  - **Description:** Add a bulk ingest path for backfill so very large replay loads do not require per-sample MQTT publishes.
  - **Acceptance Criteria:**
    - Node-forwarder can upload bounded batches (compressed) and receive an `acked_seq` response.
    - Bulk ingest remains idempotent and respects caps/rate limits.
  - **Status:** To Do (deferred/optional)


- **OT-15: Phase 2: time anchors + drift correction for long offline windows**
  - **Description:** Improve timestamp correctness for long outages by storing monotonic anchors (mono_ns ↔ wall_ts) and applying bounded drift correction on reconnect.
  - **Acceptance Criteria:**
    - Payload includes `time_quality` and sufficient anchor metadata to identify suspect windows.
    - Controller can label/annotate suspect time windows rather than silently mixing low-quality timestamps.
  - **Status:** To Do (deferred/optional)

---

## Standalone Rust Telemetry + Predictive

> **Note:** This epic is **optional/deferred** and does **not** gate production readiness right now. Production uses the Rust telemetry-sidecar ingest pipeline and the Python predictive worker is **disabled by default** unless explicitly enabled via Setup Center.

### Deferred / Optional
- **RS-1: Define standalone Rust ingest + predictive rollout plan**
  - **Description:** Draft an ADR and rollout plan covering scope, API boundaries, and migration steps from sidecar + Python.
  - **Acceptance Criteria:**
    - ADR documents Rust ingest + predictive architecture, LLM client, sandbox/tooling, and retry strategy.
    - Rollback plan explicitly supports reverting to sidecar + Python predictive worker via feature flags.
    - Test plan includes integration, load, and failure-injection coverage.
  - **Status:** To Do (deferred/optional; not required for production while predictive is disabled by default)


- **RS-2: Implement standalone Rust ingest + predictive pipeline**
  - **Description:** Deliver a single Rust service that handles MQTT ingest, DB writes, and predictive scoring end to end.
  - **Acceptance Criteria:**
    - Rust pipeline ingests MQTT telemetry, applies COV/rolling average logic, and updates status/alarms.
    - Predictive inference uses the Rust LLM client with sandbox/tooling and retries.
    - Feature flag allows switching between sidecar-only and full Rust pipeline.
  - **Status:** To Do (deferred/optional; not required for production while predictive is disabled by default)


- **RS-3: Add dedicated tests and rollback validation for Rust pipeline**
  - **Description:** Build the test harness and rollback drills needed to ship the full Rust rewrite safely.
  - **Acceptance Criteria:**
    - End-to-end tests cover MQTT to DB to predictive alarms.
    - Load and failure tests validate backpressure, retries, and sandbox timeouts.
    - Rollback procedure is verified without data loss or duplicate ingest.
  - **Status:** To Do (deferred/optional; not required for production while predictive is disabled by default)


---

## Node Agent

### Blocked
- **NA-48: Validate generic Pi 5 node stack performance on hardware**
  - **Description:** Validate the “single generic Pi 5 node stack” performance targets on real hardware, including non-blocking control plane behavior under concurrent ADC/1-wire/MQTT/BLE load and bounded buffering behavior during uplink loss.
  - **References:**
    - `project_management/tickets/TICKET-0015-pi5-generic-node-stack-(single-image-feature-toggles).md`
  - **Acceptance Criteria:**
    - With ADC sampling at 10 channels × 2 Hz, node-agent API responses remain responsive (no event-loop stalls / watchdog resets).
    - With 1-wire sensors configured at 12-bit (750 ms worst-case per read), ADC sampling cadence is not impacted (bus-owner isolation works).
    - MQTT publish/subscribe continues without drops while BLE ingest/provisioning is active.
    - Buffers are bounded and retention policies prevent unbounded growth during uplink outages.
  - **Status:** Blocked: hardware validation (Pi 5 + representative sensors + load test harness)


- **NA-38: Validate mesh networking on real hardware**
  - **Description:** Run mesh networking on physical hardware with a zigpy coordinator and end devices to validate commissioning, link metrics, and backfill stability under field conditions.
  - **Acceptance Criteria:**
    - Commissioning succeeds with at least one physical coordinator and multiple end devices.
    - Nodes can join/leave and send/receive data through the mesh network in real conditions.
    - Mesh health metrics remain stable over a 24-hour soak.
    - Backfill/diagnostics remain stable during coordinator restarts.
  - **Status:** Blocked: hardware validation (zigpy coordinator + end devices + 24h soak)


- **NA-39: Validate BLE provisioning on real hardware**
  - **Description:** Validate the BLE provisioning flow end-to-end using a real Raspberry Pi node and the iOS app.
  - **Acceptance Criteria:**
    - iOS app discovers and connects to the node BLE service.
    - Wi-Fi credentials are applied and the node appears for adoption.
    - Local UI reflects provisioning status during the session.
  - **Status:** Blocked: hardware validation (Pi/iOS)


- **NA-40: Validate Renogy BT-2 telemetry on real hardware**
  - **Description:** Validate Renogy BT-2 BLE telemetry on a real Pi 5 + Rover controller and compare values against the Renogy mobile app.
  - **Acceptance Criteria:**
    - Pi 5 maintains a stable BT-2 connection and publishes telemetry on schedule.
    - Dashboard values match the Renogy mobile app within expected tolerance.
    - BLE disconnects recover without crashing node-agent.
  - **Run Log:**
    - 2026-01-04: Started validation on production hardware (real Pi 5 on LAN + BT-2 in proximity). Log: `reports/prod-pi5-renogy-deploy-20260104_051738.log`.
    - Findings:
      - Pi image was Debian 13 (trixie) with Python 3.13 (not Raspberry Pi OS / Python 3.11); original offline overlay shipped only cp311 wheels. Resolution: shipped multi-Python offline deps (py311 + py313) and relaxed deploy-from-server inspection to support both (validated on node2 @ `10.255.8.20`).
      - BT-2 shows as `BT-TH-BFAA8307` at `10:CA:BF:AA:83:07`; GATT notify/write UUIDs vary. Fix: node-agent now probes candidate notify/write characteristics and selects a working pair automatically (manual UUID overrides should no longer be required on this hardware).
      - Verified live values via `GET http://127.0.0.1:9000/v1/display/state` and confirmed MQTT connected (`comms.status=connected`).
    - 2026-01-12: Audited Rover Modbus protocol against official docs and corrected node-agent register decoding to match Table 1 (0x0102 charging current; 0x0103 packed temps; treat currents as unsigned). Evidence: `apps/node-agent/.venv/bin/python -m pytest -q apps/node-agent/tests/test_renogy_bt2.py`.
    - 2026-01-14/15: Node 1 telemetry stalled after a node-agent restart (collector repeatedly reported “device not found” even though BlueZ still had the device cached). Fix shipped: node-agent now falls back to the cached BlueZ device path when BLE scanning cannot rediscover the device (prevents post-restart telemetry dropouts).
    - Next:
      - Compare dashboard values against the Renogy mobile app and run a ≥1h soak (disconnect/reconnect + publish stability).
  - **Status:** Blocked: hardware validation (Renogy BT-2 + Renogy app comparison + ≥1h soak)


- **NA-41: Validate Renogy Pi 5 deployment workflow on real hardware**
  - **Description:** Validate the Renogy Pi 5 deployment bundle workflow (adopt + restore) on physical hardware.
  - **Acceptance Criteria:**
    - Provisioned Pi boots, enables `renogy-bt.service` (shipped in the generic node stack), publishes telemetry, and is adopted without SSH edits.
    - Replacement hardware can restore from the stored bundle and resume telemetry.
  - **Run Log:**
    - 2026-01-04: Started validation on production hardware (real Pi 5 on LAN). Log: `reports/prod-pi5-renogy-deploy-20260104_051738.log`.
    - Findings:
      - Pi image mismatch (Debian 13 / Python 3.13) made the offline cp311 overlay incompatible. Resolution: shipped multi-Python offline deps (py311 + py313) and relaxed deploy-from-server inspection to support both (validated on node2 @ `10.255.8.20`).
      - Renogy UUID overrides were required. Follow-up: the Renogy collector now probes candidate notify/write characteristics and selects a working pair automatically.
      - Dashboard adoption failed with `403 Invalid adoption token` because the dashboard preferred a node-advertised token while core-server accepts only controller-issued tokens. Follow-up: dashboard + core-server use controller-issued, MAC-bound tokens only (no advertised-token fallback).
      - Telemetry-sidecar dropped metrics for unknown sensors (no `sensors` rows). Follow-up: adoption syncs `http://<node>:9000/v1/config` and registers allowlisted Renogy sensors on adopt so ingest starts without manual DB seeding.
      - Dashboard “Latest” values rendered as `-`. Follow-up: core-server now joins `metrics` at read time and returns `latest_value`/`latest_ts` (trigger hotfix removed; index added via `infra/migrations/019_remove_sensor_latest_values_trigger.sql`).
      - 2026-01-05: Shipped follow-up production fixes; verified on a real Pi 5 Renogy node (report: `reports/prod-renogy-node1-fix-20260105_1327.md`). Re-validate the full deploy/adopt/restore workflow on Raspberry Pi OS.
    - Next:
      - Re-run the dashboard deploy/adopt + restore workflow without SSH edits and capture evidence (1× fresh deploy, 1× rerun idempotency, 1× restore to replacement hardware).
  - **Status:** Blocked: hardware validation (Pi 5 deploy + adopt + restore)


- **NA-46: Validate reservoir depth pressure transducer on hardware (Waveshare ADS1263 + 4–20 mA)**
  - **Description:** Validate the reservoir depth pressure transducer integration on a real Pi 5 node using the Waveshare High-Precision AD HAT (ADS1263) and a 4–20 mA current-loop transducer.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - `project_management/tickets/TICKET-0005-reservoir-depth-pressure-transducer-integration.md`
    - `project_management/tickets/TICKET-0032-pi5-ads1263-analog-contract-and-fail-closed.md`
    - `docs/runbooks/reservoir-depth-pressure-transducer.md`
  - **Acceptance Criteria:**
    - Node reports `analog_backend=ads1263` and `analog_health.ok=true` in status; dashboard surfaces this without SSH.
    - Node configured with `ads1263.enabled=true` publishes a `water_level` sensor at ~2 Hz with correct depth mapping for the 0–5 m transducer (verify against known depth or a loop calibrator).
    - Low/high current faults surface as telemetry `quality` markers and remain visible upstream (no silent drops).
    - Sampling does not block the local node-agent UI/API while telemetry is running (config requests are accepted promptly under load).
    - Soak test: sample/publish continuously for ≥1 hour without crashes/leaks, and verify the controller stores trend history.
  - **Run Log:**
    - 2026-01-13: Started validation on Node 1 (`10.255.8.170`); initial inspection found header SPI disabled (`/dev/spidev0.*` missing) and `ads1263.enabled=false` in `node_config.json`.
    - 2026-01-13: Plan revised: remove legacy “ADS1115” analog stubs, move ADS1263 GPIO off `RPi.GPIO` to `gpiozero`/`lgpio`, and fail-closed in production so misconfigured ADC hardware yields “no data” + explicit backend health (see `TICKET-0032` + ADR 0005).
  - **Status:** In Progress


- **NA-62: Pi 5 ADS1263 analog contract + fail-closed backend (remove “ADS1115” stubs)**
  - **Description:** Refactor the node-agent analog pipeline so production Pi nodes use a generic `analog` driver backed by ADS1263 (gpiozero + spidev), publish backend/health in status, and never emit synthetic analog values unless explicitly running in simulation mode.
  - **References:**
    - `ADC_ADS1263_EXECUTION_PLAN.md`
    - `project_management/tickets/TICKET-0032-pi5-ads1263-analog-contract-and-fail-closed.md`
    - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
    - `docs/development/analog-sensors-contract.md`
  - **Acceptance Criteria:**
    - Core server + dashboard never present “ADS1115” as a selectable/visible option for Pi hardware sensors; legacy values are treated as aliases but not exposed.
    - Node-agent uses `gpiozero` (+ `lgpio` where available) for CS/DRDY/RST and `spidev` for SPI; no `RPi.GPIO` dependency is required.
    - In production, if ADS1263 is disabled/unhealthy, analog sensors publish no telemetry (fail-closed) and the dashboard shows backend health.
    - Tier A: Node 1 reservoir depth sensor reads plausible values with correct unit conversion and updates continuously.
  - **Run Log:**
    - 2026-01-17: Fixed ADS1263 GPIO cleanup so failed init does not leak `gpiozero` pin reservations (prevents “pin GPIO22 is already in use” on config re-apply). Evidence: `make ci-node` (unit test: `tests/test_ads1263_hat.py::test_ads1263_hat_failed_init_releases_gpio_pins`).
    - 2026-01-17: Added ADS1263 SPI device auto-detect fallback for Pi 5 stacks that expose SPI0 as `/dev/spidev10.0` (prevents misleading “SPI disabled” failures when `/dev/spidev0.0` is absent). Evidence: `make ci-node` (unit test: `tests/test_ads1263_hat.py::test_ads1263_hat_autodetects_spi_bus_on_linux`).
    - 2026-01-17: Updated Pi 5 imaging tooling to enable SPI by default (uncomment/appends `dtparam=spi=on` in `config.txt`) so new nodes don’t require manual SPI enablement for ADS1263. Evidence: `python3 tools/node_offline_install_smoke.py`.
    - 2026-01-17: Added `ads1115` → `analog` alias for backwards-compatible node configs so legacy sensors still work without warnings. Evidence: `make ci-node` (unit test: `tests/test_sensor_type_aliases.py`).
  - **Status:** In Progress


- **NA-64: Fix Pi 5 deploy offline debs (remove RPi.GPIO; ship pigpiod + runtime deps)**
  - **Description:** Make Pi 5 deploy-from-server and preconfigured-media installs free of “expected errors” by staging only compatible runtime debs and ensuring the pigpio daemon unit is present when pulse counters are enabled.
  - **Why:** Clean Pi 5 installs should not produce deterministic dpkg/service errors. `python3-rpi.gpio` conflicts with Pi 5’s `python3-rpi-lgpio`, and staging the tiny `pigpio` meta-package fails because it depends on `*-dev` packages. We want “try offline deps” to succeed on every Pi 5 node image we support.
  - **Acceptance Criteria:**
    - Offline deb staging does **not** include `python3-rpi.gpio`.
    - Offline deb staging includes `pigpiod` + runtime libs (so `pigpiod.service` exists).
    - Deploy-from-server only enables `pigpiod.service` when the unit exists (no noisy errors).
    - `make ci-node` and `make ci-core` remain green.
    - Tier A: redeploy a real Pi 5 node (Node2) and confirm install logs contain no dpkg conflict/“unit does not exist” noise for pigpio.
  - **Run Log:**
    - 2026-01-18: Found deploy logs still attempting to install `python3-rpi.gpio_*.deb` and `pigpio_*.deb` on Node2 even though the current overlay no longer contains those packages; root cause was stale `.deb` files left behind in `/opt/node-agent/debs` from earlier deployments (overlay extraction does not remove deleted files). Fix: the deploy job now wipes `/opt/node-agent/debs` before extracting the overlay and purges any accidentally-installed `pigpio` / `python3-rpi.gpio` packages before running `dpkg -i /opt/node-agent/debs/*.deb`. Evidence: `make ci-core`.
  - **Status:** In Progress


- **NA-51: Validate pulse counter inputs on hardware**
  - **Description:** Validate pulse counter capture on real hardware with representative flow/rain sensors (including burst rates) and verify deltas map to correct engineering units.
  - **Acceptance Criteria:**
    - No missed pulses under realistic burst rates and wiring conditions.
    - Delta telemetry matches an external reference counter within tolerance.
    - Long-run soak (≥1h) shows no drift/leaks and the controller stores trend history correctly.
  - **Status:** Blocked: hardware validation (Pi 5 + pulse sensors + reference counter)


- **NA-54: Validate offline Pi 5 installs on real hardware (no WAN)**
  - **Description:** Validate both Pi deployment paths on a real Pi 5 with the WAN disconnected (isolated LAN): preconfigured media first-boot and deploy-from-server over SSH.
  - **Acceptance Criteria:**
    - With WAN unplugged/blocked, preconfigured media first-boot completes and node-agent becomes healthy.
    - With WAN unplugged/blocked, deploy-from-server completes and node-agent becomes healthy.
    - pigpio pulse counters and ADS1263 sampling remain stable enough for basic telemetry during the run (no crashes/restarts).
  - **Status:** Blocked: hardware validation (Pi 5 + offline LAN)


- **NA-61: Validate Renogy BT-2 settings apply flow on hardware**
  - **Description:** Validate the end-to-end “read → edit → validate → apply → read-back verify → history/rollback” settings flow on a real Renogy Rover controller via BT‑2.
  - **Acceptance Criteria:**
    - A safe test setting (non-destructive field) can be changed and read-back verified, then rolled back successfully.
    - Apply history records the correct diff + user + timestamps.
    - No polling instability after apply (no tight loops, no repeated disconnects).
  - **Status:** Blocked: hardware validation (Renogy BT-2 controller settings apply)


### To Do
---

## Core Infrastructure

### Blocked
- **DT-59: Validate core correctness cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean Mac host to verify core-server correctness end-to-end (auth, adoption, sensors, metrics query, backups, and other high-risk controller paths) without interference from an already-installed stack.
  - **Acceptance Criteria:**
    - A clean-host run is executed with the clean-state preflight/postflight checks passing.
    - The core Tier‑B suite runs and passes (or failures are triaged into follow-up tickets with clear repro steps).
    - Evidence is recorded (command(s) run + log path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)

- **OT-13: Validate offline buffering cluster on clean host (Tier B)**
  - **Description:** Run clean-host Tier‑B validation for the offline telemetry spool + backfill replay feature (OT). This validates the full stack under a verified clean state (no orphaned launchd jobs/processes pre/post), including outage simulation and replay without offline flaps.
  - **Acceptance Criteria:**
    - A clean-host run is executed with the clean-state preflight/postflight checks passing.
    - The offline buffering scenarios pass (disconnect → buffer → reconnect → replay; reboot mid-outage).
    - Evidence is recorded (command(s) run + log path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)

### In Progress
- [ ] No open items

### Done
- **DT-74: Enforce Tier-A screenshot review hard gate**
  - **Description:** Add an executable Tier‑A gate that fails validation unless the run log includes explicit screenshot review evidence (viewed paths, visual checks, findings, reviewer declaration), and wire it into the Tier‑A runbook/SOP.
  - **Acceptance Criteria:**
    - A repository command validates screenshot-review evidence in a Tier‑A run log and exits non-zero on missing/invalid evidence.
    - Tier‑A runbook explicitly requires the hard-gate command before marking Tier‑A complete.
    - Run-log templates include the exact required screenshot-review block format.
    - Project instructions state that Tier‑A UI validation must pass the screenshot hard gate.
    - Local validation includes one failing and one passing gate invocation.
  - **Notes / Run Log:**
    - 2026-02-11: Added `tools/tier_a_screenshot_gate.py` and `make tier-a-screenshot-gate RUN_LOG=...`.
    - 2026-02-11: Updated Tier‑A runbook (`docs/runbooks/controller-rebuild-refresh-tier-a.md`), run-log template (`project_management/runs/RUN-TEMPLATE-tsse-tier-a-validation.md`), and run-log README (`project_management/runs/README.md`) with the hard-gate requirements.
    - 2026-02-11: Updated `AGENTS.md` testing expectations to require the hard-gate command for Tier‑A UI runs.
    - 2026-02-11: Validation:
      - `python3 -m py_compile tools/tier_a_screenshot_gate.py` (pass)
      - `python3 tools/tier_a_screenshot_gate.py --repo-root <tmpdir> --run-log <tmp pass log>` (pass)
      - `python3 tools/tier_a_screenshot_gate.py --repo-root <tmpdir> --run-log <tmp fail log>` (expected fail, non-zero)
      - `make tier-a-screenshot-gate RUN_LOG=/tmp/tier_a_gate_make_pass.md` with temporary in-repo screenshot path (pass)
      - `make tier-a-screenshot-gate` without `RUN_LOG` (expected fail, non-zero)
  - **Status:** Done

- **DT-73: Defer iOS/watch from `main` and prune mobile-only build/test hooks**
  - **Description:** Keep `main` production-focused (macOS controller + web dashboard) by removing iOS/watch source and automation surfaces from `main` while preserving the mobile codebase on a dedicated freeze branch.
  - **Acceptance Criteria:**
    - `apps/ios-app/**`, `maestro/ios/**`, and mobile-only CI workflow/scripts are removed from `main`.
    - Mobile pre-commit/CI hooks are removed from active selectors and Make targets on `main`.
    - Active docs clearly state iOS/watch is deferred on `main` and reference the preservation branch.
    - The preservation branch `freeze/ios-watch-2026q1` exists and points to the last commit containing iOS/watch sources.
  - **Notes / Run Log:**
    - 2026-02-06: Started repo-wide mobile extraction from `main`; removed iOS/watch source directories, CI workflow, and simulator helper scripts.
    - 2026-02-06: Updated Makefile, pre-commit selector, API SDK generator, release validator, and docs to remove iOS/watch codepaths on `main`.
    - 2026-02-06: Pushed preservation branch `freeze/ios-watch-2026q1` to `origin` and verified `apps/ios-app/**`, `maestro/ios/**`, and `.github/workflows/ios-ci.yml` are absent on `main`.
    - 2026-02-06: Regenerated OpenAPI/SDK artifacts after removing `dashboard_demo_payload` from Rust OpenAPI paths to keep contract drift checks green.
    - 2026-02-06: Validation passed:
      - `cargo test --manifest-path apps/farmctl/Cargo.toml`
      - `python3 -m py_compile tools/api-sdk/generate.py tools/e2e_installer_stack_smoke.py tools/git-hooks/select-tests.py tools/release/release.py`
      - `PATH="/opt/homebrew/opt/openjdk@17/bin:$PATH" python3 tools/api-sdk/generate.py`
      - `make ci-farmctl-smoke`
      - `make ci-core-smoke`
      - `make ci-web-smoke-build`
      - `make e2e-installed-health-smoke` (run log: `project_management/runs/RUN-20260206-installed-controller-smoke-main-integrity-cleanup.md`)
  - **Status:** Done


### To Do
- **DT-58: Clean-host E2E runner + runbook (Tier B)**
  - **Description:** Create a repeatable clean-host environment and runbook for Tier‑B E2E validation (clean-state enforced), so production smoke (Tier A) and clean-host correctness (Tier B) can both be tracked consistently.
  - **Acceptance Criteria:**
    - A runbook exists under `docs/runbooks/` describing how to prepare a clean Mac host (no installed Farm services) and run Tier‑B suites.
    - The runbook includes how to run the primary suites (installer/setup E2E + web E2E), where logs/artifacts go, and how to capture evidence in `project_management/runs/`.
    - The runbook explicitly documents constraints (no DB/settings reset on the production controller; Tier‑B runs must be on a clean host).
  - **Status:** To Do


- **DT-72: Fix Sim Lab runner `sqlalchemy` import failure in Tier-A e2e-web-smoke**
  - **Description:** The sim-lab runner (`tools/sim_lab/run.py`) fails on `from sqlalchemy import create_engine` with `ModuleNotFoundError: No module named 'sqlalchemy'` when launched by `e2e_web_smoke.py` via `poetry run python`. This prevents the sim-lab control server (port 8100) from starting, which in turn causes `sim-lab-smoke.mjs` to time out waiting for `/healthz`. The smoke test uses the control server as a node discovery service (`/sim-lab/status` returns node API addresses). The failure blocks Tier-A `e2e-web-smoke` validation.
  - **Acceptance Criteria:**
    - `FARM_E2E_REQUIRE_INSTALLED=1 make e2e-web-smoke` passes on the production controller host.
    - The sim-lab runner starts successfully and the control server responds on port 8100.
    - The `sqlalchemy` dependency is available in the poetry virtualenv used by `e2e_web_smoke.py`.
  - **Status:** To Do

- **DT-68: Sim Lab and onboarding bloat reduction**
  - **Description:** Reduce new-developer overwhelm by splitting bootstrap/dependency install targets and making Sim Lab / node-agent local simulation an explicit opt-in path (while keeping CI/E2E coverage intact).
  - **References:**
    - `project_management/tickets/TICKET-0045-sim-lab-and-onboarding-bloat-reduction.md`
  - **Acceptance Criteria:**
    - Repo provides component-scoped bootstrap targets (core/web/node/sim-lab) and docs recommend a minimal Day‑1 path.
    - Sim Lab instructions are clearly labeled dev/CI only, and do not imply node-agent runs on the controller in production.
    - CI only installs Sim Lab/node-agent Python deps in jobs that actually run Sim Lab/E2E.
  - **Status:** To Do


### Deferred / Optional
- **DT-57: Pi 5 network boot “zero-touch” provisioning (spec gap)**
  - **Description:** Close the gap between the current `farmctl netboot` prototype (static HTTP artifacts only) and the requested “power on a Pi 5 with blank media and it discovers the server and provisions itself” experience.
  - **References:**
    - `project_management/tickets/TICKET-0018-pi5-network-boot-zero-touch-provisioning.md`
    - `apps/farmctl/src/netboot.rs`
  - **Acceptance Criteria:**
    - A documented and implementable workflow exists for a factory/default Pi 5 to discover the provisioning server without manual per-device EEPROM editing.
    - Any required LAN services (e.g., DHCP/RA, TFTP/HTTP) are explicitly implemented or explicitly delegated with an operator-friendly setup path.
    - The final outcome is a booted OS with node-agent installed/configured without interactive Pi login.
  - **Status:** To Do (deferred/optional; large spec gap and not required for near-term production milestones)

---

## Discovery and Adoption

### To Do
- [ ] No open items


---

## Dashboard Web

### Blocked
- **DW-97: Validate Map cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm the Map tab (basemaps, overlays, saved views, device placement, and markup tools) works end-to-end and remains stable under WebKit/Chromium.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - Map UX is validated in a production build (no dev server) with Playwright coverage and screenshots captured for the key flows.
    - Evidence is recorded (commands + artifact path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)


- **DW-98: Validate Trends/COV/CSV cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm Trends (range/interval bucketing, independent axes, CSV export, and COV semantics) is correct and does not regress.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - `make e2e-web-smoke` passes and the Trends-focused Playwright scenarios pass (independent-axes stability, drawer navigation, CSV export).
    - Evidence is recorded (commands + artifact path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)


- **DW-99: Validate Backups/Exports cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm Backups/Restore/Exports are safe and production-correct (auth headers, download behavior, formats, and permissions) without risking the production controller’s persistent state.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - Backups tab flows (download/export/import/restore where applicable) are validated in production build and covered by automation where feasible.
    - Evidence is recorded (commands + artifact path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)

- **DW-114: Validate dashboard layout/IA cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm the dashboard’s cross-tab layout/IA consistency (headers, banners, spacing, and navigation) remains stable under the production build and WebKit/Chromium.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - `make e2e-web-smoke` passes.
    - Playwright screenshot sweep for all sidebar tabs is captured and **viewed**, and evidence is recorded under `project_management/runs/`.
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)

- **DW-212: Validate Analytics Temp Compensation on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm the Analytics → Temp Compensation workflow (fit, preview charts, create derived compensated sensor) works in a production build and remains stable under WebKit/Chromium.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - `/analytics/compensation` loads and renders charts without JS errors.
    - The workflow can create a derived compensated sensor from a target sensor + temperature reference sensor (and can be deleted afterwards without errors).
    - Evidence is recorded under `project_management/runs/` and includes screenshots of the selected/preview/created states.
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)

### In Progress
- **DW-193: Playwright: add Chromium desktop project**
  - **Description:** Add a desktop Chromium project to the dashboard-web Playwright config so desktop coverage is available alongside mobile.
  - **Acceptance Criteria:**
    - `apps/dashboard-web/playwright.config.ts` includes a `chromium-desktop` project that uses the Desktop Chrome device profile.
    - `cd apps/dashboard-web && npm run test:playwright -- --project=chromium-desktop` passes.
  - **Status:** In Progress

- **DW-208: Analytics Overview: Soil moisture card defaults to open**
  - **Description:** The Analytics Overview “Soil moisture” section should default to open on page load so the fleet-level moisture chart is visible without extra clicks.
  - **Acceptance Criteria:**
    - Analytics Overview → “Soil moisture” section renders expanded by default on first load.
    - Operators can still collapse/expand it normally via the standard `CollapsibleCard` affordance.
    - No layout regressions across common breakpoints; UI follows the standard dashboard page pattern and token set.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - `make ci-web-smoke` passes.
    - Dev UI visual check on `127.0.0.1:3000` shows Soil moisture open by default (screenshot captured + viewed).
    - Tier A validation is intentionally deferred (do not rebuild/refresh yet).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Set Analytics Overview “Soil moisture” section `defaultOpen` to true.
    - 2026-02-02: Validation:
      - `make ci-web-smoke` (pass)
      - Dev screenshot (viewed): `manual_screenshots_web/20260202_dev_analytics_soil_open/analytics.png`
  - **Status:** In Progress (validated locally; Tier A pending)

- **DW-209: Analytics Overview: fix mobile overflow + migrate range selector to shadcn/ui**
  - **Description:** On mobile, the Analytics Overview Forecasts/Power sections can render charts/cards that overflow their parent containers (cards spill past borders). Fix the layout so the outer containers expand to fit (without narrowing charts), keep all top-level section containers the same width, and migrate the 24h/72h/7d range selector UI from the custom segmented control to the shadcn/ui component set.
  - **Acceptance Criteria:**
    - Analytics Overview renders without chart/card content visually spilling outside of its section containers on mobile (~390×844).
    - Charts are not “smooshed” narrower to fit mobile; instead the page provides a wider layout surface (horizontal scroll/zoom is acceptable) so charts remain readable.
    - All top-level section containers on the Analytics Overview page share the same width.
    - Mobile browsers can pinch-zoom **out** to fit the page width (viewport allows `minimum-scale < 1`; no `maximum-scale=1` lockout).
    - The Analytics 24h/72h/7d range selector uses shadcn/ui components (no `SegmentedControl` usage for this control).
    - UI changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (page pattern + Tailwind tokens; no inline styles/raw hex colors; no design drift).
    - `make ci-web-smoke` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-02: Set a minimum layout width for Analytics Overview so cards expand to fit charts on mobile (no “smooshed” graphs) while preserving uniform section widths.
    - 2026-02-02: Migrated the Analytics range selector to shadcn/ui `Button`-based toggle styling.
    - 2026-02-02: Enabled mobile zoom-out on `/analytics` via `minimum-scale=0.25` + `user-scalable=yes` while keeping `initial-scale=1` to avoid triggering desktop breakpoints (nav stays in mobile mode).
    - 2026-02-02: Hardened coarse-pointer behavior by gating desktop sidebar + Analytics multi-column grids behind `@media (min-width:1024px) and (pointer:fine)` so iOS zoom-out doesn’t force the page into “desktop layout”.
    - 2026-02-02: Disabled Highcharts chart zoom/pan on coarse-pointer devices + applied an Analytics-scoped `touch-action` override so browser pinch-zoom works even over charts.
    - 2026-02-02: Validation: `make ci-web-smoke` (pass).
    - 2026-02-02: Tier A refreshed installed controller to `0.1.9.238-analytics-zoom` (run: `project_management/runs/RUN-20260202-tier-a-dw209-analytics-zoom-0.1.9.238-analytics-zoom.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.238-analytics-zoom.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - Screenshots captured: `manual_screenshots_web/tier_a_0.1.9.238_dw209_analytics_zoom_20260202_005131` (needs viewing)
    - 2026-02-02: Tier A refreshed installed controller to `0.1.9.241-analytics-mobile-window` (run: `project_management/runs/RUN-20260202-tier-a-dw209-dw214-0.1.9.241-analytics-mobile-window.md`)
      - Bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.241-analytics-mobile-window.dmg`
      - Installed smoke: `make e2e-installed-health-smoke` (pass)
      - WebKit screenshots (**viewed**): `manual_screenshots_web/tier_a_0.1.9.241_dw209_pinchzoom_20260202_211214Z/*`
  - **Status:** In Progress (Tier A refreshed to installed `0.1.9.241-analytics-mobile-window`; pending on-device pinch-zoom confirmation)

- **DW-182: Setup Center: consolidate setup-daemon API calls into typed modules**
  - **Description:** Move setup-daemon fetch/post calls out of `SetupPageClient.tsx` into `apps/dashboard-web/src/app/(dashboard)/setup/api/` modules with typed return values and shared error handling.
  - **Acceptance Criteria:**
    - Setup-daemon endpoints (`health-report`, `config`, `preflight`, `local-ip`, `install`, `upgrade`, `rollback`, `diagnostics`) are accessed via new `setup/api/` modules (no direct `fetchJson`/`postJson` calls in the page).
    - Each module exports typed functions (request/response shapes) and centralizes setup-daemon error handling (consistent messages, status parsing).
    - `SetupPageClient.tsx` uses the new modules without behavior changes.
    - Changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no visual regressions).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-22: Consolidated setup-daemon calls into `apps/dashboard-web/src/app/(dashboard)/setup/api/`. Rewired Setup Center sections to use typed modules (no direct fetchJson/postJson in the page).
    - 2026-01-22: Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-22: Tier A refreshed installed controller to `0.1.9.197` (run: `project_management/runs/RUN-20260122-tier-a-dw182-dw183-dw185-dw186-setup-center-refactor-0.1.9.197.md`; screenshots: `manual_screenshots_web/20260121_210307/setup.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** In Progress (Tier A refreshed to installed `0.1.9.197`; pending screenshot viewing)

- **DW-183: Setup Center: extract validation helpers for config save flows**
  - **Description:** Move port/seconds/ms/count parsing + non-empty checks from `SetupPageClient.tsx` into reusable helpers under `apps/dashboard-web/src/app/(dashboard)/setup/lib/`.
  - **Acceptance Criteria:**
    - New helpers cover port parsing, min-seconds/ms/count checks, and required string validation with consistent error messages.
    - `SetupPageClient.tsx` uses the helpers for setup-daemon config save validation.
    - Helpers are reusable for other Setup Center forms without duplicating logic.
    - Changes follow `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no visual regressions).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-22: Extracted validation helpers into `apps/dashboard-web/src/app/(dashboard)/setup/lib/` and rewired controller-config save validation to use them.
    - 2026-01-22: Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-22: Tier A refreshed installed controller to `0.1.9.197` (run: `project_management/runs/RUN-20260122-tier-a-dw182-dw183-dw185-dw186-setup-center-refactor-0.1.9.197.md`; screenshots: `manual_screenshots_web/20260121_210307/setup.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** In Progress (Tier A refreshed to installed `0.1.9.197`; pending screenshot viewing)

- **DW-185: Setup Center: extract section components from SetupPageClient**
  - **Description:** Decompose the Setup Center render tree so each CollapsibleCard section lives in a focused component under `apps/dashboard-web/src/app/(dashboard)/setup/sections/`, reducing `SetupPageClient` size without UI drift.
  - **Acceptance Criteria:**
    - Each CollapsibleCard section (Installer actions, Health snapshot, Analytics feeds, Controller configuration, Hyperlocal weather forecast, Solar PV forecast, Offline maps, Integrations, AI anomaly detection) is moved into its own `sections/*.tsx` component with clear, typed props.
    - `SetupPageClient` keeps data-fetching/state management and passes only the required data + callbacks to sections (no behavioral changes).
    - Existing UI patterns/components (CollapsibleCard, PageHeaderCard, NodeButton) remain unchanged; visual hierarchy stays consistent (no layout or copy drift).
    - UI changes conform to `apps/dashboard-web/AGENTS.md` guardrails; if any deliberate one-off is required, add a `DW-*` UI debt ticket (owner + measurable exit criteria) before merge.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-22: Extracted Setup Center sections into `apps/dashboard-web/src/app/(dashboard)/setup/sections/` and rewired `SetupPageClient.tsx` into a small orchestrator (no UI drift intended).
    - 2026-01-22: Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-22: Tier A refreshed installed controller to `0.1.9.197` (run: `project_management/runs/RUN-20260122-tier-a-dw182-dw183-dw185-dw186-setup-center-refactor-0.1.9.197.md`; screenshots: `manual_screenshots_web/20260121_210307/setup.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** In Progress (Tier A refreshed to installed `0.1.9.197`; pending screenshot viewing)

- **DW-186: Setup Center: guard Integrations actions by capability**
  - **Description:** Ensure the Integrations section (Emporia login/meter prefs + integration tokens) is read-only for users without `config.write`, so view-only users cannot trigger mutation calls.
  - **Acceptance Criteria:**
    - `SetupPageClient` passes `canEdit` into `IntegrationsSection`.
    - Emporia login, meter preference save, and token save/clear controls are disabled when `canEdit` is false.
    - Input fields in Integrations are read-only/disabled for non-editors and clearly communicate the permission requirement.
    - Integrations mutation handlers short-circuit when `canEdit` is false (no API writes).
    - UI changes follow `apps/dashboard-web/AGENTS.md` guardrails (no visual drift; use existing component variants).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
  - **Notes / Run Log:**
    - 2026-01-22: Wired `canEdit` into `IntegrationsSection` and disabled all mutation controls for non-editors.
    - 2026-01-22: Validation: `make ci-web-smoke` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-22: Tier A refreshed installed controller to `0.1.9.197` (run: `project_management/runs/RUN-20260122-tier-a-dw182-dw183-dw185-dw186-setup-center-refactor-0.1.9.197.md`; screenshots: `manual_screenshots_web/20260121_210307/setup.png`; installed smoke: `make e2e-installed-health-smoke` (pass)).
  - **Status:** In Progress (Tier A refreshed to installed `0.1.9.197`; pending screenshot viewing)

- **DW-121: Weather station: add “Rotate token / setup” entrypoint on node detail**
  - **Description:** Operators need an obvious way to rotate a WS-2902 token after initial setup (to work around station UI limits or recover from lost config). Add a weather-station-specific management entrypoint on the weather station node detail UI that can display the new ingest path/token.
  - **Acceptance Criteria:**
    - Weather station nodes expose a “Rotate token / setup” action in the node detail (canonical detail surface) (requires `config.write`).
    - Clicking the action opens a modal that can refresh status and rotate the token (displaying the new ingest path + token).
    - Uses the by-node core-server endpoints (`CS-77`) so the integration remains manageable without knowing `integration_id`.
    - `make ci-web-smoke` passes.
  - **Notes / Run Log:**
    - 2026-01-14: Tests intentionally not run (per operator request: “No tests”). Recommended follow-up validation: `make ci-web-smoke`.
  - **Status:** In Progress

- **DW-159: Sensors & Outputs: Add sensor drawer UX/IA refactor**
  - **Description:** Refactor the “Add sensor” detail drawer (Sensors & Outputs → per-node panel) to eliminate design drift and UI debt. Provide a cohesive, task-first structure with consistent component variants, predictable hierarchy, and maintainable sectioning across both Derived and Hardware sensor flows.
  - **Acceptance Criteria:**
    - Drawer shell matches the existing right-side drawer pattern (header with title/subtitle + close, scrollable body, consistent padding/spacing).
    - Clear task-first IA:
      - “Choose sensor type” (Hardware vs Derived) is obvious and uses a single segmented control pattern (no bespoke pill buttons).
      - The active mode content is structured into consistent sections (Basics → Inputs → Expression → Advanced; or equivalent).
    - Visual hierarchy:
      - Exactly one primary action per mode (Derived: “Create”; Hardware: “Apply to node”), with supporting actions as secondary/danger as appropriate.
      - No redundant/double-intros (drawer header vs internal banners) and no confusing nested containers.
    - Component system enforcement:
      - All action buttons inside the drawer and its embedded forms use `NodeButton` variants (no one-off raw `<button>` styles).
      - Any new button variants (e.g., danger/dashed) are added centrally in `NodeButton` and reused.
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/`.
  - **Notes / Run Log:**
    - 2026-01-19: Audit focus: remove nested “card-in-card” drift, standardize sections via `CollapsibleCard`, and ensure a single primary action per mode.
    - 2026-01-19: Continue: align drawer IA to task-first sections (Basics → Inputs → Expression → Advanced), standardize spacing/typography rhythm, and remove remaining one-off component variants.
    - 2026-01-19: Follow-up UX request: audit the full drawer end-to-end (shell + Hardware mode + Derived mode) for remaining IA breakdown and inconsistent container nesting; refactor so the drawer reads like one cohesive flow (not a set of bolted-on sections).
  - **Status:** In Progress

- **DW-160: Dashboard web: Standardize collapsible section containers across tabs**
  - **Description:** Reduce UI drift and improve information density by making major section cards collapsible via a single shared component. Collapsing must be consistent across all tabs (Nodes, Sensors & Outputs, Trends, Map, Analytics, Power, Backups, Schedules, Admin tabs) without bespoke per-page `details` hacks.
  - **Owner:** Platform UI (Codex)
  - **Acceptance Criteria:**
    - A shared `CollapsibleCard` (or equivalent) is used as the canonical collapsible container for major section cards across dashboard tabs.
    - Clicking the section header toggles collapse/expand; header action controls (buttons, dropdowns) do **not** toggle collapse.
    - Defaults are sensible and task-first (e.g., primary/most-used sections default open; advanced/rare sections may default closed).
    - No regressions in visual hierarchy (no duplicate headers; consistent paddings/typography rhythm; respects `apps/dashboard-web/AGENTS.md`).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing a collapsed and expanded section.
  - **Notes / Run Log:**
    - 2026-01-19: Standardize on `src/components/CollapsibleCard.tsx` and convert remaining top-level section containers on each tab (no page-specific `details`).
    - 2026-01-19: Continue: expand to Setup Center + remaining dashboard tabs; choose sensible defaults (primary open, advanced closed) and keep header click targets consistent.
    - 2026-01-19: Follow-up UX request: make additional section containers collapsible across *all* tabs and within key workflows (including drawers/modals where appropriate), without adding bespoke page-level collapse logic.
  - **Status:** In Progress

- **DW-161: Overview: Redesign “Configure local sensors” UX (node hierarchy + drag/drop + mobile)**
  - **Description:** Fix the Overview → “Configure local sensors” modal so hiding/reordering sensors is not tedious. The UI must be node-first (clear hierarchy), allow drag-and-drop reordering, and remain usable on mobile. This prevents per-sensor “click fatigue” and makes Overview presets predictable.
  - **Owner:** Platform UI (Codex)
  - **Acceptance Criteria:**
    - Sensors are presented with a clear node hierarchy (node selector or grouped lists), not as a flat list.
    - Shown sensor priority order is editable via drag-and-drop (no up/down button-only workflow).
    - Bulk actions exist to reduce tedium (at minimum: per-node “Hide all” / “Show all”, and “Reset to default”).
    - Mobile-friendly layout exists (single-column flow; no desktop-only two-pane requirement).
    - Preferences persist across refreshes (existing localStorage prefs migrate forward without breaking).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` of the new modal on a realistic sensor set.
  - **Notes / Run Log:**
    - 2026-01-19: Plan: adopt the same node+sensor reorder UX pattern as `DisplayOrderModal` (drag/drop + optional up/down), add per-node bulk hide/show, and migrate localStorage prefs forward with a new version.
    - 2026-01-19: Continue: validate on large sensor sets; ensure mobile flow remains usable and deterministic ordering is preserved when sensors appear/disappear.
    - 2026-01-19: Follow-up UX request: eliminate remaining “click fatigue” by ensuring bulk hide/show + drag/drop work smoothly at scale and are clearly node-scoped (no flat list confusion).
  - **Status:** In Progress

- **DW-162: Analytics: Per-container time range controls (24h / 72h / 7d)**
  - **Description:** Add a single, consistent time-range control to each Analytics container that has charts so operators can switch between 24h / 72h / 7d views without leaving the page. This keeps the page responsive by default, while still allowing deeper historical inspection when needed.
  - **Acceptance Criteria:**
    - Each Analytics section card that renders graphs includes a range selector with exactly: 24 hours, 72 hours, 7 days.
    - The selection applies to **all** charts inside that container (no per-chart drift).
    - Weather forecast container uses a segmented control and shows only two plot-pairs at once (Temp/Humidity + Cloud/Precip), following the existing conventions:
      - 24h/72h: hourly line plots.
      - 7d: daily min/max temperature range bars; cloud cover remains a line plot.
    - PV forecast vs measured:
      - Always shows the entire current day (as today’s baseline), and the 72h/7d selections include 2 or 6 additional historical days.
      - Uses persisted forecast history (forecast series, not recomputed) as the overlay against measured series.
    - Time-axis labeling is readable on small plots (ticks align clearly with the plotted data and do not feel “off by one”).
    - `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
    - Tier A validation is recorded with at least one viewed screenshot under `manual_screenshots_web/` showing the 24h/72h/7d control in use.
  - **Notes / Run Log:**
    - 2026-01-19: In progress: unify range selector per container, use segmented control for Weather forecast, and implement PV forecast history overlays (day-bounded window).
    - 2026-01-19: Follow-up: capture Tier A evidence showing the 72h/7d selections in use (including Weather forecast segmented control and PV forecast historical overlay behavior) and verify small-plot x-axis tick alignment remains clear.
  - **Status:** In Progress

- **DW-166: Overview: Move “Feed health” into Overview (remove from Analytics Overview)**
  - **Description:** “Feed health” is cross-cutting operational status and belongs in the Overview tab, not inside Analytics. Move it to Overview once Analytics IA is reorganized, while preserving the same data/behavior and avoiding duplication.
  - **Acceptance Criteria:**
    - Overview tab includes a “Feed health” section (collapsible; task-first placement).
    - Analytics Overview no longer shows “Feed health” (or shows it only as a link/shortcut).
    - No behavior loss (same sources, same status semantics).
  - **Owner:** Platform UI (Codex)
  - **Status:** To Do

- **DW-181: Trends: Related sensors defaults to scanning all sensors**
  - **Description:** Remove default partial-scan behavior in Trends → Related Sensors so runs evaluate all eligible sensors in scope by default (completeness-first), not a truncated subset.
  - **Acceptance Criteria:**
    - Defaults use backend scope querying: `candidate_source=all_sensors_in_scope` in Simple mode and by default in Advanced.
    - Default runs send `evaluate_all_eligible=true`; backend honors evaluate-all for default runs (not gated to Advanced/small pools), so eligible sensors are not truncated by the old partial-scan gate.
    - Advanced still allows explicit speed tradeoffs by disabling evaluate-all and/or switching to `Visible in Trends`.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml related_sensors_unified_v2`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx tests/relatedSensorsProviderAvailability.test.tsx tests/relatedSensorsWorkflowImprovements.test.tsx tests/relatedSensorsPinnedSemantics.test.tsx`
      - `cd apps/dashboard-web && npm run build`
    - Tier A validated on installed controller (no DB/settings reset); Tier B deferred to `DW-98`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-01-22: Started.
    - 2026-02-17: Implemented completeness-first defaults across dashboard + backend: default candidate source is now `all_sensors_in_scope`, default `evaluate_all_eligible=true`, and backend evaluate-all no longer blocks on Advanced/eligible<=500. Advanced retains opt-in speed mode (disable evaluate-all and/or use `Visible in Trends`).
    - 2026-02-17: Local validation passed:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml related_sensors_unified_v2`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx tests/relatedSensorsProviderAvailability.test.tsx tests/relatedSensorsWorkflowImprovements.test.tsx tests/relatedSensorsPinnedSemantics.test.tsx`
      - `cd apps/dashboard-web && npm run build`
    - 2026-02-17: Tier A validated on installed controller `0.1.9.269` (run: `project_management/runs/RUN-20260217-tier-a-dw181-related-sensors-all-scan-0.1.9.269.md`; reviewed screenshots: `manual_screenshots_web/20260217_113553/trends.png`, `manual_screenshots_web/20260217_113553/trends_related_sensors_large_scan.png`; screenshot hard gate: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-dw181-related-sensors-all-scan-0.1.9.269.md` PASS).
    - 2026-02-17 (follow-up correction): Simple mode now defaults scope to `All nodes`, removed the “Broaden to all nodes” shortcut button, removed the Simple-mode “Refine (more candidates)” path, and runs full completeness mode for both auto-run and “Find related sensors” (no quick-suggest truncation path in Simple mode).
    - 2026-02-17: Tier A correction validated on installed controller `0.1.9.270` (run: `project_management/runs/RUN-20260217-tier-a-dw181-simple-all-nodes-correction-0.1.9.270.md`; reviewed screenshots: `manual_screenshots_web/20260217_120526/trends_related_sensors_large_scan.png`, `manual_screenshots_web/20260217_120526/trends_related_sensors_scanning.png`; screenshot hard gate: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-dw181-simple-all-nodes-correction-0.1.9.270.md` PASS).
    - 2026-02-17 (evaluate-all follow-up): Removed the evaluate-all coverage-prefilter path in unified v2 so `evaluate_all_eligible=true` runs do not silently drop candidates before scoring; expanded event/co-occurrence candidate ceilings to avoid hidden truncation in full-pool runs.
    - 2026-02-17: Tier A follow-up validated on installed controller `0.1.9.271` (run: `project_management/runs/RUN-20260217-tier-a-dw181-evaluate-all-eligible-0.1.9.271.md`; reviewed screenshot: `manual_screenshots_web/20260217_124653/trends_related_sensors_large_scan.png` shows `Evaluated: 367 of 367 eligible sensors`; screenshot hard gate: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-dw181-evaluate-all-eligible-0.1.9.271.md` PASS).
  - **Status:** Done (validated on installed controller `0.1.9.271`; clean-host E2E deferred indefinitely to DW-98 per user instruction)

- **DW-221: Trends: Related Sensors v2 unified refresh (Simple/Advanced + unified backend job)**
  - **Description:** Replace the strategy-first Related Sensors flow with a unified, non-expert-friendly experience that still supports expert controls. Add a single backend analysis job (`related_sensors_unified_v2`) that blends event-match and co-occurrence evidence, and refresh the dashboard UI with Simple/Advanced mode, insight-card results, confidence tiers, and preview explainability.
  - **Acceptance Criteria:**
    - Backend:
      - `POST /api/analysis/jobs` accepts `job_type=related_sensors_unified_v2` and returns merged related-sensor ranking from event/co-occurrence evidence.
      - Unified result payload includes blended score, confidence tier, per-strategy evidence fields, and explainability summary.
      - Job remains cancel-aware and emits phase progress updates.
    - Dashboard UX:
      - Trends → Related Sensors defaults to `Simple` mode with hybrid flow (quick suggestions + explicit “Refine results” run).
      - `Advanced` mode exposes expert controls, including events/co-occurrence weighting and detection thresholds.
      - Results render as unified ranked insight cards with confidence + evidence badges (not strategy-fragmented lists).
      - Preview panel surfaces unified “why related” details plus top co-occurrence timestamps and episodes when available.
    - Validation:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
      - `cd apps/dashboard-web && npm run lint` passes.
      - `cd apps/dashboard-web && npm run test:smoke` passes.
      - Tier A validated on installed controller; Tier B deferred to `DW-98` if not completed in the same cycle.
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-06: Implemented backend `related_sensors_unified_v2` job in Rust by composing event-match + co-occurrence jobs and merging candidates into blended ranking with confidence tiers (`high|medium|low`), per-strategy evidence, and summary copy.
    - 2026-02-06: Refreshed dashboard Related Sensors panel to unified Simple/Advanced UX, added quick auto-suggest + refine flow, replaced table rows with insight cards, and updated preview to surface unified evidence/timestamps.
    - 2026-02-06: Updated Playwright stub analysis API to support `related_sensors_unified_v2`.
    - 2026-02-06: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `cargo test --manifest-path apps/core-server-rs/Cargo.toml services::analysis::jobs::related_sensors_unified_v2::tests`, `cd apps/dashboard-web && npm run lint`, `cd apps/dashboard-web && npm run test:smoke`.
    - 2026-02-06: Tier A validated on installed controller `0.1.9.251-related-unified-v2` (run: `project_management/runs/RUN-20260206-tier-a-dw221-related-sensors-unified-0.1.9.251-related-unified-v2.md`; viewed screenshots: `manual_screenshots_web/tier_a_0.1.9.251_dw221_related_sensors_unified_20260206_0023/trends_related_sensors_large_scan.png`, `manual_screenshots_web/tier_a_0.1.9.251_dw221_related_sensors_unified_20260206_0023/trends_related_sensors_scanning.png`).
  - **Status:** Done (validated on installed controller `0.1.9.251-related-unified-v2`; clean-host E2E deferred to DW-98)

- **DW-222: Trends Related Sensors preview: avoid sparse single-point lag-aligned candidate series**
  - **Description:** In Related Sensors preview, lag alignment can shift candidate timestamps far enough that the candidate collapses to one visible point while focus still renders as a line. Add a guard so preview falls back to the raw candidate timeline when lag-aligned data is too sparse, and show clear UI copy indicating the fallback.
  - **Acceptance Criteria:**
    - For preview requests where `candidate_aligned` has `<=1` point and raw candidate has `>1` points, the chart renders the raw candidate series instead of the aligned series.
    - Preview UI shows explanatory copy when this fallback path is active.
    - Existing lag-aligned behavior remains unchanged when aligned data is sufficiently dense.
    - `cd apps/dashboard-web && npm run lint` passes.
    - `cd apps/dashboard-web && npm run test:smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validated on installed controller; Tier B deferred to `DW-98` if not completed in the same cycle.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-06: Implemented sparse-lag fallback in `PreviewPane` so aligned candidate series is suppressed when it produces `<=1` point and raw has `>1`; added user-facing note: “Lag alignment produced too few points ...”.
    - 2026-02-06: Local validation passed: `cd apps/dashboard-web && npm run lint`, `cd apps/dashboard-web && npm run test:smoke`, `cd apps/dashboard-web && npm run build`.
    - 2026-02-06: Tier A validated on installed controller `0.1.9.252-preview-fallback`; run: `project_management/runs/RUN-20260206-tier-a-dw222-preview-fallback-0.1.9.252-preview-fallback.md`; viewed screenshots: `manual_screenshots_web/20260206_015437/trends_related_sensors_large_scan.png`, `manual_screenshots_web/20260206_015437/trends_related_sensors_scanning.png`.
  - **Status:** Done (validated on installed controller `0.1.9.252-preview-fallback`; clean-host E2E deferred to DW-98)

- **DW-223: Trends Related Sensors: restore matrix-first visual scan in Simple mode**
  - **Description:** Reintroduce the colorful correlation matrix as a first-class visual in Related Sensors, prioritizing visual scanning in Simple mode while keeping unified ranked cards visible.
  - **Acceptance Criteria:**
    - Related Sensors Simple mode renders the correlation matrix before the ranked cards/preview section.
    - Matrix candidates are selected from unified related results using score cutoff (`blended_score >= cutoff`) with cap (`up to 25` related sensors + focus sensor).
    - Clicking a matrix cell updates Related Sensors preview context to a relevant candidate.
    - Existing unified ranking and preview behavior remains intact.
    - `cd apps/dashboard-web && npm run lint` passes.
    - `cd apps/dashboard-web && npm run test:smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validated on installed controller; Tier B deferred to `DW-98` if not completed in the same cycle.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-06: Implemented matrix-first rendering path in Related Sensors panel, wired matrix job execution (`correlation_matrix_v1`) from unified candidates, and added matrix-cell click-to-preview behavior.
    - 2026-02-06: Added score-cutoff based matrix inclusion utility + unit tests (`apps/dashboard-web/tests/correlationMatrixSelection.test.ts`).
    - 2026-02-06: Local validation passed: `cd apps/dashboard-web && npm run lint`, `cd apps/dashboard-web && npm run test:smoke`, `cd apps/dashboard-web && npm test -- --run tests/correlationMatrixSelection.test.ts`, `cd apps/dashboard-web && npm run build`.
    - 2026-02-06: Tier A validated on installed controller `0.1.9.254-matrix-refresh-fix`; run: `project_management/runs/RUN-20260206-tier-a-dw223-dw224-matrix-refresh-fix-0.1.9.254-matrix-refresh-fix.md`; viewed screenshot: `manual_screenshots_web/20260206_025805/trends_related_sensors_scanning.png`.
    - 2026-02-06: Follow-up fix shipped in same Tier A run: removed Related Sensors matrix auto-refresh submit loop that caused layout jitter while idle.
  - **Status:** Done (validated on installed controller `0.1.9.254-matrix-refresh-fix`; clean-host E2E deferred to DW-98)

- **DW-224: Trends: add separate Selected Sensors correlation matrix card**
  - **Description:** Add a second correlation matrix surface in Trends for the current Sensor picker selection, separate from Related Sensors.
  - **Acceptance Criteria:**
    - Trends page includes a distinct “Selected Sensors Correlation Matrix” card outside the Related Sensors section.
    - Matrix computes pairwise correlations for selected sensors in the current range/interval window.
    - Empty state is clear when fewer than 2 sensors are selected.
    - `cd apps/dashboard-web && npm run lint` passes.
    - `cd apps/dashboard-web && npm run test:smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validated on installed controller; Tier B deferred to `DW-98` if not completed in the same cycle.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-06: Implemented `SelectedSensorsCorrelationMatrixCard` and mounted it in Trends outside Related Sensors.
    - 2026-02-06: Local validation passed: `cd apps/dashboard-web && npm run lint`, `cd apps/dashboard-web && npm run test:smoke`, `cd apps/dashboard-web && npm run build`.
    - 2026-02-06: Tier A validated on installed controller `0.1.9.254-matrix-refresh-fix`; run: `project_management/runs/RUN-20260206-tier-a-dw223-dw224-matrix-refresh-fix-0.1.9.254-matrix-refresh-fix.md`; viewed screenshot: `manual_screenshots_web/20260206_025805/trends_selected_sensors_matrix_card_result.png`.
  - **Status:** Done (validated on installed controller `0.1.9.254-matrix-refresh-fix`; clean-host E2E deferred to DW-98)

- **DW-225: Trends Related Sensors preview: choose representative episode by default + warn on sparse episodes**
  - **Description:** Fix Related Sensors preview confusion when the default selected episode is extremely sparse (dot-only candidate series) by selecting a more representative episode automatically and warning when the current episode has very few points/coverage.
  - **Acceptance Criteria:**
    - When a candidate has `episodes`, Related Sensors preview auto-selects a default episode by ranking episodes with: max `coverage`, then max `num_points`, then max `score_peak` (stable tie-break).
    - When the selected episode is sparse (low coverage and/or very few points), preview renders a clear banner explaining why the chart may show dots/flat lines and how to fix (pick another episode, widen range, switch Raw view).
    - Episode selection remains user-overridable by clicking an episode button.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no design drift; token set).
    - `make ci-web-smoke` passes.
    - `cd apps/dashboard-web && npm run build` passes.
    - Tier A validation is intentionally deferred (do not rebuild/refresh yet).
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-07: Started (investigating dot-only preview + default episode selection).
    - 2026-02-07: Implemented representative default episode selection + sparse-episode warning banner:
      - `apps/dashboard-web/src/features/trends/utils/episodeSelection.ts` (coverage → points → peak tie-break)
      - `apps/dashboard-web/src/features/trends/components/relationshipFinder/PreviewPane.tsx` (per-candidate episode selection + warning banner)
      - `apps/dashboard-web/src/features/trends/utils/candidateNormalizers.ts` (Similarity badges: add `|r|` + diurnal note)
    - 2026-02-07: Local validation passed:
      - `make ci-web-smoke-build`
      - `cd apps/dashboard-web && npm test -- --run tests/episodeSelection.test.ts`
    - 2026-02-07: Tier A validated on installed controller `0.1.9.255-related-diurnal-penalty` (no DB/settings reset):
      - Installed smoke: `make e2e-installed-health-smoke` (PASS)
      - Screenshot sweep captured + viewed: `manual_screenshots_web/20260207_000801/trends_related_sensors_scanning.png`
      - Tier A ticket: `project_management/tickets/TICKET-0050-tier-a:-tsse-37-+-dw-225-related-sensors-diurnal-penalty-+-preview-defaults-(0.1.9.255).md`
  - **Status:** Done (validated on installed controller `0.1.9.255-related-diurnal-penalty`; clean-host E2E deferred to DW-98)

- **DW-226: Trends Related Sensors preview: make episode chart x-axis interpretable**
  - **Description:** The Related Sensors preview episode chart is often too zoomed-in on the x-axis (showing an ultra-narrow episode window), which produces confusing visuals like a single dot for one sensor and a flat line for another. Expand the preview window around the selected episode by default and visually highlight the true episode region.
  - **Acceptance Criteria:**
    - Related Sensors preview requests a wider time window around the selected episode by default (Auto context) so typical sensors show multiple points and the chart is interpretable.
    - Preview exposes a `Context` control with at least: `Auto`, `Episode`, `±6h`, `±24h`, `±72h`.
    - Preview visually highlights the actual episode window (shaded band + start/end markers) even when context is widened.
    - Preview charts do not render the Highcharts navigator (maximize plot area for the preview card).
    - `make ci-web-smoke-build` passes.
    - Tier A validated on installed controller (no DB/settings reset); Tier B deferred to `DW-98`.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-07: Started (Related Sensors episode preview too zoomed-in on x-axis; dot-only + flat-line episodes not interpretable).
    - 2026-02-07: Implemented preview context widening + episode highlight:
      - `apps/dashboard-web/src/features/trends/components/relationshipFinder/PreviewPane.tsx` (Context select; expands `episode_start_ts/end_ts` around selected episode; adds episode band + boundary lines)
      - `apps/dashboard-web/src/components/TrendChart.tsx` (supports `navigator` toggle + x-axis plot bands/lines)
    - 2026-02-07: Local validation passed: `make ci-web-smoke-build`.
    - 2026-02-07: Tier A validated on installed controller `0.1.9.256-related-preview-context` (no DB/settings reset):
      - Run: `project_management/runs/RUN-20260207-tier-a-dw226-related-preview-context-0.1.9.256-related-preview-context.md`
      - Installed smoke: `make e2e-installed-health-smoke` (PASS)
      - Screenshot sweep captured + viewed:
        - `manual_screenshots_web/20260207_004704/trends_related_sensors_scanning.png`
        - `manual_screenshots_web/20260207_004704/trends_related_sensors_large_scan.png`
      - Bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.256-related-preview-context.dmg`
      - Bundle log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.256-related-preview-context.log`
  - **Status:** Done (validated on installed controller `0.1.9.256-related-preview-context`; clean-host E2E deferred to DW-98)

- **DW-227: Trends sensor picker checkbox selection parity**
  - **Description:** Fix Trends sensor picker selection so operators can toggle sensors using either the sensor row card (existing behavior) or the checkbox itself. Keep row-click selection intact while making checkbox interaction reliable and preventing double-toggle behavior.
  - **Acceptance Criteria:**
    - In Trends sensor picker, clicking a sensor row card still toggles that sensor on/off.
    - Clicking the sensor checkbox also toggles that sensor on/off.
    - Checkbox, label, and row interactions do not double-toggle (single user click = one state transition).
    - Max-selection guard (`20`) behavior remains unchanged.
    - Regression coverage is added/updated for row-click + checkbox-click toggling.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - `cd apps/dashboard-web && npm run lint` passes.
    - `cd apps/dashboard-web && npm run test -- tests/trendsPage.test.tsx` passes.
    - `cd apps/dashboard-web && npm run test -- tests/smoke.test.ts` passes.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-09: Implemented shared sensor toggle handler in `TrendsPageClient` and rewired sensor row + checkbox interactions so both paths toggle selection without regressions.
    - 2026-02-09: Added Trends page regression test covering row-click and checkbox-click parity (`apps/dashboard-web/tests/trendsPage.test.tsx`).
    - 2026-02-09: Local validation passed:
      - `cd apps/dashboard-web && npm run lint`
      - `cd apps/dashboard-web && npm run test -- tests/trendsPage.test.tsx`
      - `cd apps/dashboard-web && npm run test -- tests/smoke.test.ts`
      - `cd apps/dashboard-web && npm run build`
  - **Status:** In Progress (implemented + locally validated; Tier A pending)

- **DW-228: Trends Pattern & Anomaly layout parity + Related Sensors context presets expansion**
  - **Description:** Fix layout regressions in Trends `Pattern & Anomaly Detector` charts so they follow the same chart-card layout pattern as the Related Sensors context graph, and expand Related Sensors preview context controls with finer presets plus a custom symmetric window.
  - **Acceptance Criteria:**
    - `Pattern & Anomaly Detector` chart cards in `MatrixProfilePanel` use consistent `min-w-0` chart-card containers and stable fixed chart regions matching the Related Sensors context graph card pattern (no overflow/collapse regressions).
    - Related Sensors `Context` selector includes `Auto`, `Episode`, `±1h`, `±3h`, `±6h`, `±24h`, `±72h`, and `Custom…`.
    - Selecting `Custom…` reveals a `Custom ±hours` numeric control (0.1..168, clamped on blur) that applies symmetric context padding around the selected episode.
    - Existing preview behavior remains intact: episode highlight bands/lines, lag-alignment fallback messaging, and computed-through clipping.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - `cd apps/dashboard-web && npm run lint` passes.
    - `cd apps/dashboard-web && npm run test -- tests/trendsPage.test.tsx` passes.
    - `cd apps/dashboard-web && npm run test -- tests/smoke.test.ts` passes.
    - `cd apps/dashboard-web && npm run build` passes.
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-09: Started from operator report: Trends Pattern/Anomaly chart layout errors and missing Related Sensors context ranges.
    - 2026-02-09: Implemented chart-card layout parity updates in `apps/dashboard-web/src/features/trends/components/MatrixProfilePanel.tsx` (stable chart containers + test ids for key chart regions).
    - 2026-02-09: Implemented Related Sensors context preset expansion + custom `±hours` control in `apps/dashboard-web/src/features/trends/components/relationshipFinder/PreviewPane.tsx`.
    - 2026-02-09: Local validation passed:
      - `cd apps/dashboard-web && npm run lint`
      - `cd apps/dashboard-web && npm run test -- tests/trendsPage.test.tsx`
      - `cd apps/dashboard-web && npm run test -- tests/smoke.test.ts`
      - `cd apps/dashboard-web && npm run build`
    - 2026-02-09: Attempted targeted Playwright validation (`trends-auto-compare.spec.ts`, `trends-matrix-profile.spec.ts`) but existing stub sensor-picker interaction remained unstable in this environment (selection not transitioning to `1/20 selected`).
  - **Status:** In Progress (implemented + locally validated; Tier A pending)

- **DW-229: Dashboard Web: standardize Highcharts render path with shared `HighchartsPanel` wrapper**
  - **Description:** Audit and replace ad-hoc `HighchartsReact` container blocks across dashboard-web with a thin shared wrapper (`HighchartsPanel`) so chart rendering uses one consistent path for container sizing, ref wiring, optional zoom-reset-on-double-click, and guardrailed imports.
  - **Acceptance Criteria:**
    - Add `apps/dashboard-web/src/components/charts/HighchartsPanel.tsx` as the canonical wrapper around `highcharts-react-official` with:
      - default `h-full w-full` container behavior
      - pass-through `options` + `constructorType`
      - optional `resetZoomOnDoubleClick`
      - optional `enableAutoReflow`
      - escape hatches (`containerProps`, `containerStyle`, `containerClassName`, `centeredMaxWidthPx`)
    - Migrate all dashboard-web direct `HighchartsReact` callsites to use `HighchartsPanel`:
      - `TrendChart.tsx`
      - `features/analytics/components/AnalyticsShared.tsx` (`ZoomableLineChart`, `ZoomableBarChart`)
      - `features/compensation/components/TemperatureCompensationCharts.tsx`
      - `features/overview/components/LocalSensorVisualizations.tsx`
      - `features/trends/components/MatrixProfilePanel.tsx`
      - `features/trends/components/relationshipFinder/CorrelationMatrix.tsx`
      - `features/trends/components/relationshipFinder/CorrelationPreview.tsx`
      - `features/trends/components/VoltageQualityPanel.tsx`
    - Add lint guardrail to block new direct `highcharts-react-official` imports outside `HighchartsPanel`.
    - Add unit coverage for wrapper behavior (`apps/dashboard-web/tests/highchartsPanel.test.tsx`).
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails (no intentional redesign; only parity refactor + bugfix-level sizing/reflow consistency).
    - Local validation passes:
      - `cd apps/dashboard-web && npm run lint`
      - `cd apps/dashboard-web && npm run test -- tests/highchartsPanel.test.tsx`
      - `cd apps/dashboard-web && npm run test -- tests/trendsPage.test.tsx`
      - `cd apps/dashboard-web && npm run test -- tests/smoke.test.ts`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-09: Implemented `HighchartsPanel` wrapper and migrated all direct dashboard-web Highcharts callsites to it while preserving existing chart semantics.
    - 2026-02-09: Added ESLint `no-restricted-imports` guard for `highcharts-react-official` with `HighchartsPanel.tsx` allowlist.
    - 2026-02-09: Added wrapper tests in `apps/dashboard-web/tests/highchartsPanel.test.tsx`.
    - 2026-02-09: Local validation passed:
      - `cd apps/dashboard-web && npm run lint`
      - `cd apps/dashboard-web && npm run test -- tests/highchartsPanel.test.tsx`
      - `cd apps/dashboard-web && npm run test -- tests/trendsPage.test.tsx`
      - `cd apps/dashboard-web && npm run test -- tests/smoke.test.ts`
      - `cd apps/dashboard-web && npm run build`
  - **Status:** In Progress (implemented + locally validated; Tier A pending)

- **DW-230: Trends chart analysis toolbar v2 (best-fit drag, multi-window, explicit save)**
  - **Description:** Redesign the Trends chart analysis UX to make best-fit analysis discoverable and professional: replace the click-start/click-end flow with drag-to-select windows, support multiple fit windows in one session, and add explicit draft/save behavior backed by chart annotations.
  - **Acceptance Criteria:**
    - Best-fit interaction is drag-based on the chart (no two-click start/end state machine); duplicate “Start best fit” entrypoints are removed.
    - Multiple best-fit windows can be created, edited, and removed in one session.
    - Best-fit rows show clear draft/saved/unsaved-changes states and support explicit Save actions.
    - Saved best-fit windows persist through `/api/chart-annotations` as typed `best_fit_v1` payloads and rehydrate on page reload.
    - Toolbar IA is reorganized for clarity (primary analysis vs secondary tools) with a desktop-first layout and condensed mobile secondary-tools surface.
    - Annotation save/delete/update failures are surfaced to users (no silent failures for best-fit persistence actions).
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Local validation passes:
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm run test -- trendChartBestFit.test.tsx trendsPage.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-09: Reworked `TrendChart` best-fit flow to use drag-selection via Highcharts `selection` events and removed the duplicate “Start best fit” control.
    - 2026-02-09: Added draft/saved fit lifecycle with explicit save/update/delete wiring through chart annotations and persisted fit hydration from `TrendsPageClient`.
    - 2026-02-09: Redesigned toolbar IA into primary/secondary groups with condensed mobile secondary controls and clearer active-tool messaging.
    - 2026-02-09: Added regression coverage in `apps/dashboard-web/tests/trendChartBestFit.test.tsx`.
    - 2026-02-09: Local validation passed:
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm run test -- trendChartBestFit.test.tsx trendsPage.test.tsx`
      - `cd apps/dashboard-web && npm run build`
    - 2026-02-09: Tier A validated on installed controller `0.1.9.259-dw230-trends-bestfit` (no DB/settings reset):
      - Run: `project_management/runs/RUN-20260209-tier-a-dw230-trends-bestfit-0.1.9.259-dw230-trends-bestfit.md`
      - Installed smoke: `make e2e-installed-health-smoke` (PASS)
      - Screenshot sweep captured + viewed:
        - `manual_screenshots_web/20260209_005651/trends.png`
      - Bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.259-dw230-trends-bestfit.dmg`
      - Bundle log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.259-dw230-trends-bestfit.log`
  - **Status:** Done (validated on installed controller `0.1.9.259-dw230-trends-bestfit`; clean-host E2E deferred to DW-98)

- **DW-231: Trends Related Sensors: operator contract + UI copy-labeling cleanup (TICKET-0069)**
  - **Description:** Related Sensors Unified v2 labels (“Score”, “Confidence”, raw co-occurrence magnitudes) were easy to misread as probability, statistical significance, or causality. Codify a strict operator contract and update the UI copy/labels/tooltips so the panel is precision-biased, self-disclosing about candidate coverage + effective interval, and avoids misleading magnitude-like pills.
  - **References:**
    - `project_management/tickets/TICKET-0069-related-sensors:-operator-contract-+-ui-copy-labeling-cleanup-(rank-score,-evidence,-coverage).md`
  - **Acceptance Criteria:**
    - UI uses `Rank score` and `Evidence` consistently for Unified v2 surfaces (no `Score`/`Confidence`/`Blend` duplication).
    - Coverage and interval disclosure are visible after a run:
      - `Evaluated: <evaluated_count> of <eligible_count> eligible sensors (limit: <candidate_limit_used>).`
      - `Effective interval: <interval_seconds_eff> (requested: <interval_seconds_requested>).`
    - Co-occurrence magnitude is not shown as an unbounded “Co-occur: <huge number>” pill; list rows show `Shared buckets` and `Co-occ strength` (0–1) instead.
    - Correlation block is titled `Correlation (bucketed levels, not used for ranking)` and is collapsed by default in Simple mode.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented operator contract copy + badge/tooltip relabeling (Rank score + Evidence), added coverage/interval disclosure, reframed correlation block, and added Vitest coverage (`apps/dashboard-web/tests/relatedSensorsOperatorContract.test.tsx`).
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-232: Trends Related Sensors: candidate pool transparency + why-not-evaluated diagnostics (TICKET-0054)**
  - **Description:** Related Sensors Unified v2 is pool-relative and limit-bounded; make candidate coverage and truncation explicit and provide a deterministic “why not evaluated?” diagnostic for missing sensors.
  - **References:**
    - `project_management/tickets/TICKET-0054-related-sensors:-candidate-pool-transparency-+-why-not-evaluated-diagnostics.md`
  - **Acceptance Criteria:**
    - Coverage disclosure is visible after every run and uses backend-provided `limits_used` + evaluated counts (no inference from UI settings).
    - Unified v2 result contract exposes stable truncation semantics:
      - `truncated_candidate_sensor_ids` (candidate-limit truncation)
      - `truncated_result_sensor_ids` (`max_results` truncation)
    - “Why not evaluated?” explains: filtered out vs not evaluated (limit) vs evaluated-below-threshold vs provider/forecast no-lake-history.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsUnifiedDiagnostics.test.ts`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added explicit Unified v2 `limits_used` + split truncation arrays and surfaced them in Trends via evaluated/eligible disclosure + deterministic diagnostics card.
    - 2026-02-10: Added deterministic diagnostics coverage: `apps/dashboard-web/tests/relatedSensorsUnifiedDiagnostics.test.ts`.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsUnifiedDiagnostics.test.ts`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-233: Trends Related Sensors: provider/forecast sensors availability UX (TICKET-0072)**
  - **Description:** Provider/forecast sensors typically have no stored lake history for relationship analysis. Exclude them by default in Simple mode, add an Advanced toggle to include them, and surface explicit “not available” diagnostics rather than silent skips.
  - **References:**
    - `project_management/tickets/TICKET-0072-related-sensors:-provider-and-forecast-sensors-availability-ux.md`
  - **Acceptance Criteria:**
    - Simple mode excludes provider/forecast sensors from the candidate pool and includes copy explaining availability.
    - Advanced mode provides toggle “Include provider/forecast sensors (may have no history)”.
    - When providers are included, Unified v2 returns explicit `skipped_candidates` diagnostics and the UI surfaces `Not available for relationship analysis (no stored history).`
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsProviderAvailability.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added `skipped_candidates` to Unified v2 result and surfaced provider/forecast availability in the Trends Relationship Finder (default exclude + Advanced include toggle + explicit labeling).
    - 2026-02-10: Added UI regression coverage: `apps/dashboard-web/tests/relatedSensorsProviderAvailability.test.tsx`.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsProviderAvailability.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-234: Unified v2: gap-aware deltas + z-magnitude scoring cap + gap counters (TICKET-0051)**
  - **Description:** Harden event/co-occ scoring against telemetry gaps and extreme z outliers by adding gap-aware deltas, quantization-aware robust scale floor, z-magnitude caps for scoring, and per-sensor gap counters surfaced in results.
  - **References:**
    - `project_management/tickets/TICKET-0051-unified-v2:-gap-aware-deltas-+-z-magnitude-scoring-cap-+-gap-counters.md`
  - **Acceptance Criteria:**
    - Deltas across large time gaps do not produce events (`gap_max_buckets` suppression).
    - Scoring uses `z_cap` for episode metrics and co-occurrence severity aggregation.
    - Quantized/step-like sensors do not inflate to extreme z-scores solely due to degenerate scale.
    - Results expose per-sensor `gap_skipped_deltas` for explainability.
    - Validation passes:
      - `make ci-core-smoke`
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented gap-aware event detection, quantization-aware scale floor, `z_cap` scoring, and per-sensor gap counters; added Rust regressions for each.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`.
  - **Status:** Done (validated locally)

- **DW-235: Unified v2: downweight system-wide co-occurrence buckets (TICKET-0052)**
  - **Description:** Update co-occurrence bucket scoring to downweight global “everyone spiked” buckets using downweight + IDF, preserving explainability in the payload.
  - **References:**
    - `project_management/tickets/TICKET-0052-unified-v2:-downweight-system-wide-co-occurrence-buckets.md`
  - **Acceptance Criteria:**
    - Co-occurrence bucket score decreases as `group_size` increases and approaches 0 when `group_size == N`.
    - Payload includes `idf` for explainability.
    - Validation passes:
      - `make ci-core-smoke`
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Replaced co-occ bucket scoring with downweight + IDF scheme and added Rust regressions to prevent global-bucket dominance.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`.
  - **Status:** Done (validated locally)

- **DW-236: Unified v2: auto bucket aggregation for event and co-occ ranking (TICKET-0070)**
  - **Description:** Align Unified v2 ranking with correlation matrix semantics by using auto bucket aggregation (Sum/Last/Avg by sensor type) for event-match and co-occurrence reads.
  - **References:**
    - `project_management/tickets/TICKET-0070-unified-v2:-auto-bucket-aggregation-for-event-and-co-occ-ranking.md`
  - **Acceptance Criteria:**
    - Event-match and co-occurrence jobs use `auto` aggregation by default.
    - State/bool sensors use `Last`, counter-like sensors use `Sum`, and continuous sensors use `Avg`.
    - Validation passes:
      - `make ci-core-smoke`
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Switched Unified v2 event/co-occ ranking to read bucket series with auto aggregation and added a DuckDB regression for aggregation semantics.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`.
  - **Status:** Done (validated locally)

- **DW-237: Unified v2: tolerant event alignment matching (tolerance buckets) + efficient matcher (TICKET-0053)**
  - **Description:** Make event alignment robust to sampling jitter by matching change events within a configured tolerance window (in buckets) after applying lag, while enforcing one-to-one matching so overlap cannot be inflated.
  - **References:**
    - `project_management/tickets/TICKET-0053-unified-v2:-tolerant-event-alignment-matching-(tolerance-buckets)-+-efficient-matcher.md`
  - **Acceptance Criteria:**
    - `EventMatchJobParamsV1` accepts optional `tolerance_buckets` (default `0` for backwards-compatible exact matching).
    - Event overlap and F1 scoring use tolerant one-to-one matching at each lag (two-pointer walk over sorted event times).
    - Episodes use the same tolerance semantics as overlap so counts are internally consistent.
    - Unified v2 passes `tolerance_buckets` through to `event_match_v1`.
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke-build`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented tolerant event matching in `event_match_v1` (one-to-one two-pointer matcher) and wired `tolerance_buckets` through Unified v2; updated UI tooltip copy to reflect tolerance semantics and added Rust unit coverage for tolerance + one-to-one behavior.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke-build`.
  - **Status:** Done (validated locally)

- **DW-238: Unified v2: directionality same vs opposite computation and UI (TICKET-0071)**
  - **Description:** Add an explicit directionality label (`same`/`opposite`/`unknown`) to Unified v2 candidates so operators can distinguish sensors that move with vs against the focus at the best lag.
  - **References:**
    - `project_management/tickets/TICKET-0071-unified-v2:-directionality-same-vs-opposite-computation-and-ui.md`
  - **Acceptance Criteria:**
    - Backend computes per-candidate directionality at `best_lag_sec` using tolerant matched-event sign agreement and (when `matched_pairs >= 5`) signed delta Pearson correlation on aligned bucket deltas.
    - Unified v2 evidence payload includes optional fields: `direction_label`, `sign_agreement`, `delta_corr`, `direction_n`.
    - Trends Related Sensors preview surfaces `Direction: same|opposite|unknown` with a tooltip including the metrics used.
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Computed directionality in `event_match_v1` and merged fields into Unified v2 evidence; surfaced Direction in the preview evidence summary with tooltip diagnostics; added synthetic Rust tests and updated dashboard-web contract tests.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-239: Trends Related Sensors: contract test suite (gap, aggregation, lag sign, derived labeling) (TICKET-0068)**
  - **Description:** Add deterministic contract tests that protect interpretation-critical Related Sensors semantics (pool-relative rank scores, gap suppression, aggregation mapping, lag sign, and derived-from-focus labeling) so future iterations can’t silently regress operator expectations.
  - **References:**
    - `project_management/tickets/TICKET-0068-related-sensors:-contract-test-suite-(gap,-aggregation,-lag-sign,-derived-labeling).md`
  - **Acceptance Criteria:**
    - Rust contract tests cover:
      - pool-relative normalization behavior (rank scores change when pool changes)
      - aggregation auto mapping normalization (type tokenization)
      - lag sign semantics (positive lag ⇒ candidate later)
      - integration harness runs Unified v2 over synthetic buckets (no external lake dependency)
    - Dashboard-web contract tests cover:
      - derived-from-focus detection is correct and bounded (cycle-safe)
      - Unified candidate normalization surfaces the derived-dependency badge
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsContractSuite.test.ts`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added Rust contract tests for lag sign, aggregation type normalization, pool-relative ranking behavior, and a deterministic Unified v2 synthetic harness.
    - 2026-02-10: Added dashboard-web derived dependency detection helper + contract tests covering bounded derived-from-focus detection and visible labeling.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsContractSuite.test.ts`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-240: Trends Related Sensors: workflow improvements (scope defaults, filters, jump-to-timestamp, matched events) (TICKET-0062)**
  - **Description:** Improve the Related Sensors workflow for operator triage: default Simple to same-node scope with an explicit broadening action, add quick filters, separate system-wide co-occurrence buckets into their own panel with jump-to-timestamp actions, and add explainability overlays/coverage metrics for matched events.
  - **References:**
    - `project_management/tickets/TICKET-0062-related-sensors:-workflow-improvements-(scope-defaults,-filters,-jump-to-timestamp,-matched-events).md`
  - **Acceptance Criteria:**
    - Simple mode defaults to `Same node` with a clear “Broaden to all nodes” action; Simple all-nodes applies stricter defaults (higher `z_threshold` for quick suggest and `min_sensors >= 3`).
    - Quick filters exist for: same unit, same type, exclude derived-from-focus, exclude system-wide buckets.
    - Operators can review system-wide buckets in a separate “System-wide events” panel (not conflated with related sensors), with jump-to-timestamp ±1h actions.
    - Preview overlays show detected focus/candidate events with a “show matched events only” toggle and coverage metrics (% focus matched, % candidate matched, % shared buckets).
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke-build`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsWorkflowImprovements.test.tsx`
      - `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3000 npm run test:playwright -- trends-related-sensors-jump.spec.ts`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented Simple scope defaults + broadening action, quick filters, system-wide events panel, jump-to-timestamp wiring (Custom ±1h), and preview event overlays/coverage metrics; added Vitest unit tests and Playwright stub coverage for jump-to-timestamp.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke-build`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsWorkflowImprovements.test.tsx`, `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3000 npm run test:playwright -- trends-related-sensors-jump.spec.ts`.
  - **Status:** Done (validated locally)

- **DW-241: Unified v2: derived sensor dependency labeling + default exclude in Simple (TICKET-0057)**
  - **Description:** Prevent tautological “top matches” by making Unified v2 dependency-aware: label candidates that are derived from the focus sensor (directly or transitively) and exclude them by default in Simple mode while keeping an explicit include toggle in Advanced.
  - **References:**
    - `project_management/tickets/TICKET-0057-unified-v2:-derived-sensor-dependency-labeling-+-default-exclude-in-simple.md`
  - **Acceptance Criteria:**
    - Backend annotates Unified v2 candidates with:
      - `derived_from_focus: bool`
      - `derived_dependency_path: [sensor_id...]` (optional; capped length)
    - UI shows a `Dependency: Derived from focus` badge and excludes derived-from-focus candidates by default in Simple mode.
    - Advanced mode provides an explicit toggle: `Include derived-from-focus candidates`.
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke-build`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsContractSuite.test.ts tests/relatedSensorsWorkflowImprovements.test.tsx`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added backend derived dependency detection + path annotation for Unified v2 candidates; updated Simple/Advanced UI defaults and toggles; updated/added tests to cover default exclusion and Advanced include behavior.
    - 2026-02-10: Local validation passed (see ticket validation section).
  - **Status:** Done (validated locally)

- **DW-242: Unified v2: fix candidate truncation bias (hashed ordering + coverage prefilter) (TICKET-0055)**
  - **Description:** Remove lexicographic candidate truncation bias in Unified v2 by ordering candidates deterministically via focus-relative priority groups + stable hash, adding a lightweight bucket-coverage prefilter to skip sparse series, and ensuring the co-occurrence stage does not reintroduce biased truncation. Update dashboard-web to submit full eligible IDs and keep job keys bounded via candidate-list hashing.
  - **References:**
    - `project_management/tickets/TICKET-0055-unified-v2:-fix-candidate-truncation-bias-(hashed-ordering-+-metadata-prefilter).md`
  - **Acceptance Criteria:**
    - Backend uses deterministic priority-group + hash ordering for candidate selection/truncation (no sensor-id prefix bias).
    - Coverage prefilter drops candidates with insufficient history/continuity (`bucket_rows >= 3`, `delta_count >= 3`) and reports prefilter/truncation summaries in the result payload.
    - Co-occurrence stage selects an explicit deterministic subset (≤ `max_sensors_used`) rather than relying on `cooccurrence_v1` truncation.
    - Simple UI submits the full eligible candidate list (no `.sort().slice(...)` truncation), and job keys remain bounded via hashing.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented hashed candidate ordering + minimum coverage prefilter for Unified v2 candidates (plus `prefiltered_candidate_sensor_ids` + timings/counts); updated dashboard-web to submit full eligible IDs while hashing the candidate set into `job_key`; added Rust + dashboard-web regressions for deterministic ordering and diagnostics.
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`.
  - **Status:** Done (validated locally)

- **DW-243: Trends Related Sensors: pin semantics (pinned candidates always evaluated) (TICKET-0056)**
  - **Description:** Allow operators to pin specific sensors so they are always evaluated by Unified v2 even when the eligible candidate pool is truncated by candidate limits/caps, improving troubleshooting workflows without forcing “evaluate all”.
  - **References:**
    - `project_management/tickets/TICKET-0056-related-sensors:-pin-semantics-(pinned-candidates-always-evaluated).md`
  - **Acceptance Criteria:**
    - Unified v2 job params support `pinned_sensor_ids` and candidate selection ensures pinned sensors are included in the evaluated pool (excluding focus), with `candidate_limit_used = max(requested_limit, pinned_count)` clamped to 1000.
    - Trends UI supports pin/unpin from a result row and from a pinned section (searchable by sensor id) and includes `pinned_sensor_ids` in submitted job params for both Find and Refine runs.
    - Coverage disclosure includes pinned inclusion counts (e.g., `Pinned included: X`).
    - Why-not-evaluated diagnostics treat pinned sensors as submitted candidates and do not mis-classify them as “not in candidate list”.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsPinnedSemantics.test.tsx tests/relatedSensorsUnifiedDiagnostics.test.ts`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added `pinned_sensor_ids` to `RelatedSensorsUnifiedJobParamsV2` and enforced pinned inclusion semantics in `related_sensors_unified_v2` (candidate_limit_used max + hard cap 1000; pinned bypasses coverage prefilter; provider/no-history pinned candidates surface via `skipped_candidates`).
    - 2026-02-10: Added pin/unpin UX to Trends Relationship Finder (Pinned section + row actions), surfaced pinned counts in coverage disclosure, and updated diagnostics to treat pinned ids as submitted candidates.
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsPinnedSemantics.test.tsx tests/relatedSensorsUnifiedDiagnostics.test.ts`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-244: Trends Related Sensors: show top lag candidates (top 3 lags) (TICKET-0076)**
  - **Description:** Surface the top 3 lag hypotheses (F1 + overlap) so operators can see when lag alignment is ambiguous and choose an alternative lag for preview alignment without affecting ranking.
  - **References:**
    - `project_management/tickets/TICKET-0076-related-sensors:-show-top-lag-candidates-(top-3-lags).md`
  - **Acceptance Criteria:**
    - Backend `event_match_v1` supports `top_k_lags` (default `0`) and returns `top_lags` (max 3) sorted by F1 desc, overlap desc, |lag| asc.
    - Unified v2 Advanced sets `top_k_lags=3` and carries `top_lags` into Unified candidate evidence.
    - Trends Related Sensors preview (Advanced) shows “Top lags” and provides a one-click “Use this lag for preview alignment” action per lag entry (chart-only; ranking unchanged).
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsTopLags.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented backend `top_k_lags`/`top_lags` output for event-match, plumbed top-lag evidence into Unified v2 Advanced, and added Advanced preview Top-lags UX + per-lag alignment override (preview-only).
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsTopLags.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-245: Trends Related Sensors: backend candidate pool query (all sensors by scope) (TICKET-0074)**
  - **Description:** Add an Advanced candidate-source selector so Unified v2 can evaluate eligible sensors by scope/filters via a backend DB query (no huge `candidate_sensor_ids` payloads), return backend coverage counts (`eligible_count`, `evaluated_count`), and support a bounded “evaluate all eligible” mode for small pools.
  - **References:**
    - `project_management/tickets/TICKET-0074-related-sensors:-backend-candidate-pool-query-(all-sensors-by-scope).md`
  - **Acceptance Criteria:**
    - Advanced UI provides a `Candidate source` selector:
      - `Visible in Trends`
      - `All sensors in scope (backend query)`
    - Backend supports `candidate_source` + empty `candidate_sensor_ids` to query candidates by filters, and returns:
      - `eligible_count`
      - `evaluated_count`
    - Advanced supports “Evaluate all eligible (may take longer)” and only enables it when `eligible_count <= 500`; backend enforces `candidate_limit_used = eligible_count` (within hard caps).
    - Coverage disclosure uses backend `eligible_count`/`evaluated_count` and includes candidate-source disclosure.
    - Validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx tests/relatedSensorsPinnedSemantics.test.tsx tests/relatedSensorsProviderAvailability.test.tsx tests/relatedSensorsUnifiedDiagnostics.test.ts tests/relatedSensorsWorkflowImprovements.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added backend candidate-source support (`visible_in_trends` vs `all_sensors_in_scope`), server-side candidate pool query by filters, backend coverage counts (`eligible_count`, `evaluated_count`), and bounded `evaluate_all_eligible` mode.
    - 2026-02-10: Updated Trends Relationship Finder Advanced to expose Candidate source selector + Evaluate-all toggle, switched disclosure to backend counts, and hardened “why not evaluated” diagnostics for backend-query mode.
    - 2026-02-10: Local validation passed: `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx tests/relatedSensorsPinnedSemantics.test.tsx tests/relatedSensorsProviderAvailability.test.tsx tests/relatedSensorsUnifiedDiagnostics.test.ts tests/relatedSensorsWorkflowImprovements.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-246: Trends Related Sensors: periodic and diurnal driver mitigation (deseasoning + low-entropy penalty) (TICKET-0073)**
  - **Description:** Reduce diurnal/periodic false positives in Unified v2 by supporting an optional deseasoning pre-step (hour-of-day mean residuals) and an optional time-of-day entropy penalty that downweights periodic sensors without changing event thresholds.
  - **References:**
    - `project_management/tickets/TICKET-0073-unified-v2:-periodic-and-diurnal-driver-mitigation-(deseasoning-and-low-entropy-penalty).md`
  - **Acceptance Criteria:**
    - Backend supports `deseason_mode` (`none` | `hour_of_day_mean`) and gates application to ≥2-day windows (otherwise skipped and labeled).
    - Backend computes time-of-day entropy over detected events, clamps `entropy_weight` to `[0.25, 1.0]`, applies it to event/co-occurrence scoring contributions, and surfaces entropy/weight in evidence.
    - Trends Related Sensors Advanced surfaces deseasoning + periodic penalty toggles with explicit copy: “Mitigate diurnal/periodic artifacts (may reduce true positives for truly periodic mechanisms).”
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsAdvancedMitigationControls.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added UTC hour-of-day mean residual deseasoning (≥2-day gate) and a periodic penalty based on normalized time-of-day entropy; plumbed through Unified v2 params + evidence and exposed Advanced controls + preview evidence.
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsAdvancedMitigationControls.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-247: Trends Related Sensors: delta correlation evidence channel (optional third signal) (TICKET-0075)**
  - **Description:** Add an optional third Unified v2 rank component based on absolute Pearson correlation of bucket deltas at best lag (`Δ corr`), guarded as Advanced-only and explicitly labeled to avoid “statistics authority bleed”.
  - **References:**
    - `project_management/tickets/TICKET-0075-unified-v2:-delta-correlation-evidence-channel-(optional-third-signal).md`
  - **Acceptance Criteria:**
    - Backend computes signed `delta_corr` on aligned bucket deltas at best lag, gates on ≥10 aligned delta pairs, and only uses it for ranking when `include_delta_corr_signal=true` (Advanced mode).
    - Backend renormalizes weights across enabled components and carries effective weights in returned params for transparency.
    - Trends Related Sensors preview surfaces `Δ corr` evidence with tooltip: “Signed correlation on bucket deltas at best lag. Not statistical significance. Not used for ranking unless enabled.”
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsAdvancedMitigationControls.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Added Advanced-only `include_delta_corr_signal` + `weights.delta_corr`, gated `delta_corr` evidence (≥10 aligned delta pairs), updated Unified blending, and surfaced `Δ corr` in preview evidence + Advanced controls.
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsAdvancedMitigationControls.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-248: Trends: correlation block refinements (delta corr, lagged corr, focus-vs-candidate default) (TICKET-0061)**
  - **Description:** Reduce correlation “authority bleed” by defaulting the correlation surface to a focus-vs-candidate list (easy scan) while keeping the full matrix as an explicit opt-in, and adding optional delta-correlation and bounded lag search modes.
  - **References:**
    - `project_management/tickets/TICKET-0061-trends:-correlation-block-refinements-(delta-corr,-lagged-corr,-focus-vs-candidate-default).md`
  - **Acceptance Criteria:**
    - Related Sensors correlation block defaults to a focus-vs-candidate list and keeps the full matrix as an explicit opt-in expansion.
    - Correlation controls include optional `value_mode` (`levels` vs `deltas`) and `lag_mode` (`aligned` vs `best_within_max` with bounded `max_lag_buckets`).
    - Tooltips/labels remain explicit that correlation is context only and not used for ranking; lag is surfaced when available.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
      - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsCorrelationBlockRefinements.test.tsx`
      - `cd apps/dashboard-web && npm run build`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented correlation-matrix backend support for deltas + bounded best-lag search, updated Related Sensors correlation block to list-first with matrix opt-in, and added dashboard-web regression coverage.
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsCorrelationBlockRefinements.test.tsx`, `cd apps/dashboard-web && npm run build`.
  - **Status:** Done (validated locally)

- **DW-249: Unified v2: data quality + missingness surfacing (TICKET-0058)**
  - **Description:** Ensure Unified v2 evidence is not driven by low-quality buckets or sparse bucket artifacts by applying a default quality policy + min-samples bucket gating, and surface deterministic bucket coverage/missingness in the preview (with explicit chart gaps).
  - **References:**
    - `project_management/tickets/TICKET-0058-unified-v2:-data-quality-+-missingness-surfacing.md`
  - **Acceptance Criteria:**
    - Bad-quality points are excluded per a documented default analysis policy.
    - Buckets with insufficient raw samples do not create events (default min samples per bucket: `>= 2`).
    - Preview surfaces missingness/coverage clearly (focus + candidate) and renders explicit gaps on missing buckets.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-core-smoke`
      - `make ci-web-smoke`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-10: Implemented analysis-default bucket read options (quality filter + min-samples), computed bucket coverage %, plumbed into Unified v2 evidence and preview series, and inserted explicit chart gaps when buckets are missing.
    - 2026-02-10: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-core-smoke`, `make ci-web-smoke`, `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsMissingnessPreview.test.tsx`.
    - 2026-02-10: Tier A validated installed controller `0.1.9.262-dw249-missingness` (run: `project_management/runs/RUN-20260210-tier-a-dw249-missingness-0.1.9.262-dw249-missingness.md`; viewed screenshot: `manual_screenshots_web/tier_a_0.1.9.262-dw249-missingness_unified_preview_2026-02-10_223632415Z/trends_related_sensors_unified_preview.png`).
  - **Status:** Done (Tier A validated installed `0.1.9.262-dw249-missingness`; Tier B deferred to `DW-98`)

- **DW-250: Unified v2: event detection enhancements (NMS, adaptive thresholds, polarity split, weighted F1) (TICKET-0059)**
  - **Description:** Upgrade Unified v2 event detection to reduce redundant near-duplicate events, improve stability across noise floors, and surface directional context (up/down) while keeping the evidence explainable to operators.
  - **References:**
    - `project_management/tickets/TICKET-0059-unified-v2:-event-detection-enhancements-(nms,-adaptive-thresholds,-polarity-split,-weighted-f1).md`
  - **Acceptance Criteria:**
    - Event detection supports NMS suppression, adaptive thresholds (optional), boundary-event labeling/exclusion (opt-in), ramp-ish detection (Δ², optional), and sparse point-events fallback (optional).
    - Event match uses weighted overlap (bounded by `z_cap`) and surfaces up/down event counts in evidence.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Implemented NMS + adaptive thresholds + boundary labeling + detector modes + weighted F1 plumbing and updated Unified v2 params/types. Local validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.266-dw250-253-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw250-dw251-dw252-dw253-0.1.9.266.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.266-dw250-253-related-sensors`; Tier B deferred to `DW-98`)

- **DW-251: Unified v2: co-occurrence scoring refinements (normalize, surprise, focus-centric, prevalence penalty) (TICKET-0060)**
  - **Description:** Refine the Unified v2 co-occurrence evidence channel so it is less dominated by single extreme buckets, less sensitive to selection artifacts, and less biased toward “events everywhere” candidates, while remaining operator-interpretable.
  - **References:**
    - `project_management/tickets/TICKET-0060-unified-v2:-co-occurrence-scoring-refinements-(normalize,-surprise,-focus-centric,-prevalence-penalty).md`
  - **Acceptance Criteria:**
    - Co-occurrence ranking uses average strength normalization and supports focus-centric top-bucket selection.
    - Optional “surprise” scoring and prevalence penalty exist (Advanced) and are bounded.
    - Operator toggle exists for bucket preference mode (“Prefer specific matches” default; “Prefer system-wide matches”).
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Implemented focus-centric co-occurrence bucket selection (rank by focus severity), co-occurrence avg normalization for Unified ranking, optional surprise scoring, and a prevalence penalty for high-rate candidates; added Advanced UI toggles (bucket preference + score mode). Validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS), `make ci-web-smoke` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.266-dw250-253-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw250-dw251-dw252-dw253-0.1.9.266.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.266-dw250-253-related-sensors`; Tier B deferred to `DW-98`)

- **DW-252: Related Sensors: offline evaluation harness + labeled set (TICKET-0063)**
  - **Description:** Add a deterministic offline evaluation harness + labeled case set so Related Sensors quality can be measured (precision@k, MRR, coverage) across scenarios and sensor types.
  - **References:**
    - `project_management/tickets/TICKET-0063-related-sensors:-offline-evaluation-harness-+-labeled-set.md`
  - **Acceptance Criteria:**
    - A developer can run the harness locally and get a deterministic Markdown/JSON report under `reports/`.
    - Labeled cases are data-driven (adding cases does not require code changes).
    - Baseline metrics exist for ≥10 cases across ≥3 sensor types.
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Added `related_sensors_eval` harness (API default + direct mode), data-driven labeled cases (`reports/related_sensors_eval/cases.json`), and captured baseline metrics (`reports/related_sensors_eval/baseline-20260211.{md,json}`). Local validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.266-dw250-253-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw250-dw251-dw252-dw253-0.1.9.266.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.266-dw250-253-related-sensors`; Tier B deferred to `DW-98`)

- **DW-253: Related Sensors: online instrumentation + UX metrics (TICKET-0064)**
  - **Description:** Emit privacy-safe interaction events for Related Sensors and define a small metric suite (time-to-first-action, refine rate, stability proxy, etc.) to assess operator usefulness online.
  - **References:**
    - `project_management/tickets/TICKET-0064-related-sensors:-online-instrumentation-+-ux-metrics.md`
  - **Acceptance Criteria:**
    - Events are emitted with stable schemas (no raw sensor values).
    - Local dev-only structured log sink exists for validation.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Validation passes:
      - `make ci-web-smoke`
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Added Related Sensors UX event emission + dev structured log sink (console + in-memory buffer) with stable schemas (`panel_opened`, `run_started`, `run_completed`, `candidate_opened`, `episode_selected`, `add_to_chart_clicked`, `refine_clicked`, `pin_toggled`, `jump_to_timestamp_clicked`). Local validation: `make ci-web-smoke` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.266-dw250-253-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw250-dw251-dw252-dw253-0.1.9.266.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.266-dw250-253-related-sensors`; Tier B deferred to `DW-98`)

- **DW-254: Related Sensors: rank stability scoring + monitoring (TICKET-0065)**
  - **Description:** Add opt-in stability scoring (subwindow reruns + overlap@k/Kendall) and monitoring outputs for pathological event evidence so rank flips can be detected and debugged without hurting production performance.
  - **References:**
    - `project_management/tickets/TICKET-0065-related-sensors:-rank-stability-scoring-+-monitoring.md`
  - **Acceptance Criteria:**
    - Stability score is deterministic, bounded in runtime, and surfaced as high/medium/low.
    - Monitoring outputs exist for evidence health (peak |Δz| percentiles, z_cap clipping %, gap suppression %).
    - No performance regressions on standard runs (stability is opt-in or bounded by pool size).
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Added opt-in stability scoring (`stability_enabled`) for Unified v2 (split window into thirds; rerun Unified v2 per subwindow; report overlap@10 score 0–1 and `high/medium/low` tier). Stability is skipped when `eligible_count > 120` to bound compute cost.
    - 2026-02-11: Added evidence health monitoring outputs: peak |Δz| percentiles (p50/p90/p95/p99), z-cap clipping %, and gap-suppression %; surfaced on `related_sensors_unified_v2` results and summarized in the Advanced UI.
    - Local validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS), `make ci-web-smoke` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.267-dw254-256-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw254-dw255-dw256-0.1.9.267-dw254-256-related-sensors.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.267-dw254-256-related-sensors`; Tier B deferred to `DW-98`)

- **DW-255: Trends: Pattern Detector integration with Related Sensors (TICKET-0066)**
  - **Description:** Integrate Pattern & Anomaly Detector and Related Sensors so operators can use detected anomaly windows as explicit Related Sensors focus events, and the UI discloses which evidence source drove ranking.
  - **References:**
    - `project_management/tickets/TICKET-0066-trends:-pattern-detector-integration-with-related-sensors.md`
  - **Acceptance Criteria:**
    - Pattern Detector can send focus events to Related Sensors in one click.
    - Related Sensors job accepts explicit focus event list (timestamps + optional severity) and results disclose source (delta-z vs pattern vs blend).
    - UI shows a simple shared-window indicator across both panels.
    - Dashboard-web changes comply with `apps/dashboard-web/AGENTS.md` UI/UX guardrails.
    - Validation passes:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
  - **Owner:** Core Analytics + Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Added `focus_events` support to Unified v2 + Event Match jobs (explicit RFC3339 timestamps + optional severity weight), with normalization (window clamp, dedupe, max-events bound) and deterministic tests.
    - 2026-02-11: Trends UI now supports one-click Pattern Detector → Related Sensors workflow via “Send to Related Sensors”; Related Sensors banner discloses explicit focus usage and results show `Evidence source` (delta‑z vs pattern vs blend). Matrix Profile lists show a simple “In Related Sensors” indicator for shared windows.
    - Local validation: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS), `make ci-web-smoke` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.267-dw254-256-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw254-dw255-dw256-0.1.9.267-dw254-256-related-sensors.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.267-dw254-256-related-sensors`; Tier B deferred to `DW-98`)

- **DW-256: Docs: operator-facing How Related Sensors works (TICKET-0067)**
  - **Description:** Write a single operator-facing “How it works” doc for Related Sensors with clear guardrails against interpreting rank/evidence as probability/significance, and link it from the Trends UI.
  - **References:**
    - `project_management/tickets/TICKET-0067-docs:-operator-facing-how-related-sensors-works.md`
  - **Acceptance Criteria:**
    - Doc exists under `docs/` with explicit “not causality / not probability / not significance” warnings.
    - Doc includes Advanced tooltip copy for key parameters.
    - UI includes a stable “How it works” link that matches current implementation semantics.
    - Validation passes:
      - `make ci-web-smoke`
  - **Owner:** Platform UI + Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-11: Added operator doc: `docs/related-sensors-operator-how-it-works.md` (≤120-word plain-language explainer, ASCII pipeline diagram, boxed warning, and tooltip copy).
    - 2026-02-11: Added in-app help page at `/analytics/trends/related-sensors-how-it-works` and linked it from the Trends Related Sensors “How it works” key; Advanced controls now include operator-grade tooltip text for key parameters.
    - Local validation: `make ci-web-smoke` (PASS).
    - 2026-02-11: Tier A validated installed controller `0.1.9.267-dw254-256-related-sensors` (run: `project_management/runs/RUN-20260211-tier-a-dw254-dw255-dw256-0.1.9.267-dw254-256-related-sensors.md`).
  - **Status:** Done (Tier A validated installed `0.1.9.267-dw254-256-related-sensors`; Tier B deferred to `DW-98`)

- **DW-69: Show “Active Development” banner during agent work**
  - **Description:** Display a prominent dashboard banner while Codex/engineering work is actively in progress, and auto-hide it when the work stops (turn ends) via an expiring marker.
  - **Acceptance Criteria:**
    - Dashboard shows an “Active Development” banner on all authenticated dashboard pages when the dev-activity marker is active.
    - Banner includes descriptive copy (“automated changes may occur”) and shows last heartbeat + auto-hide timing.
    - Marker is time-bound (TTL) and auto-expires even if it is not explicitly cleared.
    - Provide an operator/automation-friendly toggle (`farmctl dev-activity start|stop|status`) to manage the marker on the controller host.
    - API exposes marker status at `GET /api/dev/activity` (public) for the web UI to poll.
    - Marker write endpoints are localhost-only to avoid remote banner injection:
      - `POST /api/dev/activity/heartbeat`
      - `DELETE /api/dev/activity`
    - `cargo build --manifest-path apps/core-server-rs/Cargo.toml` and `cd apps/dashboard-web && npm run build` remain green; run `make ci-web-smoke` / `make e2e-web-smoke` from a clean state before marking Done.
  - **Notes / Run Log:**
    - 2026-01-07: Added `/api/dev/activity` + `/api/dev/activity/heartbeat` + `DELETE /api/dev/activity` (TTL-backed marker persisted server-side under `/Users/Shared/FarmDashboard/setup/dev_activity.json`) and dashboard banner polling via React Query.
    - 2026-01-07: Secured marker writes to localhost-only (no auth token required for local automation; remote calls rejected).
    - 2026-01-07: Added `farmctl dev-activity start|stop|status` to set/clear the marker by calling the localhost-only endpoints.
    - 2026-01-07: Build: `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (pass); `cd apps/dashboard-web && npm run build` (pass).
    - 2026-01-07: Bundled + upgraded installed controller to `0.1.9.28`; verified `GET /api/dev/activity` toggles via `POST /api/dev/activity/heartbeat` and clears via `DELETE /api/dev/activity`.
    - 2026-01-07: Renumbered from `DW-66` → `DW-69` after merging `feature/ios-watch-production` (task id `DW-66` is used by the Power tab work).
    - Test note: E2E not run on this host; clean-state test hygiene gate requires no running Farm launchd jobs/processes before/after the run.
  - **Status:** In Progress (tests/E2E gated by clean-state requirement on this host)

- **DW-77: Dashboard: edit node/sensor names + sensor display decimals (per-sensor + bulk by type)**
  - **Description:** Provide admin/engineer UX to rename nodes and sensors and control display precision per sensor, including a bulk action to apply a default precision per sensor type (per-sensor config remains source of truth).
  - **Acceptance Criteria:**
    - Nodes UI supports renaming a node (writes via existing node update endpoint; gated by `config.write`).
    - Sensors UI supports renaming a sensor and setting “Display decimals” for that sensor (persisted per sensor, not global).
    - Sensors UI provides a bulk “Set decimals by type” action that updates each matching sensor’s stored decimals value (individual sensors remain editable afterward).
    - Sensor value rendering across the dashboard respects per-sensor decimals where applicable (tables/tooltips/labels), with safe fallbacks.
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Added node rename UX (Node detail surfaces) and sensor rename + “Display decimals” controls (Sensor drawer) with a bulk “Set decimals…” modal by sensor type; map markers update labels live; tables/trends/tooltips respect per-sensor decimals via `sensor.config.display_decimals`. Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Status:** In Progress (implemented; validation gated by clean-state E2E)

- **DW-78: Dashboard IA cleanup (Overview tab + remove legacy catch-all sections)**
  - **Description:** Remove legacy “catch-all” UI that grew before dedicated tabs existed, and centralize cross-cutting status into a dedicated Overview tab.
  - **Acceptance Criteria:**
    - Dashboard includes a new `/overview` tab and it is the default post-login landing page.
    - “System overview” + schedule snapshot (SystemBanner) is shown on Overview only (not on Nodes/Schedules).
    - Setup Center no longer duplicates node adoption/discovery UI.
    - Connection tab no longer duplicates discovery scanning; it provides a clear link to Nodes for adoption workflows.
    - `cd apps/dashboard-web && npm run build` remains green; controller bundle upgrade refreshes the installed UI.
  - **Notes / Run Log:**
    - 2026-01-08: Added Overview tab + quick links; moved SystemBanner to Overview only; clarified the schedule empty-state CTA (“Open schedules”); removed adoption UI from Setup Center and discovery scan from Connection; added explicit Backups entry points from Nodes; refreshed the installed controller to `0.1.9.35`.
    - 2026-01-08: Sidebar nav polish: removed the confusing “live” badge from the Analytics nav item (badges are now reserved for counts/status) and stopped background polling of the analytics bundle outside the Analytics page. Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Status:** In Progress (implemented; validation gated by clean-state E2E)


- **DW-82: Enforce light mode (disable dark mode styling)**
  - **Description:** Force the dashboard UI to render in light mode regardless of the user’s OS/system theme preference. Dark mode currently causes rendering issues in several panels (notably Schedules).
  - **References:**
    - `apps/dashboard-web/src/app/globals.css`
  - **Acceptance Criteria:**
    - UI renders in light mode even when macOS is set to Dark appearance.
    - `dark:` Tailwind variants do not activate based on `prefers-color-scheme` (dark mode is opt-in only via an explicit `.dark` class, which the app does not set).
    - Schedules and other pages render consistently (no dark-mode-only regressions).
    - `cd apps/dashboard-web && npm run build` remains green.
  - **Notes / Run Log:**
    - 2026-01-08: Enforced light-only UX by switching Tailwind dark variant to class-based (no `.dark` applied) and removing `prefers-color-scheme: dark` overrides in global CSS; set `color-scheme: light`. Refreshed installed controller to `0.1.9.41`. Build: `cd apps/dashboard-web && npm run build` (pass).
  - **Status:** In Progress (implemented; validation gated by clean-state E2E)

- **DW-257: Setup Center + Power tab: battery capacity/SOC/runway UX (Renogy + ADC-hat load)**
  - **Description:** Add Setup Center controls to configure the battery model (sticker capacity + cutoff + anchoring) and the power runway projection (select the true load power sensor from ADS1263/ADC-hat). Surface estimated SOC, remaining Ah, and conservative runway (hours/days) in the Power tab for Renogy nodes.
  - **Acceptance Criteria:**
    - Setup Center includes a new “Battery & runway” section for Renogy nodes that supports:
      - enabling/disabling battery model + runway
      - entering sticker capacity (Ah) and cutoff SOC (%)
      - selecting one or more load power sensors (unit W) to represent true load (ADC-hat)
      - PV derate + history window + projection days
    - Power tab (Renogy cards) shows:
      - Estimated SOC (%), Renogy SOC (%), sticker capacity (Ah), remaining (Ah), runway (hours/days)
      - A clear label that runway is conservative beyond PV horizon (PV=0).
    - UI changes comply with `apps/dashboard-web/AGENTS.md` guardrails (page pattern, token set, hierarchy, shared components; no new design drift).
    - Local validation passes:
      - `make ci-web-smoke`
  - **Owner:** Platform UI (Codex)
  - **Notes / Run Log:**
    - 2026-02-18: Added Setup Center “Battery SOC + power runway” section (per-node cards) and updated Power tab Renogy summary to show estimated SOC, remaining/sticker Ah, and conservative runway. Local validation: `make ci-web-smoke` (PASS). Tier‑A validation tracked as DW-258.
  - **Status:** Done

### To Do
- **DW-258: Tier A validate battery/runway UI on installed controller**
  - **Description:** Validate the new Battery & Runway Setup Center section and Power tab runway readbacks on the installed controller (Tier A; no DB/settings reset), with screenshot evidence.
  - **Acceptance Criteria:**
    - Tier A refresh uses a known-good controller bundle build with the battery/runway features enabled.
    - Setup Center can save the Battery & Runway config for a Renogy node.
    - Power tab shows live estimated SOC and runway within 2 refresh intervals.
    - At least one screenshot is captured **and viewed** and stored under `manual_screenshots_web/`; referenced in a run log under `project_management/runs/`.
    - Screenshot gate passes:
      - `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-....md`
  - **Notes / Run Log:**
    - 2026-02-18: Tier A validated installed controller `0.1.9.274-battery-runway-fix` (run: `project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md`; screenshot gate: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md` PASS).
  - **Status:** Done

- **DW-196: shadcn Phase 5 — component upgrades (Switch, AlertDialog, Toast, ScrollArea, etc.)**
  - **Description:** Continue the shadcn/design-token migration by replacing remaining hand-rolled UI patterns with shadcn primitives. Phase 3 (gray→token color classes) and Phase 4 (CollapsibleCard + Radix) are complete. Phase 5 covers the remaining component upgrades identified in the mid-migration audit (`project_management/shadcn_mid_migration_audit.md`).
  - **Acceptance Criteria:**
    - Switch/Checkbox: 15+ raw `<input type="checkbox">` toggles migrated to shadcn Switch or Checkbox.
    - AlertDialog: 7+ `window.confirm()` calls replaced with shadcn AlertDialog.
    - Sonner (Toast): inline banner states in 8+ pages replaced with toast notifications where appropriate.
    - ScrollArea: 20+ overflow divs replaced with shadcn ScrollArea.
    - Accordion/Collapsible: 13+ `<details>` elements replaced with Radix primitives.
    - Tooltip: native `title` attributes replaced with shadcn Tooltip on key interactive elements.
    - `npm run lint && npm run test:smoke && npm run build` pass.
    - UI changes conform to `apps/dashboard-web/AGENTS.md` guardrails.
  - **Status:** To Do

### Deferred / Optional
- **DW-62: Add system topology tab (UniFi inventory + node association)**
  - **Description:** Provide a topology view that shows network infrastructure (from UniFi) and associates Pi 5 nodes by hostname/MAC so operators can see “what is connected where”.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - Dashboard includes a Topology tab that renders UniFi inventory/topology data when configured.
    - Pi 5 nodes are correlated to UniFi devices by MAC/hostname and mismatches are visible.
    - Topology is view-only by default; write actions require explicit capability checks.
  - **Status:** To Do (deferred/optional; depends on AN-25 UniFi topology ingest)


- **DW-63: UniFi Protect events UI (motion + AI thumbnails)**
  - **Description:** Provide a dashboard surface that shows recent UniFi Protect motion/AI events and thumbnails (where available) for operator awareness.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - A dashboard view lists recent motion/AI events and last detected timestamps.
    - Thumbnails render safely without exposing credentials in URLs or logs.
    - Feature is hidden/disabled unless UniFi Protect is configured.
  - **Status:** To Do (deferred/optional; depends on AN-24 UniFi Protect ingest)



---

## Schedules and Alarms

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Done
- **SA-10: Implement rule-based conditional alarms engine + APIs (Rust core-server)**
  - **Description:** Add first-class, user-configurable conditional/threshold alarm rules so operators can define advanced alarm logic (thresholds, ranges, rolling/deviation windows, offline checks, and consecutive-period conditions) without hardcoded alarm types.
  - **Acceptance Criteria:**
    - Core-server exposes authenticated rule CRUD + preview endpoints (`/api/alarm-rules*`) with capability gating (`alerts.view` for reads, `config.write` for mutations).
    - Alarm rule schema supports composable conditions (`all`/`any`/`not`) and core condition types (`threshold`, `range`, `offline`, `rolling_window`, `deviation`, `consecutive_periods`) with server-side validation limits.
    - Runtime evaluation writes transition-aware alarm state (`triggered`/`resolved`) into `alarms` + `alarm_events` with `rule_id` and target metadata.
    - Sensor ingest path includes a fast-path evaluation trigger so matching rules react quickly after telemetry arrives.
    - Local validation passes:
      - `cargo build --manifest-path apps/core-server-rs/Cargo.toml`
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
  - **Notes / Run Log:**
    - 2026-02-10: Implemented migration `042_alarm_rules_v1.sql`, alarm engine service, alarm-rules routes, OpenAPI wiring, and alarm payload extensions. Local validation passed (build + tests).
    - 2026-02-10: Tier A validated on installed controller `0.1.9.263` (run: `project_management/runs/RUN-20260210-tier-a-sa10-sa11-alarms-0.1.9.263.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **SA-11: Build dashboard “Alarms” page with guided + advanced rule authoring**
  - **Description:** Add a dedicated dashboard Alarms page that supports both basic guided alarm creation and advanced condition composition while remaining understandable for non-expert operators.
  - **Acceptance Criteria:**
    - Dashboard route `/alarms` exists with: rule library, active alarms, rule health, and alarm history surfaces.
    - Rule create/edit flow includes a guided wizard (basics, condition, advanced timing) and supports complex conditions without requiring manual JSON edits.
    - Alarm rule CRUD/enable/disable/preview is wired to backend APIs with typed schema validation and optimistic refresh.
    - Sidebar and overview navigation include the Alarms destination and show active alarm count context.
    - UI follows `apps/dashboard-web/AGENTS.md` guardrails (shared page pattern/tokens, coherent hierarchy, no one-off component drift); any intentional exception is tracked as a `DW-*` debt ticket.
    - Local validation passes:
      - `cd apps/dashboard-web && npm run build`
      - `make ci-web-smoke`
  - **Notes / Run Log:**
    - 2026-02-10: Implemented new alarms feature module (`apps/dashboard-web/src/features/alarms/**`), page route (`apps/dashboard-web/src/app/(dashboard)/alarms/**`), API/query wiring, and nav integration. Local validation passed (build + smoke).
    - 2026-02-10: Tier A validated on installed controller `0.1.9.263` with viewed screenshots under `manual_screenshots_web/tier_a_0.1.9.263_sa12_alarms_20260210_192137/` (run: `project_management/runs/RUN-20260210-tier-a-sa10-sa11-alarms-0.1.9.263.md`).
  - **Status:** Done (validated on installed controller; clean-host E2E deferred to DT-59)

- **SA-12: Tier A validate conditional alarms on installed controller**
  - **Description:** Validate the new rule engine and dashboard Alarms workflows on the installed controller without resetting DB/settings, and capture production evidence.
  - **Acceptance Criteria:**
    - Installed controller is refreshed with the alarms-rule build and passes `farmctl health` + `/healthz`.
    - Tier A smoke verifies at least three operator alarm scenarios end-to-end (example categories: threshold/range, rolling/deviation window, consecutive-period condition).
    - Alarm transitions and history are visible in `/api/alarms`, `/api/alarms/history`, and the dashboard Alarms page.
    - At least one screenshot is captured and viewed under `manual_screenshots_web/`, with run evidence recorded under `project_management/runs/`.
    - Follow uptime discipline and rollback immediately if the refreshed build is unhealthy.
  - **Notes / Run Log:**
    - 2026-02-10: Completed Tier A validation on installed controller `0.1.9.263` including `make e2e-installed-health-smoke` (pass), `farmctl health --json` (all checks ok), three end-to-end alarm scenarios (threshold, rolling window, consecutive periods), preview checks, and viewed UI screenshots (run: `project_management/runs/RUN-20260210-tier-a-sa10-sa11-alarms-0.1.9.263.md`).
  - **Status:** Done

- **SA-13: Add incident management backend + APIs (incidents, notes, grouping, action logs)**
  - **Description:** Introduce an operator-grade incident workflow on top of alarm events: group related alarm transitions into incidents, support assign/snooze/close + notes, and expose action-log context for investigation.
  - **Acceptance Criteria:**
    - SQL migration adds `incidents`, `incident_notes`, and extends `alarm_events` with `incident_id` + `target_key` fields and indexes.
    - Alarm event emitters (alarm engine + predictive + schedule alarms) attach `rule_id`/`target_key` where available and associate the event to an open/snoozed incident using a gap-based grouping rule.
    - Core-server exposes incident APIs:
      - `GET /api/incidents` (read via `alerts.view` or `config.write`)
      - `GET /api/incidents/{id}`
      - `POST /api/incidents/{id}/assign|snooze|close` (mutations require `config.write`)
      - `GET/POST /api/incidents/{id}/notes`
      - `GET /api/action-logs` for contextual schedule/action evidence
    - Local validation passes:
      - `make ci-core-smoke`
  - **Notes / Run Log:**
    - 2026-02-11: Implemented incidents migration + grouping service, wired emitters to attach `incident_id`/`target_key`, and added incident + action-log APIs. OpenAPI export updated and TS SDK regenerated. Tests: `make ci-core-smoke` (pass).
  - **Status:** Done (local validation complete; Tier A validation tracked as SA-17)

- **SA-14: Add alarm rule stats/bands guidance endpoint + dashboard builder UX**
  - **Description:** Provide stats + visualization guidance (mean/median, ±1–3σ, robust bands, histogram) so operators can set sensible thresholds and bands without guessing.
  - **Acceptance Criteria:**
    - Core-server exposes `POST /api/alarm-rules/stats` (requires `config.write`) returning classic + robust stats and suggested bands over a configurable baseline window (default 7 days).
    - Dashboard alarm rule builder includes a Guidance step with:
      - Time-series preview + band overlays
      - Histogram + stats table
      - One-click “apply threshold/band” actions
    - Local validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
  - **Notes / Run Log:**
    - 2026-02-11: Added stats/bands guidance endpoint and a new dashboard Guidance step (time-series preview + band overlays, histogram + stats table, and one-click apply actions). Tests: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass).
  - **Status:** Done (local validation complete; Tier A validation tracked as SA-17)

- **SA-15: Add alarm rule backtest (historical replay) + builder step**
  - **Description:** Add a first-class Backtest step so operators can tune alarms against historical data, reduce false positives, and understand fire/clear behavior before saving.
  - **Acceptance Criteria:**
    - Core-server implements `alarm_rule_backtest_v1` as an analysis job and exposes it via existing `/api/analysis/jobs*` endpoints.
    - Backtest output includes summary totals, per-target breakdown, and transition timestamps for drill-down.
    - Dashboard builder includes a Backtest step that runs the job, polls status, and renders the results with drill-down.
    - Local validation passes:
      - `make ci-core-smoke`
      - `make ci-web-smoke`
  - **Notes / Run Log:**
    - 2026-02-11: Added `alarm_rule_backtest_v1` analysis job and a dashboard Backtest step (run/cancel, summary totals, per-target drill-down with transitions and firing intervals). Tests: `make ci-core-smoke` (pass), `make ci-web-smoke` (pass).
  - **Status:** Done (local validation complete; Tier A validation tracked as SA-17)

- **SA-16: Revamp dashboard `/alarms` into incident-first triage + investigation**
  - **Description:** Rebuild the `/alarms` UX so it is a polished, operator-friendly incident console with powerful investigation tools and full-scan related-sensor analysis.
  - **Acceptance Criteria:**
    - `/alarms` defaults to an Incidents view with search/filter/sort and an incident detail drawer (assign/snooze/close/notes).
    - Incident detail includes:
      - Context chart around trigger time (auto-window; user-adjustable)
      - Related signals panel using `related_sensors_unified_v2` with **controller-wide** candidate scope by default (no “visible-only” truncation) and robust filtering controls
      - “Other events” panel (action logs + node context) with search/filter
    - Rules view retains a clean rule library and launches a guided builder with Guidance + Backtest steps.
    - `/alarms2` route is untouched; sidebar label may change to “Alarms (Experimental)”.
    - Local validation passes:
      - `make ci-web-smoke`
  - **Notes / Run Log:**
    - 2026-02-11: Rebuilt `/alarms` to default to an incident console with an investigation drawer (context chart + controller-wide related signals + action-log context + notes/assign/snooze/close), and kept a Rules view with the upgraded builder. `/alarms2` untouched; sidebar label updated. Tests: `make ci-web-smoke` (pass).
  - **Status:** Done (local validation complete; Tier A validation tracked as SA-17)

- **SA-17: Tier A validate incidents + alarm builder guidance/backtest on installed controller**
  - **Description:** Validate the new incident workflow, related-signal investigation, and rule guidance/backtest on the installed controller with no DB/settings reset; capture UI evidence and pass screenshot gate.
  - **Acceptance Criteria:**
    - Installed controller refresh/upgrade is healthy (`/healthz`, `farmctl health`).
    - Operator flows verified end-to-end:
      - Incident creation/grouping from alarm event transitions (fire/resolve) and gap-based merge behavior
      - Assign/snooze/close + notes persist and render correctly
      - Related signals analysis runs controller-wide and is filterable
      - Rule builder guidance stats/histogram render and Backtest runs successfully
    - Run log includes screenshot review block and passes:
      - `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-....md`
  - **Notes / Run Log:**
    - 2026-02-11: Completed Tier A validation on installed controller `0.1.9.265-alarms-incidents` including `make e2e-installed-health-smoke` (pass) and viewed screenshots under `manual_screenshots_web/20260211_043204/` (run: `project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`).
  - **Status:** Done (validated on installed controller `0.1.9.265-alarms-incidents`; clean-host E2E deferred to DT-59)
---

## Analytics

### Blocked
- **AN-19: Validate Renogy Rover Modbus RTU polling on real hardware**
  - **Description:** Validate the Renogy Modbus RTU feed against a real Rover controller and serial adapter.
  - **Acceptance Criteria:**
    - Live polling ingests `solar_kw`, `load_kw`, `battery_kw`, and status metrics on every poll.
    - Multiple controllers on one RS-485/RS-232 bus can be polled by unit ID and surfaced in `device_ids`.
    - Poll failures surface `missing_devices` and error metadata in `/api/analytics/feeds/status`.
  - **Status:** Blocked: hardware validation (Renogy Rover + USB/RS-232/RS-485 adapter)


- **AN-29: Validate Forecast.Solar PV forecast overlay on real Renogy node hardware**
  - **Description:** Validate that Forecast.Solar predictions and Renogy measured telemetry align in the Analytics overlay on a live Pi node with a Renogy charge controller.
  - **References:**
    - `project_management/tickets/TICKET-0029-forecast.solar-public-plan-pv-forecast-integration.md`
  - **Acceptance Criteria:**
    - With PV forecast configured for a real Renogy node, the Analytics overlay shows non-empty Forecast.Solar predicted PV power and a measured PV power line from Renogy telemetry.
    - Forecast values update on refresh/poll and are persisted in the controller DB; measured telemetry continues to ingest without gaps.
    - Any unit mismatches are rejected or corrected at ingest, and the UI labels/axes are consistent.
  - **Notes / Run Log:**
    - 2026-01-06: End-to-end forecast overlay plumbing deployed and rendered, but PV parameters are still placeholders; keep this task blocked until real site PV geometry (lat/lon, tilt, azimuth, kWp) is entered and the Renogy node is online so we can validate prediction vs measured alignment.
  - **Status:** Blocked: hardware validation (real PV geometry + Renogy telemetry)


### In Progress
- [ ] No open items


### Deferred / Optional
- **AN-20: Validate Emporia ESPHome MQTT bridge on real hardware**
  - **Description:** Validate Emporia local ESPHome MQTT ingest with live hardware (optional). Cloud API ingest remains the production path for now.
  - **Acceptance Criteria:**
    - ESPHome MQTT payloads ingest successfully into analytics samples.
    - Feed status reports MQTT source, topic filter, and device/channel mapping.
    - Cloud API fallback remains available and documented.
  - **Status:** To Do (deferred/optional; hardware validation requires live Emporia ESPHome bridge)


- **AN-24: UniFi Protect ingest (motion + AI thumbnails)**
  - **Description:** Integrate UniFi Protect to ingest motion/AI detection events and surface them in the dashboard (optional), including thumbnails for recent detections where available.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - A configured UniFi Protect connection can list recent motion/AI events and expose “last detected” timestamps.
    - Thumbnails (or snapshot URLs) are retrievable and displayed in the dashboard without leaking credentials.
    - Implementation is disabled unless configured.
  - **Status:** To Do (deferred/optional; requires UniFi Protect credentials)


- **AN-25: UniFi topology ingest (network infra + Pi association by MAC/hostname)**
  - **Description:** Pull network topology/inventory from UniFi and associate Pi 5 nodes to UniFi devices using MAC/hostname (optional), enabling a “system topology” dashboard tab.
  - **References:**
    - `project_management/tickets/TICKET-0027-client-feedback-2026-01-04-(ops-ux-integrations).md`
  - **Acceptance Criteria:**
    - UniFi inventory/topology data can be fetched and cached by the controller.
    - Pi 5 nodes are associated deterministically (by MAC/hostname) and discrepancies are visible to operators.
    - Topology UI is view-only by default; mutations require explicit capability gating.
  - **Status:** To Do (deferred/optional; requires UniFi Network credentials)


---

## Architecture & Technical Debt

### Blocked
- **ARCH-6B: Tier B (clean-host) validation for ARCH-6 pruning pass**
  - **Description:** Validate the pruning pass against the installer-path clean-host E2E gate to ensure no regressions in install/upgrade/rollback/uninstall workflows.
  - **Acceptance Criteria:**
    - Tier B preflight is clean (no `com.farmdashboard.*` launchd jobs/processes or mounted artifacts).
    - `make e2e-installer-stack-smoke` passes on a clean host.
    - Tier B postflight is clean (no orphaned launchd jobs/processes); if not clean, track/fix the underlying cleanup bug.
  - **Status:** Blocked: Tier B deferred indefinitely per user instruction (no Tier B runs)

### In Progress
- [ ] No open items

### Done
- **ARCH-7: Shrink generated SDK artifacts (drop docs/tests)**
  - **Description:** Reduce repo LOC and future churn by removing generated SDK docs/tests that are not used at runtime, and enforce generator settings so they do not get reintroduced.
  - **Acceptance Criteria:**
    - `python3 tools/api-sdk/generate.py` does not generate:
      - `apps/dashboard-web/src/lib/api-client/docs/`
      - `apps/node-agent/app/generated_api/generated_api/docs/`
      - `apps/node-agent/app/generated_api/generated_api/test/`
    - Guardrails exist so generation fails if those directories are produced.
    - Validation passes:
      - `python3 tools/api-sdk/generate.py`
      - `make ci-node-smoke`
      - `make ci-web-smoke-build`
  - **Notes:**
    - 2026-02-16: Removed generated SDK docs/tests and enforced generator guardrails to keep `main` honest and small.
  - **Status:** Done

- **ARCH-6: Repo-wide pruning pass (delete legacy/redundant/unused code + artifacts)**
  - **Description:** Perform a comprehensive, folder-wide pruning pass to reduce LOC and eliminate tech debt caused by dead legacy code, redundant surfaces, and “just-in-case” fallbacks. References in docs/tickets/logs are not sacred: delete legacy items even if referenced, and update/remove the references so `main` stays honest.
  - **Acceptance Criteria:**
    - Repo root meta files that are not part of active workflows are removed (and any references updated).
    - Non-shipping app surfaces are removed (e.g., ESP32 firmware and WAN portal scaffolds) and the board/epics/docs stop implying they are active.
    - Legacy Python controller runtime/tooling ambiguity is eliminated:
      - The legacy Python `apps/core-server/` directory is removed (no tooling-only naming ambiguity remains).
      - `make migrate` and `make seed` do not depend on Python/Poetry and route through Rust-first tooling (`farmctl db ...`).
      - OpenAPI contract storage path is unambiguous and CI drift checks remain green.
    - `pm/` symlink is removed; all references use `project_management/`.
    - `project_management/runs/` and `project_management/feedback/` are curated: delete artifacts before **2026-01-01** (even if referenced) and update/remove references accordingly.
    - Validation passes:
      - `make ci-full`
      - Tier A: installed controller refresh validation passes (no DB/settings reset) with at least one captured and viewed screenshot under `manual_screenshots_web/` and a run log under `project_management/runs/`.
    - Tier B clean-host E2E (`make e2e-installer-stack-smoke`) is deferred indefinitely per user instruction (tracked as ARCH-6B).
  - **Notes / Run Log:**
    - 2026-02-12: Tier‑A run log + screenshot hard gate: `project_management/runs/RUN-20260212-tier-a-arch6-prune-0.1.9.268-arch6-prune.md`.
  - **Status:** Done

- **ARCH-2: Main-branch integrity pass for dead/fallback/stub code**
  - **Description:** Remove legacy fallback/dead/stub codepaths on `main` that can be mistaken for real production features, and enforce fail-closed behavior where implementation is incomplete.
  - **Acceptance Criteria:**
    - Deceptive fallback/stub paths touched in this pass are removed or fail closed with explicit operator-facing errors.
    - Removed/deferred behaviors are reflected in `project_management` trackers so `main` scope remains honest.
    - `make ci-farmctl-smoke` and `make ci-web-smoke` pass after the cleanup.
  - **Notes / Run Log:**
    - 2026-02-06: Removed `farmctl bundle --stub` mode and stub artifact/native-deps writers so controller bundle builds now fail closed when native deps are missing.
    - 2026-02-06: Removed mobile-only SDK/release/test fallback paths on `main` (`swift5` SDK target, `ios` release target validation, optional iOS smoke include in installer-stack smoke).
    - 2026-02-06: Removed `/api/dashboard/demo` from Rust routes/OpenAPI and removed dashboard-web snapshot 404 fallback reconstruction so dashboard snapshot now fails closed on backend contract errors.
    - 2026-02-06: Validation passed:
      - `make ci-core-smoke`
      - `make ci-farmctl-smoke`
      - `make ci-web-smoke-build`
  - **Status:** Done

- **ARCH-3: Refactor farmctl bundle “god file” into scoped modules (phase 1)**
  - **Description:** Reduce mixed-scope complexity in `apps/farmctl/src/bundle.rs` by extracting node-overlay packaging into a dedicated module while preserving behavior.
  - **Acceptance Criteria:**
    - Node overlay packaging logic lives in `apps/farmctl/src/bundle_node_overlay.rs`.
    - `apps/farmctl/src/bundle.rs` no longer contains overlay tar/systemd/script copy helpers.
    - `cargo test --manifest-path apps/farmctl/Cargo.toml` passes.
  - **Notes / Run Log:**
    - 2026-02-06: Added `bundle_node_overlay.rs` and moved node overlay build/copy/export/tar logic out of `bundle.rs`; wired module through `main.rs`.
    - 2026-02-06: Validation passed:
      - `cargo test --manifest-path apps/farmctl/Cargo.toml`
      - `make ci-farmctl-smoke`
  - **Status:** Done

- **ARCH-4: Stub/dead-code audit and follow-up ticketing across active surfaces**
  - **Description:** Audit core-server-rs, dashboard-web, farmctl, and node-agent for remaining stubs/dead branches that can appear as active features; either remove them or track explicit implementation tickets.
  - **Acceptance Criteria:**
    - Audit output lists each stub/dead branch, owning component, and disposition (`remove`, `implement`, or `defer`).
    - Any retained stub has an explicit fail-closed UX/API behavior and a linked task.
    - New follow-up tickets are added to `project_management/TASKS.md` with owners and measurable exit criteria.
  - **Notes / Run Log:**
    - 2026-02-06: Completed cross-surface audit and disposition:
      - `apps/core-server-rs/src/routes/dashboard.rs` (`/api/dashboard/demo`) → **remove** (done).
      - `apps/dashboard-web/src/lib/api.ts` 404 snapshot reconstruction fallback → **remove** (done; fail closed to `/api/dashboard/state` contract).
      - `apps/farmctl/src/config.rs` DMG path fallback + `apps/farmctl/src/server.rs` static asset fallback → **retain** (non-deceptive resilience; no fake success).
      - `apps/core-server-rs/src/routes/backups*.rs` retention/default path fallback helpers → **retain** (non-deceptive resilience).
      - `apps/node-agent/app/hardware/gpio.py` pulse-counter stub mode when `pigpio` unavailable → **follow-up task** (`ARCH-5`).
    - 2026-02-06: Added CI guardrail `tools/production_token_guardrail.py` + baseline allowlist `tools/guardrails/production_token_allowlist.json`; wired to CI (`.github/workflows/ci.yml`) and local target (`make ci-integrity-guardrail`).
    - 2026-02-06: Wired the staged-path pre-commit selector (`tools/git-hooks/select-tests.py`) to run `make ci-integrity-guardrail` for touched production surfaces.
    - 2026-02-06: Validation passed:
      - `make ci-integrity-guardrail`
      - `python3 -m py_compile tools/production_token_guardrail.py`
      - Installed-controller health smoke/run log: `project_management/runs/RUN-20260206-installed-controller-smoke-main-integrity-cleanup.md`
  - **Status:** Done

### To Do
- **ARCH-5: Node-agent GPIO pulse counter must fail closed in production when pigpio is unavailable**
  - **Description:** Remove the production “stub mode” behavior for pulse counters in `apps/node-agent/app/hardware/gpio.py` so missing `pigpio` cannot silently simulate pulse data on production nodes.
  - **Owner:** Node Agent
  - **Acceptance Criteria:**
    - Production builds (`BUILD_FLAVOR=prod`) do not emit simulated pulse counts when `pigpio` is unavailable.
    - Node health reports an explicit pulse backend error state and no fake pulse telemetry is published.
    - Non-production test/dev paths can still use deterministic test doubles under explicit test-only gates.
    - Validation includes `make ci-node-smoke` and `make ci-core-smoke` (for end-to-end heartbeat/status compatibility).
  - **Status:** To Do


---

## Time-Series Similarity Engine (TSSE)

**Hard requirement:** Every pending/incomplete TSSE ticket/task must be implemented by a single agent. The Collab Harness multi-agent workflow is no longer required for remaining TSSE work.

### Blocked
- [ ] No open items


### Status Rollup (audited 2026-02-07)
- **Done:** TSSE-1, TSSE-2, TSSE-3, TSSE-4, TSSE-5, TSSE-6, TSSE-7, TSSE-8, TSSE-9, TSSE-10, TSSE-11, TSSE-12, TSSE-13, TSSE-14, TSSE-15, TSSE-16, TSSE-17, TSSE-18, TSSE-19, TSSE-20, TSSE-21, TSSE-22, TSSE-23, TSSE-24, TSSE-25, TSSE-36, TSSE-37
- **In Progress:** (none)
- **To Do:** (none)


### Tasks (detailed)
- **TSSE-37: Related sensors scoring: penalize diurnal lag artifacts + bound score by lag correlation**
  - **Description:** Prevent weak/diurnal correlations from scoring near 1.0 in `related_sensors_v1` by penalizing near-24h lag artifacts, gating multi-episode bonuses, and blending the final score with the global lag |r| signal so low-|r| matches cannot dominate ranking.
  - **Acceptance Criteria:**
    - Candidates with best lag near an integer multiple of 24h add `diurnal_lag` to `why_ranked.penalties` and have their final score reduced (down-ranked by default, not hard-excluded).
    - `multi_episode` bonus is not applied to diurnal-lag candidates.
    - Final score is bounded by the global lag |r| so extremely low `lag_score` cannot produce near-1.0 results.
    - `why_ranked.score_components` includes `lag_r_abs` for UI/back-compat and `lag_signal_factor` for explainability.
    - Tests cover the new diurnal penalty + bonus gating behavior.
    - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
    - Tier A validated on installed controller (no DB/settings reset); Tier B deferred to DT-59.
  - **Owner:** Core Analytics (Codex)
  - **Notes / Run Log:**
    - 2026-02-07: Implemented diurnal-lag penalty + base-signal bounding for `related_sensors_v1` scoring:
      - `apps/core-server-rs/src/services/analysis/tsse/scoring.rs` adds `diurnal_lag` penalty, gates `multi_episode` bonus for diurnal, and applies a `lag_signal_factor` multiplier so low-|r| cannot score near 1.0.
      - Adds explainability keys: `lag_r_abs`, `lag_r_signed`, `is_diurnal_lag`, `diurnal_penalty_multiplier`, `lag_signal_factor`.
      - Adds unit tests covering diurnal detection + bonus gating.
    - 2026-02-07: Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
    - 2026-02-07: Tier A validated on installed controller `0.1.9.255-related-diurnal-penalty` (no DB/settings reset):
      - Installed smoke: `make e2e-installed-health-smoke` (PASS)
      - UI evidence captured + viewed: `manual_screenshots_web/20260207_000801/trends_related_sensors_scanning.png`
      - Tier A ticket: `project_management/tickets/TICKET-0050-tier-a:-tsse-37-+-dw-225-related-sensors-diurnal-penalty-+-preview-defaults-(0.1.9.255).md`
  - **Status:** Done (validated on installed controller `0.1.9.255-related-diurnal-penalty`; clean-host E2E deferred to DT-59)

- **TSSE-36: TSSE stats Phase 3/4/5 — n_eff, lag correction, BH-FDR (matrix + related sensors)**
  - **Description:** Complete Phase 3/4/5 of `project_management/archive/trackers/TSSE_TSE_STATISTICAL_CORRECTNESS_TRACKER.md` so correlation “significance” is time-series aware, corrected for lag selection and multiple comparisons, and the dashboard reflects the semantics (`p` vs `q`, `n` vs `n_eff`).
  - **Acceptance Criteria:**
    - Correlation matrix:
      - Cells expose `n_eff` and `q_value` and drive `status` from BH-FDR `q_value <= alpha`.
      - Spearman significance uses t-approx p-values (no Pearson/Fisher-z p-value reuse).
    - Related sensors:
      - Lag selection bias corrected via `p_lag` (Sidak-style) using real `m_lag`.
      - Candidate-set BH-FDR `q_value` computed across candidates using `p_lag`.
      - Returned candidates are filtered by `q_value <= alpha` (not raw p-value).
    - Dashboard:
      - Trends correlation tooltip shows `p` and `q` and surfaces `n_eff` where present.
      - Related sensors preview shows `p_raw`, `p_lag`, `q`, `n_eff`, and `m_lag`.
    - Tests:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.
      - `make ci-web-smoke` passes.
    - Tier A:
      - Installed controller refreshed via runbook with **no DB/settings reset**.
      - At least one screenshot captured + viewed under `manual_screenshots_web/` showing Trends correlation/related sensors UX with the new p/q labels.
  - **Notes / Run Log:**
    - Tracker: `project_management/archive/trackers/TSSE_TSE_STATISTICAL_CORRECTNESS_TRACKER.md`
    - 2026-02-06: Implemented additional statistical correctness hardening and TSSE UI polish:
      - Added effect-size floors (`min_abs_r`) to `correlation_matrix_v1` and `related_sensors_v1` significance gating (`q <= alpha && |r| >= min_abs_r`).
      - Added explicit bucket aggregation controls (`bucket_aggregation_mode`: `auto|avg|last|sum|min|max`) to correlation and related-sensors job params; `auto` now resolves per-sensor aggregation mode by sensor type in the bucket reader.
      - Correlation matrix now returns computed `r` for non-significant cells (status carries significance semantics); tooltips now show `p`, `q`, `n`, `n_eff`, method, and status.
      - Related Sensors preview now surfaces `p_raw`, `p_lag`, `q`, `n`, `n_eff`, and `m_lag`; Trends control panel exposes aggregation mode and min `|r|` controls.
      - Removed dead TSSE placeholder modules (`apps/core-server-rs/src/services/analysis/tsse/preview.rs`, `apps/core-server-rs/src/services/analysis/tsse/qdrant_client.rs`) and dropped their module exports.
    - 2026-02-06: Local validation passed:
      - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
      - `make ci-web-smoke`
      - `cargo build --manifest-path apps/core-server-rs/Cargo.toml`
      - `cd apps/dashboard-web && npm run build`
    - 2026-02-06: Tier A validated on installed controller (no DB/settings reset):
      - Refresh run: `project_management/runs/RUN-20260206-tier-a-tsse36-0.1.9.250-tsse36-ui-polish.md`
      - Installed version updated `0.1.9.249-derived-builder-guardrails` -> `0.1.9.250-tsse36-ui-polish`
      - Installed health smoke: `make e2e-installed-health-smoke` (PASS)
      - Screenshot captured + viewed: `manual_screenshots_web/tier_a_0.1.9.250-tsse36-ui-polish_20260206c/tsse_relationship_panel_correlation_stats_key.png`
    - 2026-02-06: Follow-up process hardening completed during closeout:
      - `tools/rebuild_refresh_installed_controller.py` now handles wrapped status payloads, resilient version inference, async upgrade completion polling, optional artifact pruning, and repeat-run speed controls (`--reuse-existing-bundle`, `--farmctl-skip-build`).
      - `apps/dashboard-web/scripts/web-screenshots.mjs` now targets current Trends `CollapsibleCard` UI and supports panel-scoped captures with resilient fallback.
      - One-time external artifact cleanup completed: `/Users/Shared/FarmDashboardBuilds` reduced from `25G` to `7.0G` (kept newest 20 controller DMGs; pruned 59 old DMGs + 7 old logs), plus removal of stale sibling artifact dirs/files (`/Users/Shared/FarmDashboardBuilds_TierA`, `/Users/Shared/FarmDashboardBuildsDirty`, orphan root-level controller DMGs).
      - Post-cleanup revalidation passed:
        - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
        - `make ci-web-smoke`
  - **Status:** Done

---

## Documentation

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Done
- **DOC-39: Add execution stop-gate policy to agent docs**
  - **Description:** Add concise, persistent guardrails so implementation turns do not end early without explicit blocker/de-scope.
  - **Acceptance Criteria:**
    - `AGENTS.md` defines a hard stop gate requiring: completed requested scope (or explicit blocker), validation run, and synchronized `project_management` trackers before final response.
    - `AGENTS.md` requires explicit user-approved de-scope when splitting large/risky implementation work.
  - **Status:** Done
---

## Setup App & Native Services

**North-Star Outcome:**
- A non-expert can take a brand-new Mac mini, run a single installer, answer a few prompts, and end up with a running system plus a health UI and guided node onboarding.
- No manual edits, no copy/paste commands, and upgrades/backups are one-click (or one command).

**North-Star Acceptance Gate (Setup):**
- Single installer DMG auto-launches the wizard (no Terminal steps).
- Minimal prompts; bundle/farmctl auto-detect; advanced fields hidden by default.
- Services start at boot with no user logged in (LaunchDaemons/system launchd domain).
- Services run as a least-privilege service user (not root); only install/bootstrap requires admin.
- DB + MQTT + Redis are bundled/provisioned without manual dependency steps.
- Controller bundles are local-path DMGs (no remote bundle downloads).
- Setup Center health/install/upgrade/backup works without manually starting a setup service.
- `make e2e-installer-stack-smoke` validates DMG install/upgrade/rollback/uninstall in a clean temp root.
- Clean uninstall/reset is supported so repeated installs are safe.
- E2E runs start and end from a verified clean state (no orphaned launchd jobs or background processes before/after).
- E2E profile leaves no persistent launchd enable/disable override keys for its label prefix (no state pollution).

**Elegant Solution Architecture (3 layers):**
1) **Distribution and Versioning (Productized Delivery)**
   - Ship a versioned controller bundle (images plus configs) so install does not require building from source.
   - Provide a single entrypoint tool (`farmctl`) that knows the release manifest and can install/upgrade/rollback.
   - Goal: "Install vX.Y.Z" becomes a deterministic, one-step action.
2) **Install and Configure (Guided and Idempotent)**
   - A setup assistant (CLI or small local web wizard) that checks prerequisites, asks for minimal inputs, writes config, generates secrets, starts services, and verifies health with clear green checks.
   - Everything is idempotent (re-running is safe).
3) **Operations and Onboarding (UI-First)**
   - A System Setup area in the dashboard for infra status, backups, updates, and guided node adoption.
   - Central place for credentials (Emporia tokens, etc.) and a one-click diagnostics export for support.

**Implementation Phases (Elegant, Not Band-Aid):**
- Phase 1: Productized release plus install tool (build/publish bundles, `farmctl install` uses release images/configs, no source build required).
- Phase 2: Guided setup assistant (interactive prompts to config/start/verify with optional local web UI).
- Phase 3: Operations and onboarding UI (Setup Center in dashboard for health, updates, backups, and node adoption).

### Blocked
- **SETUP-40: Validate installer/setup cluster on clean host (Tier B)**
  - **Description:** Run Tier‑B validation on a clean host to confirm the installer + setup flow is production-correct (quarantine-safe DMG, bootstrap admin, install/upgrade/rollback, and clean uninstall/reset) without relying on the already-installed production controller state.
  - **Acceptance Criteria:**
    - Clean-state preflight/postflight checks pass on the clean host.
    - `make e2e-setup-smoke` and `make e2e-setup-smoke-quarantine` pass.
    - Evidence is recorded (commands + artifact path under `project_management/runs/`).
  - **Status:** Blocked: clean-host E2E validation (prod host cannot be stopped/reset)


### In Progress
- **SETUP-33: Fix installer DMG Gatekeeper/quarantine “app is corrupted” on fresh downloads**
  - **Description:** The production installer DMG must be installable when downloaded/quarantined (no manual `xattr` steps). Current `FarmDashboardInstaller-0.1.8.dmg` is rejected by Gatekeeper and surfaces “app is corrupted/damaged”.
  - **Acceptance Criteria:**
    - A quarantined-downloaded `FarmDashboardInstaller-<ver>.dmg` opens via Finder without manual `xattr` removal.
    - The embedded `Farm Dashboard Installer.app` passes `spctl -a -vv --type execute` on a fresh macOS host.
    - The DMG install path remains E2E-validated (`make e2e-setup-smoke-quarantine`).
  - **Notes / Field Failures:**
    - 2026-01-04: Wizard `POST /api/install` fails with `AuthorizationExecuteWithPrivileges failed (status=-60011)` (no admin prompt; install button appears no-op).
    - 2026-01-04: Root cause fixed in `apps/farmctl/src/privileged.rs` (use `0` options for `AuthorizationExecuteWithPrivileges`); rebuilt local installer DMG for re-test.
    - 2026-01-05: Even with flags fixed, the production setup-daemon runs as a LaunchDaemon (`_farmdashboard`) and cannot present GUI auth prompts; `/api/install` now fails with `AuthorizationExecuteWithPrivileges failed (status=-60007)` and no prompt.
  - **Status:** In Progress


- **SETUP-34: Unblock headless Setup Center install/upgrade in production**
  - **Description:** The setup-daemon runs as `_farmdashboard` in the system launchd domain, so it cannot rely on `AuthorizationExecuteWithPrivileges` prompts for install/upgrade/rollback. Unblock production Setup Center actions by avoiding GUI auth prompts when the service user can perform the operation (and reserving admin prompts for the initial LaunchDaemon bootstrap path via the installer app).
  - **Acceptance Criteria:**
    - In production LaunchDaemon mode, `POST /api/install` and `POST /api/upgrade` perform the operation end-to-end (no 500/no-op) without requiring an interactive auth dialog.
    - Upgrade/rollback restarts services safely (terminate service-user-owned processes; rely on launchd KeepAlive), and leaves no orphaned processes.
    - `make e2e-setup-smoke` remains green and a real production Mac mini validation run is logged.
  - **Run Log:**
    - 2026-01-05: Implemented `apps/farmctl/src/server.rs` guard: when already running as `config.service_user`, `install|upgrade|rollback` run directly (no auth prompt).
    - 2026-01-05: Built controller bundle `FarmDashboardController-0.1.8.3.dmg` with updated `core-server` + `telemetry-sidecar`; kicked off a prod upgrade attempt via a user-session `farmctl serve` on `127.0.0.1:8801` (awaiting admin approval in SecurityAgent to proceed).
    - 2026-01-05: Bundle build regression: `farmctl bundle --native-deps /usr/local/farm-dashboard/native` failed because the `native` path is a symlink root; fixed by canonicalizing `--native-deps` before copy in `apps/farmctl/src/bundle.rs`.
    - 2026-01-05: Bundle build regression: `farmctl bundle` failed due to `npm` cache permissions (`EACCES` in `~/.npm/_cacache`); fixed by setting `npm_config_cache` to a temp directory in `apps/farmctl/src/bundle.rs`. Built `build/FarmDashboardController-0.1.8.4.dmg` and `build/FarmDashboardInstaller-0.1.8.4.dmg` successfully.
    - 2026-01-05: Production validation: `POST /api/upgrade` succeeds without an auth prompt when the setup daemon is running as `_farmdashboard` and the controller bundle is referenced by a stable path (not a transient `/Volumes/...` mount). Remaining paper-cut: farmctl cannot clear quarantine `xattr` on bundles it does not own; follow-up is to cache bundles under a `_farmdashboard`-owned directory or copy them to a service-owned cache path before xattr/mount.
    - 2026-01-05: Production upgrade executed via setup-daemon `POST /api/upgrade` (port `8800`) with `bundle_path=/Users/Shared/FarmDashboardController-0.1.8.4.dmg`; `farmctl upgrade` returned `Upgraded to 0.1.8.4`, `core-server`/`telemetry-sidecar` restarted under `_farmdashboard` via launchd `KeepAlive`. Non-fatal stderr persists: `xattr: [Errno 13] Permission denied` on the bundle path (bundle owned by login user).
    - 2026-01-09: Ported `farmctl` upgrade hardening: remove existing `farmctl` before copy, best-effort config saves during upgrade/rollback/status/health/diagnostics when `config.json` is service-owned, and clearer privileged auth hints for headless LaunchDaemon contexts.
  - **Status:** In Progress


- **SETUP-38: Expose controller config + preflight in Setup Center**
  - **Description:** Add a first-class admin panel in the dashboard Setup Center to view/edit the setup-daemon config (ports, MQTT host, backups, bundle DMG path, etc.) and surface setup-daemon preflight checks so an operator can diagnose common install/runtime issues without SSH.
  - **Acceptance Criteria:**
    - Setup Center includes a “Controller configuration” panel that loads and saves setup-daemon config via the `/api/setup-daemon/*` proxy using the dashboard auth token.
    - The panel includes descriptive labels, inline validation for ports/required fields, and an “Advanced settings” toggle for rarely-touched paths/users/binaries.
    - Setup Center shows setup-daemon preflight checks (`GET /api/setup-daemon/preflight`) with clear OK/Warn/Error badges.
    - Setup Center includes an MQTT host helper (“Use this Mac’s IP”) wired to `GET /api/setup-daemon/local-ip`.
    - Installer actions (Install/Upgrade/Rollback/Diagnostics) work via the auth-gated `/api/setup-daemon/*` proxy (no raw unauthenticated fetches).
    - Deployed controller bundle includes the new Setup Center panel; verification log includes the bundle version and `/healthz` check.
  - **Run Log:**
    - 2026-01-07: Added Setup Center “Controller configuration” panel (setup-daemon config editor + preflight viewer) and fixed installer actions to use authenticated JSON requests.
    - 2026-01-07: Hardened `/api/setup-daemon/*` proxy to require `config.write` for reads + writes; secured `POST /api/analytics/feeds/poll` behind `config.write`.
    - 2026-01-07: Built controller bundle `0.1.9.17` (output: `/Users/Shared/FarmDashboardController-0.1.9.14.dmg`) and upgraded via `POST http://127.0.0.1:8800/api/upgrade` (result: `Upgraded to 0.1.9.17`, `/healthz` 200).
    - 2026-01-07: Refreshed installed stack to controller bundle `0.1.9.18` via `POST http://127.0.0.1:8800/api/upgrade` (result: `Upgraded to 0.1.9.18`, `/healthz` 200).
    - 2026-01-07: Expanded setup-daemon config schema (MQTT username/password; core polling toggles + intervals) and exposed these as Advanced options in Setup Center; bundled + upgraded installed controller to `0.1.9.21`.
    - 2026-01-07: Removed the “setup-daemon schema mismatch” UX gap by adding a controller-owned config API: Rust core-server now serves `/api/setup/controller/runtime-config` (reads/writes `/Users/Shared/FarmDashboard/setup/config.json` for MQTT auth + polling toggles/intervals) and core-server/telemetry-sidecar now load these overrides from the config file at startup.
    - 2026-01-07: Updated Setup Center to load/save “Advanced controller settings” via the new core-server endpoint (no setup-daemon restart required for these fields).
    - 2026-01-07: Built controller bundle `0.1.9.23` (`/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.23.dmg`) and upgraded the installed controller via setup-daemon (`POST http://127.0.0.1:8800/api/upgrade`); `GET http://127.0.0.1:8000/healthz` is ok and `/api/openapi.json` includes `/api/setup/controller/runtime-config`. Follow-up fix: tightened config.json override behavior so core-server/sidecar do not unexpectedly apply port/DB changes that also require reconfiguring other launchd services (mosquitto/postgres) when running in “restart-only” prod upgrades.
    - 2026-01-07: Built controller bundle `0.1.9.24` (`/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.24.dmg`) and upgraded via setup-daemon; runtime-config now includes telemetry offline threshold and Setup Center preserves runtime-config values even when saving only setup-daemon fields.
    - 2026-01-07: Extended runtime-config + Setup Center Advanced settings with telemetry-sidecar tuning (MQTT topic prefix/keepalive + enable listener + batch/flush/queue/status poll); added Setup Center Mapillary token + admin-only token fetch endpoint for Map tab street view; bundled + upgraded installed controller to `0.1.9.26` (`/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.26.dmg`); `/healthz` 200.
    - Test note: E2E not run on this host; clean-state test hygiene gate requires no running Farm launchd jobs/processes before/after the run.
  - **Status:** In Progress


### To Do
- **SETUP-36: Validate bootstrap admin + session-login UX (prod)**
  - **Description:** Validate the production UX for “login-first” + bootstrap admin credentials using the real installer DMG on a clean Mac (or a clean manual reset).
  - **Acceptance Criteria:**
    - After install, **the dashboard lands on `/login`** and requires sign-in at the start of each browser session.
    - The installer logs (Operations → Install) show the bootstrap admin email + temporary password.
    - Signing in with the bootstrap credentials succeeds and the dashboard can reach auth-gated pages without manual token hacks.
    - Admin can change the password from the dashboard (**Users** → **Set password**) and re-login succeeds with the new password.
  - **Status:** To Do


- **SETUP-37: Verify uninstall cleanup repeatability (no orphans/ports)**
  - **Description:** Verify and harden the uninstall/reset cleanup path so repeated installer E2E runs (and operator uninstalls) always leave a clean machine state.
  - **References:**
    - `project_management/tickets/TICKET-0025-verify-and-harden-uninstall-process-cleanup.md`
  - **Acceptance Criteria:**
    - Run `make e2e-installer-stack-smoke` 5 times consecutively.
    - After each run, `python3 tools/test_hygiene.py` reports a clean state.
    - Manual `farmctl uninstall --remove-roots --yes` leaves no launchd jobs/processes/ports behind.
  - **Status:** To Do

- **SETUP-41: Infrastructure Dashboard rebrand (installer + bundle naming + UX copy)**
  - **Description:** Update installer bundles, setup UI copy, and controller artifacts to use the “Infrastructure Dashboard” product name.
  - **References:**
    - `project_management/tickets/TICKET-0077-infrastructure-dashboard-commercial-integrations.md`
  - **Acceptance Criteria:**
    - Setup app and DMG names use Infrastructure Dashboard branding.
    - UI copy and README references are updated to the new product name.
    - Bundled controller DMG naming follows the Infrastructure Dashboard convention.
    - `make e2e-setup-smoke` passes with the new naming.
  - **Status:** To Do
