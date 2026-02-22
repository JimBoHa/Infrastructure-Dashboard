# Infrastructure Dashboard Project Board

> **Note:** Active tickets live in `project_management/TASKS.md`; completed tickets live in `project_management/TASKS_DONE_2026.md`; indefinitely deferred/deprecated items live in `project_management/TASKS_DEFERRED_INDEFINITE.md`.
> This board rolls up epic status from those tickets; epic definitions live in `project_management/EPICS.md`.
> Detailed requirement tickets live in `project_management/tickets/` and should be linked from `project_management/TASKS.md`.
> It supersedes all previous trackers and plans.

This project board provides a high-level overview of the work to be done on the Infrastructure Dashboard project.

**How to update this board:**
- Update `project_management/TASKS.md` first; reflect major milestone/status changes here.
- Epic status should reflect the highest-severity implementation ticket (`Blocked` > `In Progress` > `To Do` > `Done`); Tier‑B clean-host E2E is tracked separately via validation cluster tickets.
- If a feature is waiting on hardware, track it as a separate **Validate … on hardware** ticket (`Blocked: hardware validation (...)`) rather than leaving implementation work as `In Progress`.
- For dashboard-web UI work, enforce the UI/UX guardrails in `apps/dashboard-web/AGENTS.md` and track deliberate UI debt as `DW-*` tickets (owner + exit criteria).

## Epics

| Epic | Status |
| --- | --- |
| [Core Infrastructure](#core-infrastructure) | Blocked: clean-host E2E validation (DT-59) |
| [Core Server](#core-server) | In Progress (WS-2902 by-node endpoints; Pi 5 SPI bootstrap; external device integration CS-109; DHCP churn validation CS-105) |
| [Rust Core Server Migration](#rust-core-server-migration) | To Do (RCS-16..RCS-20 follow-ups: tracing + outputs refactor + no-Python guardrails) |
| [Telemetry Ingest Sidecar](#telemetry-ingest-sidecar) | In Progress (TS-9 node-health split + core parity; Tier A pending) |
| [Offline Telemetry Spool + Backfill Replay](#offline-telemetry-spool-backfill-replay) | Done (Tier A + hardware validated; Tier B OT-13) |
| [Standalone Rust Telemetry + Predictive](#standalone-rust-telemetry-predictive) | To Do (deferred/optional; not required for production while predictive is disabled by default) |
| [Node Agent](#node-agent) | Blocked: hardware validation (generic stack perf + mesh + BLE + Renogy + pulse + ADS1263) |
| [Discovery and Adoption](#discovery-and-adoption) | Done (Tier A validated; Tier B DT-59) |
| [Dashboard Web](#dashboard-web) | In Progress (DW-193 Playwright desktop project + DW-208 soil moisture default open + DW-209 analytics overflow + DW-259 external device management UI; Tier‑B clusters DW-97/DW-98/DW-114/CS-69) |
| [Schedules and Alarms](#schedules-and-alarms) | Done (Tier A validated installed `0.1.9.265-alarms-incidents`; Tier B DT-59) |
| [Backups and Restore](#backups-and-restore) | Done (Tier A validated; Tier B DW-99) |
| [Analytics](#analytics) | Blocked: hardware/credential validation (Renogy Modbus + external feeds); forecasts shipped (Forecast.Solar + Open-Meteo) with cloud-cover/current-weather follow-ups |
| [Time-Series Similarity Engine (TSSE)](#tsse) | Done (TSSE-37 Tier‑A validated on installed `0.1.9.255-related-diurnal-penalty`; Tier B DT-59) |
| [iOS App](#ios-app) | Deferred indefinitely (moved off `main`; freeze/ios-watch-2026q1) |
| [Documentation](#documentation) | Done (DOC-39 execution stop gate + DOC-37/38 runbook discipline) |
| [Setup App & Native Services](#setup-app-native-services) | In Progress (SETUP-33/SETUP-34/SETUP-38 setup UX + admin config) |
| [Architecture & Technical Debt](#architecture-technical-debt) | In Progress (ARCH-5 pulse fail-closed follow-up; ARCH-6B Tier B validation deferred) |

---

## Tier B Validation (Clean Host)

> Implementation may be marked **Done under Tier A** (“validated on installed controller; no DB/settings reset”) while clean-host E2E correctness is tracked here.

- [ ] DT-58 Clean-host E2E runner + runbook (Tier B)
- ⏸ DT-59 Validate core correctness cluster on clean host (Tier B)
- ⏸ OT-13 Validate offline buffering cluster on clean host (Tier B)
- ⏸ SETUP-40 Validate installer/setup cluster on clean host (Tier B)
- ⏸ DW-97 Validate Map cluster on clean host (Tier B)
- ⏸ DW-98 Validate Trends/COV/CSV cluster on clean host (Tier B)
- ⏸ DW-114 Validate dashboard layout/IA cluster on clean host (Tier B)
- ⏸ DW-212 Validate Analytics Temp Compensation on clean host (Tier B)
- ⏸ DW-99 Validate Backups/Exports cluster on clean host (Tier B)
- ⏸ CS-69 Validate Power/Analytics composition cluster on clean host (Tier B)

---

## <a name="core-infrastructure"></a>Core Infrastructure

### Blocked
- ⏸ DT-59 Validate core correctness cluster on clean host (Tier B)

### In Progress
- [ ] No open items

### To Do
- [ ] DT-58 Clean-host E2E runner + runbook (Tier B)

### Deferred / Optional
- [ ] DT-57 Pi 5 network boot “zero-touch” provisioning (spec gap)
- _Deprecated / indefinitely deferred items live in `project_management/TASKS_DEFERRED_INDEFINITE.md` (e.g., DT-47/DT-48/DT-49)._

### Done
- ✅ DT-74 Tier-A screenshot review hard gate (`make tier-a-screenshot-gate`) + required run-log block in Tier-A runbook/templates
- ✅ DT-73 Defer iOS/watch from `main` and prune mobile-only build/test hooks (mobile surfaces removed from `main`; preservation branch pushed: `freeze/ios-watch-2026q1`; validated with `make ci-core-smoke`, `make ci-farmctl-smoke`, `make ci-web-smoke-build`, `make e2e-installed-health-smoke`; run log: `project_management/runs/RUN-20260206-installed-controller-smoke-main-integrity-cleanup.md`)
- ✅ TSSE-36 Statistical correctness + TSSE UX polish (Tier‑A validated installed `0.1.9.250-tsse36-ui-polish`; run: `project_management/runs/RUN-20260206-tier-a-tsse36-0.1.9.250-tsse36-ui-polish.md`)
- ✅ DT-66 Define fastest dashboard-web validation loop (smoke + build commands + runtime)
- ✅ DT-67 Codify dashboard-web validation loop (make target + pre-commit)
- ✅ DT-69 Document full test suite + TSSE validation commands + reports artifact guidance
- ✅ DT-70 Codify installed-controller uptime discipline (upgrade only validated builds; rollback on failure)
- ✅ DT-60 ADS1263 Phase 0: split “big diff” into phase commits + gates
- ✅ DT-61 ADS1263 Phase 1: build flavor + fail-closed analog (“no simulation in production”)
- ✅ DT-62 ADS1263 Phase 2: remove “ADS1115” as a concept (analog=ADS1263 only)
- ✅ DT-63 ADS1263 Phase 3: Pi5 ADS1263 backend + health (gpiozero + spidev; deterministic health; fail-closed)
- ✅ DT-64 ADS1263 Phase 4: “Add hardware sensor” from dashboard (Pi-only) end-to-end
- ✅ DT-65 ADS1263 Phase 5: Reservoir depth transducer (AIN0 vs AINCOM + 163Ω shunt)
- ✅ DT-56 Require clean-state pre/postflight checks for test runs
- ✅ DT-55 Remove hardcoded iOS smoke-test password (audit)
- ✅ DT-54 Remove panic-on-startup in WAN portal state init (audit; WAN portal removed in ARCH-6)
- ✅ DT-53 Fix `farmctl native-deps` relative output path installs
- ✅ DT-52 Deprecate dashboard-web manifest stub (static dashboard is served by core-server)
- ✅ DT-51 Consolidate Sim Lab tooling paths under `tools/sim_lab/`
- ✅ DT-45 WAN read-only portal scaffolding (AWS template + pull agent skeleton; scaffold removed in ARCH-6)
- ✅ DT-50 Remove obsolete dashboard service config fields from the setup wizard
- ✅ DT-43 Productize “preconfigured media” deployment option (Pi 5)
- ✅ DT-44 Prototype Pi 5 network-boot provisioning workflow
- ✅ DT-38 Remove container-stack dependency from Sim Lab E2E harness
- ✅ DT-46 Temporarily disable iOS/watch smoke in the pre-commit selector
- ✅ DT-40 Remove container runtime from the repo and CI
- ✅ DT-39 Refactor farmctl monolith into modules
- ✅ DT-42 Add fast installer-path smoke checks + better E2E logs
- ✅ Setup native dependencies with Postgres/Timescale, Mosquitto, Redis, and Grafana
- ✅ Create initial database schema with migrations
- ✅ Implement Makefile for managing the infrastructure
- ✅ Implement a seed script for populating demo data
- ✅ Path-aware commit hook selector (DT-24)
- ✅ Offline boot config generator (DT-2)
- ✅ Dashboard-web Vitest JSDOM shims for charts/downloads (DT-3)
- ✅ Sim Lab runner + `make demo-live` target (DT-4)
- ✅ Sim Lab control API service (DT-23)
- ✅ Simulated outputs + repeatable fault scenarios (DT-6)
- ✅ Demo-live rerunnable migrations + default web port 3001 (DT-7)
- ✅ Remove guardrail reminders from test output (DT-8)
- ✅ CI path gating + doc-only fast path (DT-9)
- ✅ Split iOS simulator workflow + manual dispatch (DT-10)
- ✅ Smoke vs full test tiers (DT-11)
- ✅ CI caching/concurrency optimization (DT-12)
- ✅ Sim Lab deterministic mesh/BLE/feed simulation (DT-13)
- ✅ Sim Lab adoption workflow (DT-5; UUID candidates + discovery token filtering)
- ✅ Sim Lab Playwright adoption smoke in CI (DT-17; make e2e-web-smoke passing)
- ✅ Run Sim Lab E2E smoke in production mode (DT-26)
- ✅ Expanded Sim Lab E2E smoke coverage + iOS simulator smoke (DT-29 through DT-35)
- ✅ Trim pre-commit scope to high-risk paths (DT-36)
- ✅ Validate dashboard-only pre-commit selection (DT-37)
- ✅ Suppress Sim Lab candidate telemetry noise (DT-27)
- ✅ Default predictive disabled for Sim Lab runs (DT-25)
- ✅ Observability foundation (structured logs + OTel collector + Tempo dashboards) (DT-14)
- ✅ Release channels + semver/changelog CI enforcement (DT-15)
- ✅ Local Sim Lab mocks + fixture feeds (DT-16)
- ✅ Contract-first API + generated SDKs (DT-18; strict OpenAPI coverage gate)
- ✅ Disposable iOS simulators for ci-ios runs (DT-19)
- ✅ Disposable watch simulator pairs for screenshots (DT-20)
- ✅ Versioned pre-commit hook with doc/log/image extension skip (DT-22)

---

## <a name="core-server"></a>Core Server

### In Progress
- 🚧 CS-77 WS-2902: expose status + rotate-token endpoints by node id
- 🚧 CS-80 Pi 5 deploy-over-SSH enables SPI0 automatically (ADS1263)

### Blocked
- ⏸ CS-81 Validate Pi 5 deploy-over-SSH SPI bootstrap on real hardware
- ⏸ CS-105 Validate DHCP churn does not break node config/sensors (Tier A)

### To Do
- [ ] No open items

### Done
- ✅ CS-108 Validate battery SOC estimator + power runway on real Renogy system (Tier A validated installed `0.1.9.274-battery-runway-fix`; run: `project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md`)
- ✅ CS-107 Battery model (estimated SOC) + power runway projection (local validation `make ci-core-smoke`; hardware validation CS-108)
- ✅ CS-106 Related sensors semantics for wind/rain (validated installed `0.1.9.272-cs106-related-sensors`; run: `project_management/runs/RUN-20260217-tier-a-cs106-wind-rain-related-sensors-0.1.9.272-cs106-related-sensors.md`)
- ✅ CS-104 DHCP-safe node-agent addressing (validated installed `0.1.9.258-cs104-dhcp-safe`; hardware validation CS-105; run: `project_management/runs/RUN-20260208-tier-a-cs104-dhcp-safe-0.1.9.258-cs104-dhcp-safe.md`)
- ✅ CS-102 Derived sensors: allow derived inputs (enable temp compensation of derived sensors) (Tier A validated installed `0.1.9.248-derived-of-derived`; Tier B DT-59)
- ✅ CS-101 Metrics: derived lag_seconds must work across bucket intervals (7d Trends) (Tier A validated installed `0.1.9.247-derived-lag-buckets`; Tier B DT-59)
- ✅ CS-99 Metrics: derived sensors must support forecast_points inputs (history for temp compensation) (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B DT-59)
- ✅ CS-100 Derived sensors: support per-input lag_seconds for temp compensation (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B DT-59)
- ✅ CS-92 Backups: implement real run + restore workflows (remove stub endpoints) (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59; run: `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`)
- ✅ CS-93 Schedule blocks: handle DST gaps/ambiguity without silently skipping events (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-98 Predictive API endpoints should be real or explicitly disabled (no stubbed “success”) (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-97 Remove unauthenticated “first user wins” bootstrap path (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-96 Require auth for core read endpoints beyond metrics/backups (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-95 Secure /api/dashboard/state snapshot (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-94 Secure setup credentials inventory endpoint (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-91 Secure backups read endpoints (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-90 Secure metrics query/ingest endpoints (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ CS-89 Renogy preset apply preserves ADS1263 node sensors (validated installed 0.1.9.235-adcfix)
- ✅ CS-88 Centralize sensor visibility policy (API boundary) (validated installed 0.1.9.165; Tier B DT-59)
- ✅ CS-87 Derived sensors: expand expression function library (math + trig + conditional) (Tier A validated installed 0.1.9.162; Tier B DT-59)
- ✅ CS-86 Sensor series integrity audit + enforcement (Tier A validated installed 0.1.9.153; Tier B DT-59)
- ✅ CS-85 Derived sensors (controller-computed from other sensors) (Tier A validated installed 0.1.9.152; Tier B DT-59)
- ✅ CS-84 WS-2902 pressure integrity: forbid external backfill; split relative vs absolute (Tier A validated installed 0.1.9.151; Tier B DT-59)
- ✅ CS-83 WS-2902: barometric pressure shows in Trends even when station uploads omit pressure (canceled: invalid data source mixing; replaced by CS-84)
- ✅ CS-82 Open-Meteo current adds barometric pressure metric (Tier A validated installed 0.1.9.149; Tier B DT-59)
- ✅ CS-59 Enforce “no permanent deletes” for telemetry history (Tier A validated installed 0.1.9.132)
- ✅ CS-76 Validate WS-2902 short ingest path/token on real station hardware (Tier A validated; Tier B DT-59)
- ✅ CS-78 WS-2902 cleanup: remove failed duplicate weather station nodes
- ✅ CS-79 Deleted nodes/sensors: stop controller-owned pollers/integrations (Tier A validated installed 0.1.9.138)
- ✅ OPS-1 Purge mistaken simulated Node 1 reservoir depth points (installed controller DB)
- ✅ CS-64 Remove production demo analytics fallbacks (explicit errors only) (deployed 0.1.9.14; tests pending)
- ✅ CS-61 Canonicalize role presets (admin/operator/view) in the API (validated installed 0.1.9.101; Tier B DT-59)
- ✅ CS-62 External “virtual nodes” support (Emporia devices) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ CS-63 Emporia ingest as sensors + metrics (per device/circuit) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ CS-65 Emporia multi-site preferences (exclude meters + address grouping) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ CS-71 Renogy BT-2 settings API (desired config + audit + apply workflow) (validated installed 0.1.9.73; hardware NA-61)
- ✅ CS-73 Offline map assets service (local tiles/glyphs/terrain + Swanton pack) (validated installed 0.1.9.112; Tier B DW-97)
- ✅ CS-74 Wire controller connection indicator (/api/connection) (validated installed 0.1.9.115; Tier B DT-59)
- ✅ CS-75 WS-2902 ingest: shorten token + short `/api/ws/<token>` path (validated installed 0.1.9.118; hardware CS-76)
- ✅ CS-72 Core node + forecast-backed sensors (Core node + weather/PV forecast surfaced as sensors; validated installed 0.1.9.91; Tier B DT-59)
- ✅ CS-67 Serve dashboard static assets with cache-safe headers (deployed 0.1.9.44)
- ✅ CS-68 Sensors CRUD: allow COV intervals (validated installed 0.1.9.69; Tier B DW-98)
- ✅ CS-70 Apply node sensor config to node-agent (dashboard-driven; deployed 0.1.9.61; Pi-only enforcement validated installed 0.1.9.113; Tier B DT-59)
- ✅ CS-57 Surface latest sensor values in core APIs (validated installed 0.1.9.70; Tier B DT-59)
- ✅ CS-66 Emporia cloud ingest: full electrical readbacks (V/A + nested devices) (validated installed 0.1.9.69; Tier B CS-69)
- ✅ CS-48 Validate deploy-from-server (SSH) on real Pi 5 hardware (node2 @ `10.255.8.20` validated)
- ✅ CS-54 WS-2902 “TCP/IP connect” integration mode (spec gap)
- ✅ CS-55 Default admin capability includes config.write
- ✅ CS-56 Session tokens reflect capability updates immediately
- ✅ CS-50 Secure deploy-from-server SSH credentials + add key-based auth option (audit)
- ✅ CS-51 Add API rate limiting on sensitive controller endpoints (audit)
- ✅ CS-52 Remove panic paths + silent data loss from Rust controller routes (audit)
- ✅ CS-49 Unify preset source-of-truth (CLI + dashboard) for Renogy/WS-2902
- ✅ CS-44 Support per-node display profile config (Pi 5 local display)
- ✅ CS-46 Issue and enforce read-only tokens for WAN portal pulls (WAN portal removed in ARCH-6)
- ✅ CS-45 Add “apply preset” config endpoints for Renogy BT-2 and WS-2902
- ✅ CS-47 Harden deploy-from-server (SSH) for product-grade UX
- ✅ CS-43 Split analytics tests into focused modules
- ✅ CS-42 Remote Pi 5 deployment API (SSH job + adoption token issuance)
- ✅ Implement authentication/authorization and enforce roles (CS-21)
- ✅ Guaranteed deterministic sensor/output identifiers and propagated rename updates
- ✅ Implemented real-mode user management
- ✅ Implemented real-mode schedules and alarms
- ✅ Expanded schema for all project entities
- ✅ Implemented REST APIs for most features in demo mode
- ✅ Implemented rolling average and configurable intervals for metrics
- ✅ Implemented change-of-value (COV) ingest for `interval=0` sensors
- ✅ Implemented default offline alarms
- ✅ Implemented analytics aggregation endpoints
- ✅ Implemented demo mode with discovery mocks
- ✅ Implemented MQTT consumer for outputs and alarms
- ✅ Expanded test coverage for new endpoints and jobs
- ✅ Added Alembic/SQL migrations
- ✅ Handled sensor deletion retention
- ✅ Implemented rich `/api/nodes` detail
- ✅ Implemented `/api/sensors/{id}` and `/api/outputs` endpoints
- ✅ Provided `/api/connection` endpoints
- ✅ Built schedules calendar API
- ✅ Implemented alarm definitions & history endpoints
- ✅ Enhanced metrics ingest/query
- ✅ Created analytics aggregation jobs + endpoints
- ✅ Implemented backups manager
- ✅ Kept demo mode from breaking alarm tests (CS-22)
- ✅ Optimized latest forecast selection query (CS-33)
- ✅ Offloaded predictive trace logging from async hot path (CS-34)
- ✅ Clamped predictive trace log size (CS-35)
- ✅ Hardened forecast + utility rate provider registry (CS-36)
- ✅ Stabilized analytics/demo serialization + feed hooks (CS-37)
- ✅ Preserved rate schedule period labels on default fallbacks (CS-40)
- ✅ Normalized test UTC timestamps + predictive env handling (CS-41)

---

## <a name="rust-core-server-migration"></a>Rust Core Server Migration

### To Do
- [ ] RCS-16 OpenTelemetry tracing
- [ ] RCS-17 Refactor outputs.rs
- [ ] RCS-20 Controller no-Python runtime guardrails

### In Progress
- [ ] No open items

### Done
- ✅ RCS-21 Core-server Python tooling rename and prune (completed via ARCH-6; legacy `apps/core-server/` removed)
- ✅ RCS-18 Integration tests
- ✅ RCS-19 Port conflict detection
- ✅ RCS-15 SQL error leakage
- ✅ RCS-14 Sunset the Python core-server (remove legacy runtime)
- ✅ RCS-12 Expand parity harness endpoint coverage beyond the “smoke subset”
- ✅ RCS-13 Switch local dev + CI default to Rust core-server
- ✅ RCS-1 Contract-first migration plan (ADR + parity harness)
- ✅ RCS-2 Rust core-server skeleton (API + static assets + OpenAPI)
- ✅ RCS-3 Static dashboard build (no Node runtime in production)
- ✅ RCS-4 Switch production controller runtime to Rust core-server
- ✅ RCS-5 Response parity harness (Python vs Rust)
- ✅ RCS-6 Implement `/api/dashboard/state` snapshot endpoint
- ✅ RCS-7 Enforce auth + capabilities in Rust core-server
- ✅ RCS-8 Expand Rust OpenAPI export for shipped endpoints
- ✅ RCS-9 Switch generated SDKs to Rust OpenAPI (contract-first)
- ✅ RCS-10 Implement missing OpenAPI paths in Rust core-server
- ✅ RCS-11 Make Rust core-server the canonical OpenAPI source

---

## <a name="telemetry-ingest-sidecar"></a>Telemetry Ingest Sidecar

### In Progress
- 🚧 TS-9 Node health telemetry split (ICMP ping vs MQTT RTT) + core controller parity (system sensors visible on Trends; Tier A pending)

### To Do
- [ ] No open items

### Done
- ✅ TSSE-37 Related sensors scoring: penalize diurnal lag artifacts + bound score by lag correlation (Tier A `0.1.9.255-related-diurnal-penalty`; Tier B DT-59)
### Done
- ✅ TS-7 Fix offline flapping for >5s sensors
- ✅ TS-8 Accept non-UUID node status topics + persist node health (validated installed 0.1.9.70; Tier B DT-59)
- ✅ TS-6 Split telemetry-sidecar ingest monolith into modules
- ✅ TS-1 Make the sidecar the only MQTT consumer
- ✅ TS-2 Run predictive alarms as a DB-driven Python worker
- ✅ TS-3 Align sidecar ingest semantics with core-server ingest
- ✅ TS-4 Add sidecar-only ingest regression tests
- ✅ TS-5 Align sidecar quality decoding with DB type

---

## <a name="offline-telemetry-spool-backfill-replay"></a>Offline Telemetry Spool + Backfill Replay

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Done
- ✅ OT-1 Phase 0: lock requirements + policy decisions (disk/time/security) + finalize ADR
- ✅ OT-2 Phase 1: Rust node-forwarder segment spool (framing + recovery + bounded retention)
- ✅ OT-3 Phase 1: Rust node-forwarder publish + replay (throttle + status priority)
- ✅ OT-4 Phase 1: node-agent sampling → local IPC (always-sample; no uplink coupling)
- ✅ OT-5 Phase 1: controller ACK topic + durable acked_seq (post-DB-commit)
- ✅ OT-6 Phase 1: controller liveness monotonicity (receipt-time last_rx_at + sample-time freshness)
- ✅ OT-7 Phase 1: enforce idempotent ingest invariants for QoS1 replay duplicates
- ✅ OT-8 Phase 1: spool health observability surfaces (node status + controller APIs + dashboards)
- ✅ OT-9 Phase 1: E2E harness for disconnect/reconnect + reboot-mid-outage + catch-up
- ✅ OT-10 Tier A validation run + evidence (installed controller; no DB/settings reset) (run: `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`)
- ✅ OT-11 Validate offline buffering on hardware (Pi 5 disconnect window + reboot-mid-outage) (run: `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`)
- ✅ OT-12 Prune legacy offline-buffer codepaths (single durability layer; no dead code)
- ✅ TICKET-0049 Requirements dump added
- ✅ ADR 0009 accepted

---

## <a name="standalone-rust-telemetry-predictive"></a>Standalone Rust Telemetry + Predictive

### To Do
- [ ] RS-1 Define standalone Rust ingest + predictive rollout plan
- [ ] RS-2 Implement standalone Rust ingest + predictive pipeline
- [ ] RS-3 Add dedicated tests and rollback validation for Rust pipeline

---

## <a name="node-agent"></a>Node Agent

### Blocked
- ⏸ NA-48 Validate generic Pi 5 node stack performance on hardware
- ⏸ NA-38 Mesh networking hardware validation (coordinator/end devices + soak test)
- ⏸ NA-39 BLE provisioning hardware validation (Pi/iOS)
- ⏸ NA-51 Validate pulse counter inputs on hardware
- ⏸ NA-54 Validate offline Pi 5 installs on real hardware (no WAN)
- ⏸ NA-40 Renogy BT-2 telemetry hardware validation (Pi 5 + BT-2; protocol decode aligned to Rover docs 2026-01-12)
- ⏸ NA-41 Renogy Pi 5 deployment workflow hardware validation (adopt + restore)
- ⏸ NA-61 Validate Renogy BT-2 settings apply flow on hardware

### In Progress
- [ ] NA-62 Pi 5 ADS1263 analog contract + fail-closed backend (remove “ADS1115” stubs; legacy `ads1115` alias; fix GPIO22 pin reservation leak; auto-detect `/dev/spidev10.0`; enable SPI by default in Pi imaging)
- [ ] NA-46 Reservoir depth pressure transducer hardware validation (Waveshare ADS1263 + 4–20 mA)
- [ ] NA-64 Fix Pi 5 deploy offline debs (remove RPi.GPIO; ship pigpiod + runtime deps)

### To Do
- [ ] No open items

### Done
- ✅ NA-66 Node-agent: require auth for config + provisioning HTTP endpoints (no secret leaks) (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59; run: `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`)
- ✅ NA-67 Node-agent provisioning: avoid blocking the event loop when applying Wi‑Fi credentials (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ NA-68 Node-agent restore/apply_config must validate and clamp timing fields (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59)
- ✅ NA-53 Offline-capable Pi 5 node installs (no internet required on the Pi)
- ✅ NA-55 Publish per-node network health telemetry (ping/latency/jitter + uptime %)
- ✅ NA-56 Publish per-node CPU/RAM telemetry (including per-core CPU)
- ✅ NA-57 Clarify + implement “power-on / auto-recovery” behavior for Pi 5 nodes (spec)
- ✅ NA-60 Renogy BT-2 Modbus settings write support (safe apply + read-back verify) (unit tests; hardware NA-61)
- ✅ NA-63 Renogy BT-2 BlueZ cached-device fallback (Tier A: user validated visually)
- ✅ NA-65 Renogy BT-2: PV energy kWh sensors (today + total) (Tier A validated installed `0.1.9.231`; run: `project_management/runs/RUN-20260131-tier-a-na65-renogy-kwh-sensors-0.1.9.231.md`)
- ✅ NA-44 Pi 5 local display (basic status + live values)
- ✅ NA-45 Pi 5 local display (advanced controls + trends)
- ✅ NA-47 Implement generic Pi 5 node stack baseline (single stack + feature toggles)
- ✅ NA-50 Implement counter-based pulse inputs (flow/rain) + delta telemetry
- ✅ NA-43 Implement reservoir depth pressure transducer via node (ADS1263 + 4–20 mA)
- ✅ Basic FastAPI app with local UI and API endpoints
- ✅ Drivers abstraction with mock capabilities
- ✅ MQTT publisher for telemetry and heartbeat
- ✅ BLE provisioning service (Linux/BlueZ + HTTP fallback)
- ✅ Systemd unit for auto-start
- ✅ Demo data for local dashboard
- ✅ Simplified provisioning via `ConfigStore` and `/v1/config`
- ✅ SD-card imaging and flashing scripts
- ✅ Publish telemetry/heartbeat with configurable intervals
- ✅ Change-of-value (COV) sensors only publish on change
- ✅ Node-agent test suite runs on Python 3.14 without DBus (dbus-next shims)
- ✅ `/v1/config` supports restore push
- ✅ `/v1/status` for adoption preview
- ✅ Include MAC addresses in discovery advertisement
- ✅ Core adoption flow integration
- ✅ Encrypted Wi-Fi secrets in provisioning queue
- ✅ NA-59 Apply live sensor list updates safely (clear sensors + prune publisher scheduling state)
- ✅ Mesh radio adapter and telemetry buffer integration
- ✅ Telemetry rolling averages
- ✅ Mesh backfill flow
- ✅ Replace asyncio-mqtt with aiomqtt (NA-25)
- ✅ Runtime simulation profile controls for Sim Lab (NA-26)
- ✅ Heartbeat outputs payload list shape (NA-27)
- ✅ Raspberry Pi 5 simulator runner (NA-30)
- ✅ Pi 5 simulator core registration mode for full-stack E2E (NA-34)
- ✅ Sensor category driver mapping (NA-31)
- ✅ Renogy BT-2 external ingest bridge (NA-32)
- ✅ Normalize sensor type strings for Sim Lab telemetry (NA-33)
- ✅ Pi 5 simulator bundle config support (NA-35)
- ✅ Renogy deployment bundle includes load voltage/current defaults (NA-37)
- ✅ NA-42 Split node-agent monolith into routers + schemas
- ✅ NA-20 Mesh adapter + pairing scaffold + telemetry buffer wiring
- ✅ NA-36 Dedupe Pi 5 simulator core registration sensor/output IDs
- ✅ NA-52 Config-driven optional service enable/disable watcher (systemd path)

---

## <a name="discovery-and-adoption"></a>Discovery and Adoption

### In Progress
- [ ] No open items

### Done
- ✅ CS-60 Controller-issued adoption tokens + auto-register sensors on adopt (MAC-bound + TTL; no node-token fallback) (validated installed 0.1.9.70; Tier B DT-59)
- ✅ Extend node agent advertisement with more metadata
- ✅ Enforce uniqueness constraints and stable naming logic
- ✅ Zeroconf scanner for discovery
- ✅ Adoption tokens stored in the database
- ✅ `/api/scan` and `/api/adopt` endpoints
- ✅ Dashboard adoption wizard
- ✅ Updated core zeroconf module
- ✅ Seeded demo adoption tokens

---

## <a name="dashboard-web"></a>Dashboard Web

### In Progress
- 🚧 DW-69 Show “Active Development” banner during agent work (deployed 0.1.9.28; tests pending)
- 🚧 DW-158 Trends: per-panel analysis keys + plain-English variable labels (reopened: per-subsection Keys + variable definitions)
- 🚧 DW-159 Sensors & Outputs: Add sensor drawer UX/IA refactor
- 🚧 DW-160 Dashboard web: Standardize collapsible section containers across tabs
- 🚧 DW-161 Overview: Redesign “Configure local sensors” UX (node hierarchy + drag/drop + mobile)
- 🚧 DW-162 Analytics: Per-container time range controls (24h / 72h / 7d)
- 🚧 DW-208 Analytics Overview: Soil moisture card defaults to open (local validation; Tier A pending)
- 🚧 DW-209 Analytics Overview: mobile overflow + range selector shadcn migration + mobile pinch-zoom-out enablement (Tier A refreshed 0.1.9.238-analytics-zoom; screenshot viewing pending)
- 📝 2026-01-19: Continuing DW-158..DW-162 after follow-up UX feedback (per-section Keys + variable tooltips, drawer cohesion, expand collapsible coverage, and Tier A evidence capture).
- 🚧 DW-227 Trends: sensor picker checkbox selection parity (implemented + local validation; Tier A pending)
- 🚧 DW-228 Trends: Pattern & Anomaly chart layout parity + Related Sensors context range options (`±1h`, `±3h`, `Custom ±hours`) (implemented + local validation; Tier A pending)
- 🚧 DW-229 Dashboard Web: standardize Highcharts render path with shared `HighchartsPanel` wrapper (implemented + local validation; Tier A pending)
- 🚧 DW-185 Setup Center: extract section components from SetupPageClient
- 📝 2026-01-22: Wired Setup Center Weather + Solar PV forecast sections into `SetupPageClient` (shared setup API modules + validation helpers); `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
- 📝 2026-01-22: Wired Setup Center Integrations + AI anomaly detection sections into `SetupPageClient` and guarded Integrations mutations behind `config.write`; `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.
- 📝 2026-01-22: Audit confirmed shared `sensorOrigin`/`powerSensors` helpers in Analytics Overview/Power/Setup pages with no local re-impls; small consolidation candidates remain (`Ws2902SensorBuilder` `configString`, Trends `sensorSourceBucket`).
- 📝 2026-01-22: Analytics Overview now pulls nodes/sensors via shared `useAnalyticsData` in WeatherStation/Status sections to avoid duplicate queries.
- 📝 2026-01-22: Extracted Analytics weather station UI + shared chart helpers into new component files (DW-187; pending wiring into `AnalyticsOverview.tsx`).
- 📝 2026-01-22: Extracted Analytics PV forecast section into its own component and moved PV windowing logic into a testable helper (DW-187).
- 📝 2026-01-22: Completed DW-187 Analytics Overview de-bloat refactor (module split + shared hooks/libs + PV helper tests); `make ci-web-smoke`, `cd apps/dashboard-web && npm run build`, and `cd apps/dashboard-web && npm test` pass.
- 📝 2026-01-22: Related sensors scans: added adaptive metrics batching + large-scan confirmation/progress/cancel UX so 7d + 1m scans no longer fail with “Requested series too large … max 25000” (DW-188; Tier A validated 0.1.9.198).
- 📝 2026-02-06: Completed DW-221. Added Rust analysis job `related_sensors_unified_v2` (event-match + co-occurrence merged ranking with confidence tiers/evidence), refreshed Trends Related Sensors to Simple/Advanced unified UX (quick suggest + refine + insight cards + unified preview), and updated Playwright stubs. Tier A validated installed `0.1.9.251-related-unified-v2` (run: `project_management/runs/RUN-20260206-tier-a-dw221-related-sensors-unified-0.1.9.251-related-unified-v2.md`); Tier B deferred to DW-98.
- 📝 2026-02-06: Completed DW-222 after Tier A feedback. Related Sensors preview now falls back to raw candidate series when lag alignment yields `<=1` point while raw has more, and shows explanatory copy. Tier A validated installed `0.1.9.252-preview-fallback` (run: `project_management/runs/RUN-20260206-tier-a-dw222-preview-fallback-0.1.9.252-preview-fallback.md`; viewed screenshots: `manual_screenshots_web/20260206_015437/trends_related_sensors_large_scan.png`, `manual_screenshots_web/20260206_015437/trends_related_sensors_scanning.png`). Tier B deferred to DW-98.
- 📝 2026-02-06: Completed DW-223/DW-224 on installed controller `0.1.9.254-matrix-refresh-fix`: Related Sensors Simple mode restores the matrix-first visual scan with score cutoff include+cap, Trends includes a separate Selected Sensors correlation matrix card, and the follow-up matrix auto-refresh submit loop was fixed to prevent layout jitter. Tier A run: `project_management/runs/RUN-20260206-tier-a-dw223-dw224-matrix-refresh-fix-0.1.9.254-matrix-refresh-fix.md`.
- 📝 2026-02-17: Completed DW-181 on installed controllers `0.1.9.269` + correction `0.1.9.270` + evaluate-all follow-up `0.1.9.271`: Related Sensors defaults are completeness-first (`candidate_source=all_sensors_in_scope`, `evaluate_all_eligible=true`) and backend evaluate-all is no longer gated by Advanced/eligible<=500. Follow-up corrections landed same day: Simple mode defaults to `All nodes`, Simple “Broaden”/“Refine” shortcuts were removed, and evaluate-all runs no longer prefilter away candidates before scoring (plus higher event/co-occurrence sensor ceilings). Tier A validated (runs: `project_management/runs/RUN-20260217-tier-a-dw181-related-sensors-all-scan-0.1.9.269.md`, `project_management/runs/RUN-20260217-tier-a-dw181-simple-all-nodes-correction-0.1.9.270.md`, `project_management/runs/RUN-20260217-tier-a-dw181-evaluate-all-eligible-0.1.9.271.md`; screenshot hard gates PASS). Tier B deferred indefinitely to DW-98 per user instruction.
- 🚧 DW-121 Weather station: add “Rotate token / setup” entrypoint on node detail
- 🚧 DW-77 Dashboard: edit node/sensor names + sensor display decimals (per-sensor + bulk by type) (implemented; tests pending)
- 🚧 DW-78 Dashboard IA cleanup (Overview tab + remove legacy catch-all sections) (deployed 0.1.9.35; tests pending; nav badge polish)
- 🚧 DW-82 Enforce light mode (disable dark mode styling) (deployed 0.1.9.41; tests pending)

### To Do
- [ ] DW-196 shadcn Phase 5 — component upgrades (Switch, AlertDialog, Toast, ScrollArea, etc.)
- [ ] DW-166 Overview: Move “Feed health” into Overview (remove from Analytics Overview)

### Deferred / Optional
- [ ] DW-62 Add system topology tab (UniFi inventory + node association)
- [ ] DW-63 UniFi Protect events UI (motion + AI thumbnails)

### Done
- ✅ DW-258 Tier A validate battery/runway UI on installed controller (validated installed `0.1.9.274-battery-runway-fix`; run: `project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md`)
- ✅ DW-257 Setup Center + Power tab: battery capacity/SOC/runway UX (local validation `make ci-web-smoke`; Tier A validation DW-258)
- ✅ DW-181 Trends: Related sensors defaults to scanning all sensors (Simple defaults to `All nodes`; no Simple “Refine” path; evaluate-all no longer prefilters candidate pool; Tier A validated installed `0.1.9.271`; run: `project_management/runs/RUN-20260217-tier-a-dw181-evaluate-all-eligible-0.1.9.271.md`; Tier B deferred indefinitely to DW-98 per user instruction)
- ✅ DW-230 Trends: chart analysis toolbar v2 (drag best-fit, multi-window fit cards, explicit save/update/delete + persisted hydration) (Tier A validated installed `0.1.9.259-dw230-trends-bestfit`; run: `project_management/runs/RUN-20260209-tier-a-dw230-trends-bestfit-0.1.9.259-dw230-trends-bestfit.md`; Tier B deferred to DW-98)
- ✅ DW-231 Trends Related Sensors: operator contract + UI copy-labeling cleanup (Rank score, Evidence, coverage disclosure + correlation “not used for ranking” framing) (validated locally)
- ✅ DW-237 Unified v2: tolerant event alignment matching (tolerance buckets) + efficient matcher (TICKET-0053) (validated locally)
- ✅ DW-238 Unified v2: directionality same vs opposite computation and UI (TICKET-0071) (validated locally)
- ✅ DW-239 Trends Related Sensors: contract test suite (gap, aggregation, lag sign, derived labeling) (TICKET-0068) (validated locally)
- ✅ DW-243 Trends Related Sensors: pin semantics (pinned candidates always evaluated) (TICKET-0056) (validated locally)
- ✅ DW-244 Trends Related Sensors: show top lag candidates (top 3 lags) (TICKET-0076) (validated locally)
- ✅ DW-245 Trends Related Sensors: backend candidate pool query (all sensors by scope) (TICKET-0074) (validated locally)
- ✅ DW-246 Trends Related Sensors: periodic and diurnal driver mitigation (deseasoning + low-entropy penalty) (TICKET-0073) (validated locally)
- ✅ DW-247 Trends Related Sensors: delta correlation evidence channel (optional third signal) (TICKET-0075) (validated locally)
- ✅ DW-248 Trends: correlation block refinements (delta corr, lagged corr, focus-vs-candidate default) (TICKET-0061) (validated locally)
- ✅ DW-249 Unified v2: data quality + missingness surfacing (TICKET-0058) (Tier A validated installed `0.1.9.262-dw249-missingness`; run: `project_management/runs/RUN-20260210-tier-a-dw249-missingness-0.1.9.262-dw249-missingness.md`; Tier B deferred to DW-98)
- ✅ DW-224 Trends: separate Selected Sensors correlation matrix card (Tier A validated installed `0.1.9.254-matrix-refresh-fix`; Tier B deferred to DW-98)
- ✅ DW-223 Trends: Related Sensors matrix-first visual scan in Simple mode + auto-refresh loop fix (Tier A validated installed `0.1.9.254-matrix-refresh-fix`; Tier B deferred to DW-98)
- ✅ DW-222 Trends: Related Sensors preview sparse-lag fallback (single-point aligned candidate guard) (Tier A validated installed `0.1.9.252-preview-fallback`; Tier B deferred to DW-98)
- ✅ DW-221 Trends: Related Sensors v2 unified refresh (Simple/Advanced + unified backend job) (Tier A validated installed `0.1.9.251-related-unified-v2`; Tier B deferred to DW-98)
- ✅ DW-197 Trends: Interactive line-of-best-fit + analysis tools polish (Tier A validated; Tier B deferred to DW-98)
- ✅ DW-200 Dashboard web: mobile horizontal overflow is reachable (validated locally)
- ✅ DW-201 Sensors & Outputs: fix overlapping/garbled layout regression (validated locally)
- ✅ DW-220 Derived Sensor Builder: document derived-of-derived guardrails (depth/cycles) (Tier A validated installed `0.1.9.249-derived-builder-guardrails`; Tier B deferred to DW-98)
- ✅ DW-218 Analytics Temp Compensation: allow compensating derived sensors (Tier A validated installed `0.1.9.248-derived-of-derived`; Tier B deferred to DW-212)
- ✅ DW-219 Derived sensors: allow derived inputs in Derived Sensor Builder (Tier A validated installed `0.1.9.248-derived-of-derived`; Tier B deferred to DW-98)
- ✅ DW-216 Analytics Temp Compensation: detrend slow changes + add fit diagnostics (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B deferred to DW-212)
- ✅ DW-217 Analytics Temp Compensation: add temperature lag (auto + derived lag_seconds) (Tier A validated installed `0.1.9.246-temp-comp-lag`; Tier B deferred to DW-212)
- ✅ DW-214 Analytics Temp Compensation: allow custom training window (Tier A validated installed `0.1.9.241-analytics-mobile-window`; run: `project_management/runs/RUN-20260202-tier-a-dw209-dw214-0.1.9.241-analytics-mobile-window.md`; Tier B deferred to DW-212)
- ✅ DW-213 Highcharts: fix WebKit crash when disabling chart zoom (Tier A validated installed `0.1.9.240-highcharts-zooming-fix`; run: `project_management/runs/RUN-20260202-tier-a-dw213-highcharts-zooming-fix-0.1.9.240-highcharts-zooming-fix.md`)
- ✅ DW-215 Trends: Sensor picker overflow fix (Tier A validated installed `0.1.9.243-dw215-sensor-picker-overflow`; run: `project_management/runs/RUN-20260203-tier-a-dw215-sensor-picker-overflow-0.1.9.243-dw215-sensor-picker-overflow.md`; Tier B deferred to DW-98)
- ✅ DW-211 Analytics: assisted temperature drift compensation wizard (derive compensated sensor from target + temperature reference) (Tier A validated installed `0.1.9.239-temp-compensation`; Tier B deferred to DW-212; run: `project_management/runs/RUN-20260202-tier-a-dw211-temp-compensation-0.1.9.239-temp-compensation.md`)
- ✅ DW-206 Trends: increase default Trend chart height (Tier A validated installed `0.1.9.236-trends-height`; viewed screenshot: `manual_screenshots_web/tier_a_0.1.9.236_trends_height_20260201_143200/trends.png`)
- ✅ DW-207 Trends: Key panels default to collapsed (Tier A validated installed `0.1.9.237-trends-keys`; viewed screenshot: `manual_screenshots_web/tier_a_0.1.9.237_trends_keys_20260202_211900/trends.png`)
- ✅ DW-210 Trends: Related Sensors results selection must not reset (Tier A validated installed `0.1.9.242-dw210-related-selection`; run: `project_management/runs/RUN-20260203-tier-a-dw210-related-sensors-selection-0.1.9.242-dw210-related-selection.md`)
- ✅ DW-202 Dashboard-web: self-healing dev auth + screenshots (ADR-0007 / TICKET-0047; validated via `make ci-web-smoke`, `npm run build`, and `web-screenshots` evidence under `manual_screenshots_web/20260131_dev_auth_workflow_*`)
- ✅ DW-198 Trends: chart analysis toolbar (custom tool palette wired to Highcharts navigation bindings + persisted annotations) (Tier A validated installed `0.1.9.229`; run: `project_management/runs/RUN-20260131-tier-a-dw198-trends-chart-analysis-toolbar-0.1.9.229.md`)
- ✅ DW-199 Sensors: fix sensor drawer crash (Highcharts stock-tools bindings) (Tier A validated installed `0.1.9.231`; run: `project_management/runs/RUN-20260131-tier-a-na65-renogy-kwh-sensors-0.1.9.231.md`)
- ✅ DW-195 shadcn Phase 3 completion + dead code pruning (1,571 gray→token replacements across 109 files; 9 dead files deleted; 8 unused exports removed; lint+test+build pass)
- ✅ DW-194 Trends: Key text introduces jargon (TSSE/MAD/F1/r/n) (Tier A validated installed `0.1.9.213`; Tier B deferred to DW-98)
- ✅ DW-191 Map tab refactor (DW-189/DW-190/DW-191) (Tier A validated installed `0.1.9.199`; run: `project_management/runs/RUN-20260123-tier-a-dw189-dw190-dw191-map-refactor-0.1.9.199.md`)
- ✅ DW-192 Map tab: post-upgrade manual smoke checklist (docs/qa/map-tab-upgrade-smoke.md)
- ✅ DW-188 Trends: Related sensors week+1m scans (adaptive batching + confirm/progress/cancel) (Tier A validated installed 0.1.9.198; Tier B deferred to DW-98)
- ✅ DW-187 Analytics Overview: de-bloat `AnalyticsOverview.tsx` (module split + shared hooks/libs) (CI + build + vitest pass)
- ✅ DW-163 Sensors & Outputs: stack Outputs below Sensors in node panels (CI + build pass)
- ✅ DW-157 Dashboard display order: drag-and-drop reorder nodes + sensors (persistent) (Tier A validated installed 0.1.9.168; Tier B deferred to DW-114/DW-98)
- ✅ DW-156 Trends: event-match mode + analysis key + opt-in deep computations (validated installed 0.1.9.166; Tier B DW-98)
- ✅ DW-155 Trends tab UI polish (cohesive layout) (validated installed 0.1.9.165; Tier B DW-98)
- ✅ DW-154 Centralize hide behavior via sensor visibility policy (validated installed 0.1.9.165; Tier B DW-97/DW-98)
- ✅ DW-153 Trends “Related sensors”: acknowledge/deprioritize + all-nodes comparisons (validated installed 0.1.9.165; Tier B DW-98)
- ✅ DW-152 Unify “Public provider data” labeling + sensor origin badges (validated installed 0.1.9.165; Tier B DW-114)
- ✅ DW-151 Fix regression: “Hide live weather” still shows Open‑Meteo sensors (Tier A validated installed 0.1.9.164; Tier B DW-98)
- ✅ DW-176 Trends: Savitzky–Golay smoothing toggle + advanced settings (Tier A validated installed 0.1.9.187; Tier B deferred to DW-98)
- ✅ DW-177 Trends: increase overlay sensor limit (>10) (Tier A validated installed 0.1.9.188; Tier B deferred to DW-98)
- ✅ DW-178 Trends: Co-occurring anomalies (multi-sensor) (Tier A validated installed 0.1.9.193; Tier B deferred to DW-98)
- ✅ DW-179 Weather station nodes: add custom sensors via dashboard (soil moisture, etc.) (Tier A validated installed 0.1.9.194; Tier B deferred to DW-98)
- ✅ DW-180 Trends: expand range + interval presets (10m/1h, 1s/30s) (Tier A validated installed 0.1.9.195; Tier B deferred to DW-98)
- ✅ DW-150 Trends: show sensor origin badges + hide external sensors toggle (Tier A validated installed 0.1.9.163; Tier B DW-98)
- ✅ DW-149 Derived sensor builder: expose extended function library + insert helpers (Tier A validated installed 0.1.9.162; Tier B DW-98)
- ✅ DW-147 Alarm Events: click-through detail drawer + context charts (Tier A validated installed 0.1.9.158; Tier B DW-114)
- ✅ DW-146 Overview: Telemetry tapestry layout stability + regression tests (Tier A validated installed 0.1.9.157; Tier B DW-114)
- ✅ DW-145 Trends: resizable chart height (Tier A validated installed 0.1.9.156; Tier B DW-98)
- ✅ DW-144 Derived sensors: create via “Add sensor” drawer UI (Tier A validated installed 0.1.9.152; Tier B DW-98)
- ✅ DW-143 Sensors & Outputs: do not auto-expand the first node (Tier A validated installed 0.1.9.150; Tier B DW-114)
- ✅ DW-142 Show node type badges next to node titles across the dashboard (Tier A validated installed 0.1.9.150; Tier B DW-114)
- ✅ DW-141 Sensors & Outputs: don’t auto-expand the first node (Tier A validated installed 0.1.9.149; Tier B DW-114)
- ✅ DW-140 Trends: render sparse series + show “last seen” when empty (Tier A validated installed 0.1.9.149; Tier B DW-98)
- ✅ DW-139 Trends: Matrix Profile explorer (motifs + anomalies + heatmap) (Tier A validated installed 0.1.9.147)
- ✅ DW-138 Overview: configure which sensors appear (and order) for local visualizations (Tier A validated installed 0.1.9.146)
- ✅ DW-137 Analytics: reservoir depth gauges default to 15 ft full-scale (Tier A validated installed 0.1.9.145)
- ✅ DW-136 Alarm events: collapse acknowledged events by default (Tier A validated installed 0.1.9.144)
- ✅ DW-135 Analytics: weather station section (WS-2902) + rich visualizations (Tier A validated installed 0.1.9.143)
- ✅ DW-134 Validate numeric input UX on installed controller (Tier A screenshots; validated installed 0.1.9.142)
- ✅ DW-132 Fix numeric input UX (decimals + range typing) across dashboard-web (tests/build pass; Tier A validated installed 0.1.9.142 via DW-134)
- ✅ DW-131 Analytics: split reservoir depth into depth charts + rich live depth panel (Tier A validated installed 0.1.9.141)
- ✅ DW-130 Overview: advanced local sensor visualizations (Tier A validated installed 0.1.9.140)
- ✅ DW-129 Map layout fills viewport height (Tier A validated installed 0.1.9.139)
- ✅ DW-74 Show node offline duration everywhere node status is shown (Tier A validated installed 0.1.9.100; Tier B DT-59)
- ✅ DW-128 Overview: fix Mermaid sitemap arrow rendering (Tier A validated installed 0.1.9.137)
- ✅ DW-127 Alerts: “Acknowledge all” actions in dashboard UI (Tier A validated installed 0.1.9.135)
- ✅ DW-126 Per-node toggle: hide live weather (Open-Meteo) from UI (Tier A validated installed 0.1.9.134)
- ✅ DW-125 Mark non-local sensors (forecast/API) with badges (Tier A validated installed 0.1.9.133)
- ✅ DW-118 Nodes: admin-only soft delete action (UI) (Tier A validated installed 0.1.9.132)
- ✅ DW-124 Sensors: soft delete action (UI) (Tier A validated installed 0.1.9.132)
- ✅ DW-119 Map: fix client-side exception on navigation away from Map (Tier A validated installed 0.1.9.127)
- ✅ DW-122 Nodes: merge node detail drawer into node detail page (remove drawer) (Tier A validated installed 0.1.9.126)
- ✅ DW-123 Sensors: merge sensor detail page into sensor detail drawer (remove detail page UX) (Tier A validated installed 0.1.9.126)
- ✅ DW-100 Configure node sensors from dashboard (push to node-agent) (deployed 0.1.9.61)
- ✅ DW-101 Renogy controller settings UI (BT-2 Modbus apply workflow) (validated installed 0.1.9.73; hardware NA-61)
- ✅ DW-105 Overview “Where things live” Mermaid sitemap (validated installed 0.1.9.87; Tier B DT-59)
- ✅ DW-111 Remove Provisioning tab; Deployment includes adopt + naming (validated installed 0.1.9.97; Tier B DT-59)
- ✅ DW-112 Node location editor in “More details” drawer (validated installed 0.1.9.102; Tier B DT-59)
- ✅ DW-106 Flatten node sensor config nesting (Hardware sensors) (validated installed 0.1.9.103; Tier B DT-59)
- ✅ DW-107 Map tab IA/UX cleanup (remove Street View + reduce placement friction) (validated installed 0.1.9.103; Tier B DW-97)
- ✅ DW-116 Offline-first Map tab stack (local tiles/glyphs/terrain + GeoJSON layers) (validated installed 0.1.9.112; Tier B DW-97)
- ✅ DW-117 Chart x-axis pan/zoom on all graphs (validated installed 0.1.9.114; Tier B DW-98)
- ✅ DW-108 Trends custom start/end datetime range (validated installed 0.1.9.103; Tier B DW-98)
- ✅ DW-109 Power AC voltage quality analysis (Emporia mains voltage) (validated installed 0.1.9.103; Tier B CS-69)
- ✅ DW-110 Power DC voltage quality analysis (Renogy voltage rails) (validated installed 0.1.9.103; Tier B CS-69)
- ✅ DW-113 Cross-tab layout consistency (headers/banners/spacing) (validated installed 0.1.9.103; Tier B DW-114)
- ✅ DW-115 Sensors & Outputs “Add sensor” row (Pi node-agent nodes only) (validated installed 0.1.9.113; Tier B DW-114)
- ✅ DW-56 Rename “Control” role to “Operator” (UX polish) (validated installed 0.1.9.101; Tier B DT-59)
- ✅ DW-95 UI: capability-gate Users + Outputs actions (validated installed 0.1.9.101; Tier B DT-59)
- ✅ DW-57 Add progress feedback for “Refresh” and “Scan again” actions (validated installed 0.1.9.75; Tier B DT-59)
- ✅ DW-58 Dashboard UI for node network health trends (ping/latency/jitter/uptime)
- ✅ DW-59 Dashboard UI for Pi 5 node resource telemetry (CPU/RAM)
- ✅ DW-60 Expand preconfigured device templates in provisioning dropdowns (validated installed 0.1.9.75; Tier B DT-59)
- ✅ DW-29 Prefer controller-issued adoption token in adopt flow (validated installed 0.1.9.70; Tier B DT-59)
- ✅ DW-61 Map tab (MapLibre + placements + polygons/overlays) (validated installed 0.1.9.69; Tier B DW-97)
- ✅ DW-71 Map basemap rendering (blank canvas) fix (validated installed 0.1.9.69; Tier B DW-97)
- ✅ DW-72 Named map saves (save-as + compact loader dropdown) (validated installed 0.1.9.69; Tier B DW-97)
- ✅ DW-73 Per-node live weather (from active map placement) (validated installed 0.1.9.69; Tier B DW-97)
- ✅ DW-75 Analytics layout + Power nodes table rendering fixes (validated installed 0.1.9.69; Tier B CS-69)
- ✅ DW-76 Trends independent-axis UX + runaway-height fix (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-79 Analytics feed health includes forecast providers (validated installed 0.1.9.69; Tier B CS-69)
- ✅ DW-80 Emporia voltage/current UI + graph access (validated installed 0.1.9.69; Tier B CS-69)
- ✅ DW-81 Analytics PV overlay historical forecast (validated installed 0.1.9.69; Tier B CS-69)
- ✅ DW-83 Trends long-range presets + auto interval (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-84 Emporia per-circuit preferences (poll/hidden/in totals) (validated installed 0.1.9.69; Tier B CS-69)
- ✅ DW-86 Trends UX reorg + dashboard IA audit (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-87 Trends custom range + interval (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-88 Trends correlation + relationship analysis (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-93 Trends gaps for offline windows + strict parsing/CSV hygiene (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-66 Power tab (node-centric Emporia + Renogy dashboards; W/V/A graphs) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ DW-67 Node-first Sensors + Trends (reduce clutter, add context) (validated installed 0.1.9.75; Tier B DW-98)
- ✅ DW-70 Emporia meter preferences UI (exclude meters + address grouping) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ DW-85 Clarify Nodes vs Sensors & Outputs responsibilities (moved per-node IO workflow into Sensors & Outputs) (validated installed 0.1.9.95; Tier B DT-59)
- ✅ DW-94 UI shows COV interval as “COV” (validated installed 0.1.9.69; Tier B DW-98)
- ✅ DW-91 Mobile nav interactions + Playwright mobile audit
- ✅ DW-92 Fix Sensors page crash on sensor click
- ✅ DW-65 Migrate dashboard UI to Preline (admin/settings templates)
- ✅ DW-68 Remove demo/fake analytics + API fallbacks (explicit errors only) (deployed 0.1.9.14; tests pending)
- ✅ DW-64 System banner UX: compact empty schedule + move scan control
- ✅ DW-54 Add dashboard login UX + token persistence
- ✅ DW-55 Capabilities management UX (edit after creation + config.write)
- ✅ DW-53 Build a read-only dashboard bundle for the WAN portal (WAN portal removed in ARCH-6)
- ✅ DW-49 Add node display profile editor UI (Pi 5 local display)
- ✅ DW-102 Nodes drawer layout polish (Local display under Outputs + collapsible)
- ✅ DW-103 Fix Node health history toggle crash + clarify label
- ✅ DW-104 Node detail drawer IA cleanup (cohesive layout + visual hierarchy)
- ✅ DW-50 Renogy BT-2 one-click setup UX (preset apply)
- ✅ DW-52 Deploy-from-server UX hardening (SSH)
- ✅ DW-51 WS-2902 one-click setup UX (weather station) (hardened 0.1.9.53: LAN host hint + sample upload + humidity/wind gust/rain rate)
- ✅ DW-48 Split ScheduleForm monolith
- ✅ DW-47 Split provisioning wizard monolith
- ✅ DW-46 Stabilize Sim Lab smoke backup restore selection
- ✅ DW-45 Expanded sensor presets in provisioning config generator
- ✅ DW-44 Remote Pi 5 deployment UI (SSH form + progress + logs)
- ✅ DW-42 Allow dev origins for Sim Lab smoke
- ✅ DW-39 Sim Lab testing dashboard UI
- ✅ DW-40 Redesign Sim Lab console layout (domain-first)
- ✅ DW-41 Wire Sim Lab console to core + Sim Lab control APIs
- ✅ DW-38 Run npm audit fix for dashboard-web
- ✅ DW-37 Silence safe ESLint warnings in dashboard web
- ✅ DW-36 Refresh baseline-browser-mapping dev dependency
- ✅ DW-34 Split nodes page into reusable components/hooks
- ✅ DW-33 Deduplicate analytics formatting helpers
- ✅ DW-26 Replace global SWR snapshot with domain React Query hooks
- ✅ DW-25 Fix dashboard web UI regressions and polish
- ✅ DW-32 Surface retention policy update failures
- ✅ DW-31 Debounce discovery scan action
- ✅ Add component/unit tests for critical flows (adoption wizard, calendar edits)
- ✅ Global layout with connection banner and quick actions
- ✅ Nodes page with detail drawer and adoption wizard
- ✅ Sensors & Outputs page with detail panels
- ✅ Users management UI
- ✅ Schedules weekly calendar
- ✅ Rich schedule editor (Visual Builder + Advanced JSON toggle)
- ✅ Schedule builder polish (edit-in-place + sensor/output pickers + validation)
- ✅ Playwright screenshot smoke tests (manual_screenshots_web)
- ✅ Trends page
- ✅ Analytics dashboard
- ✅ Backups browser and restore workflow
- ✅ Settings for integrations and demo mode
- ✅ Comprehensive mock data wiring
- ✅ Retention/backups UI tests
- ✅ Trends axis toggle test
- ✅ Adoption restore selector
- ✅ Restore activity feed poll + RTL coverage
- ✅ Deprecated (hidden): Provisioning config generator (node + sensor JSON builder; replaced by Deployment)
- ✅ Sensor templates/presets for node sensor config (used in Sensors & Outputs)
- ✅ Optional bearer token support for API requests (NEXT_PUBLIC_AUTH_TOKEN)
- ✅ Insightface-inspired theme refresh (global palette, glass header, predictive alarm pills)
- ✅ Sidebar shell + hero header (insightface layout lift across tables/forms/drawers)

---

## <a name="schedules-and-alarms"></a>Schedules and Alarms

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Done
- ✅ SA-17 Tier A validate incidents + alarm builder guidance/backtest on installed controller (installed `0.1.9.265-alarms-incidents`; run: `project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`)
- ✅ SA-16 Revamp dashboard `/alarms` into incident-first triage + investigation (Tier A validated installed `0.1.9.265-alarms-incidents`; run: `project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`; Tier B DT-59)
- ✅ SA-15 Add alarm rule backtest (historical replay) + builder step (Tier A validated installed `0.1.9.265-alarms-incidents`; run: `project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`; Tier B DT-59)
- ✅ SA-14 Add alarm rule stats/bands guidance endpoint + dashboard builder UX (Tier A validated installed `0.1.9.265-alarms-incidents`; run: `project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`; Tier B DT-59)
- ✅ SA-13 Add incident management backend + APIs (incidents, notes, grouping, action logs) (Tier A validated installed `0.1.9.265-alarms-incidents`; run: `project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`; Tier B DT-59)
- ✅ SA-12 Tier A validate conditional alarms on installed controller (installed `0.1.9.263`; run: `project_management/runs/RUN-20260210-tier-a-sa10-sa11-alarms-0.1.9.263.md`)
- ✅ SA-11 Build dashboard “Alarms” page with guided + advanced rule authoring (Tier A validated installed `0.1.9.263`; Tier B DT-59)
- ✅ SA-10 Implement rule-based conditional alarms engine + APIs (Rust core-server) (Tier A validated installed `0.1.9.263`; Tier B DT-59)
- ✅ SA-9 Fix schedule timezone + block execution semantics (Tier A validated installed 0.1.9.100; Tier B DT-59)
- ✅ Implemented conditional automation based on forecasts/analytics
- ✅ RRULE/weekly block schedules
- ✅ REST endpoints for schedules CRUD
- ✅ Alarm definitions and history
- ✅ MQTT command publishing
- ✅ Demo data and tests
- ✅ Extended schedule models
- ✅ Updated schedule engine
- ✅ Ensured default offline alarms
- ✅ Added pytest coverage

---

## <a name="backups-and-restore"></a>Backups and Restore

### In Progress
- _None_

### Done
- ✅ DW-89 Backups controller settings export/restore (validated installed 0.1.9.69; Tier B DW-99)
- ✅ DW-90 Backups database export (raw/sql/csv/json) (validated installed 0.1.9.69; Tier B DW-99)
- ✅ DW-96 Backups auth-aware downloads + secure raw backup download (validated installed 0.1.9.69; Tier B DW-99)
- ✅ Automated config backups with retention
- ✅ REST endpoints for backups management
- ✅ Dashboard backups dialog
- ✅ Demo dataset with backups
- ✅ Extended backup manager
- ✅ Adoption flow queue restore
- ✅ Restore activity feed

---

## <a name="analytics"></a>Analytics

### Blocked
- ⏸ AN-19 Renogy Rover Modbus hardware validation
- ⏸ AN-29 Validate Forecast.Solar PV overlay on real Renogy node

### In Progress
- [ ] No open items

### To Do
- _None_

### Deferred / Optional
- [ ] AN-20 Emporia ESPHome MQTT bridge validation (optional)
- [ ] AN-24 UniFi Protect ingest (motion + AI thumbnails) (optional)
- [ ] AN-25 UniFi topology ingest (network infra + Pi association by MAC/hostname) (optional)

### Done
- ✅ CS-58 Renogy BT-2 telemetry → analytics (battery SOC + power + derived storage series) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ AN-35 Analytics Overview soil moisture aggregation (validated installed 0.1.9.232; Tier B CS-69)
- ✅ AN-31 Analytics power UX per-node breakdown (no implied coupling) (validated installed 0.1.9.75; Tier B CS-69)
- ✅ AN-30 Analytics power chart bucketing (24h 5-min + 168h hourly; deployed `0.1.9.12`, revalidated `0.1.9.46`)
- ✅ AN-18 Live external feed QA (Emporia cloud ingest validated; auto-poll enabled by default in prod bundle `0.1.9.8`; Tesla/Enphase waiting on credentials)
- ✅ AN-26 Emporia setup UX: accept username/password to derive a cloud token
- ✅ AN-23 Per-node battery voltage chart on Analytics
- ✅ AN-27 Forecast.Solar PV forecast integration (Public plan)
- ✅ AN-28 Hyperlocal weather forecast (Open-Meteo) hourly + weekly
- ✅ AN-32 Setup Center Forecast.Solar PV configurables + check-plane validation (validated installed 0.1.9.69; Tier B CS-69)
- ✅ AN-33 Open-Meteo forecast: add cloud cover (persist + API + graphs) (validated installed 0.1.9.69; Tier B CS-69)
- ✅ AN-34 Hyperlocal current weather endpoint for per-node live panels (validated installed 0.1.9.69; Tier B CS-69)
- ✅ AN-22 Wire reservoir depth telemetry into Analytics Water
- ✅ AN-17 Emporia local ESPHome MQTT bridge ingest
- ✅ AN-21 Setup Center predictive alarms config (LLM endpoint + optional token)
- ✅ AN-16 Predictive alarms: fix seeded demo DB integration + bootstrap/status controls
- ✅ AN-14 Dashboard predictive alarm visualization (TICKET-0002)
- ✅ AN-13 External anomaly detection integration (TICKET-0001 / ADR 0001)
- ✅ AN-12 Predictive alarm schema migration (TICKET-0003)
- ✅ Implement analytics feed scaffolding + demo replay providers
- ✅ Emporia/Tesla/Enphase HTTP polling + fixture-driven contract tests
- ✅ Timescale aggregation jobs
- ✅ REST endpoints for analytics
- ✅ Dashboard analytics components
- ✅ Seeded analytics dataset
- ✅ Computed water usage totals
- ✅ Computed soil moisture
- ✅ Tracked alarm counts
- ✅ Refactored analytics feeds into provider modules with shared manager
- ✅ File-based utility rate schedules persisted for Analytics UI (AN-6)
- ✅ AN-10 Utility provider dispatcher with PGE/ERCOT/NYISO mappers, fixture-driven contract tests, and HTTP/file/fixed fallback surfaced in status
- ✅ AN-15 Predictive alarms scaffold (stubs)

---

## <a name="tsse"></a>Time-Series Similarity Engine (TSSE)

Single-agent execution is REQUIRED for any pending/incomplete TSSE task/ticket; the Collab Harness multi-agent workflow is no longer required for remaining TSSE work.
Recent progress: 2026-01-23 Worker A implemented server-side analysis job schemas + runners for `correlation_matrix_v1`, `event_match_v1`, `cooccurrence_v1`, and `matrix_profile_v1` (bounded params, cancel-aware progress phases, DuckDB-backed results). 2026-01-23 Worker C delivered API/UX design proposals (job-based Related Sensors UX flow, analysis job polling/progress, preview drilldown, and large-series paging/streaming UI plan with decimation/watermarks). 2026-01-23 Worker A reviewed data-plane implementation (lake/replication/DuckDB/chart paging) and documented acceptance gaps + proposed changes. 2026-01-24 Worker A implemented cursor-based paging for `/api/metrics/query` and dashboard paging merge so charts never fail on “series too large” (unit tests added). 2026-01-24 Worker C refactored Trends Relationships/Co-occurrence/Matrix Profile panels to job-based UX (submit/poll/cancel/result) and captured UI contract expectations for analysis job results. 2026-01-24 Worker C aligned dashboard co-occurrence job request params with the backend (`sensor_ids` required, optional `focus_sensor_id`) and removed legacy client-only fields from the payload. 2026-01-24 Worker C fixed `InlineBanner` tone typing (`danger`) so TSSE panels pass `next build` TypeScript checks. 2026-01-24 Worker B implemented runner-level tracing spans + durable phase timing events for TSSE analysis jobs and added a consistent `why_ranked` summary for `event_match_v1` candidates. 2026-01-24 Implemented `tsse_recall_eval` to produce recall@K evidence for ANN candidate generation on curated/synthetic pairs (TSE-0008/0009). 2026-01-24 Worker D added TSSE bench tooling (`tsse_bench_dataset_gen` + `tsse_bench`) and security hardening tests (authz, job caps, preview clamp, path validation), plus a Tier‑A TSSE evidence template. 2026-01-24 Added DuckDB correctness tests for `DuckDbQueryService` (TSSE-24). 2026-01-24 Tier‑A validated installed controller to `0.1.9.212` and closed TSSE‑1 by reviewing captured screenshots and fixing the installed Trends 403 (`analysis.run`) regression via an admin-capabilities backfill migration (run: `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.212.md`). 2026-01-25: Fixed TSSE lag search skipping sharp peaks by switching to bucket-aligned lag evaluation with exact sweep when bounded and adding a regression test (TSSE-26). 2026-01-25: Batched Related Sensors candidate reads and scored batches concurrently to remove sequential DuckDB reads (TSSE-27). 2026-01-25: Added significance filtering (p-value + min overlap) for Related Sensors scoring and correlation matrix outputs (TSSE-28). 2026-01-25: Recompute rolling Pearson sums every 1000 iterations to avoid drift (TSSE-29). 2026-01-25: Added correlation confidence intervals to API responses and surfaced them in Trends UI (TSSE-30). 2026-01-25: Extracted TSSE scoring magic numbers into named constants with rationale (TSSE-31). 2026-02-06: Implemented TSSE-36 local completion pass: added `min_abs_r` effect-size gating and `bucket_aggregation_mode` (`auto|avg|last|sum|min|max`) to correlation/related jobs; auto mode now resolves per-sensor aggregation by type; correlation matrix now surfaces `r` for non-significant cells while status remains `q`-driven; Trends UI now exposes p/q/n/n_eff/m_lag semantics in matrix tooltips and similarity previews; removed dead TSSE placeholders (`tsse/preview.rs`, `tsse/qdrant_client.rs`). Local validation passed: `cargo test --manifest-path apps/core-server-rs/Cargo.toml`, `make ci-web-smoke`, `cargo build --manifest-path apps/core-server-rs/Cargo.toml`, `cd apps/dashboard-web && npm run build`. 2026-02-06: Tier‑A validated on installed controller `0.1.9.250-tsse36-ui-polish` (run: `project_management/runs/RUN-20260206-tier-a-tsse36-0.1.9.250-tsse36-ui-polish.md`), with viewed TSSE semantics screenshot `manual_screenshots_web/tier_a_0.1.9.250-tsse36-ui-polish_20260206c/tsse_relationship_panel_correlation_stats_key.png`; also hardened `tools/rebuild_refresh_installed_controller.py` (upgrade polling + artifact prune + repeat-run speed flags) and `apps/dashboard-web/scripts/web-screenshots.mjs` (current Trends panel selectors + scoped captures), performed one-time external artifact cleanup, then removed stale sibling artifact dirs/files under `/Users/Shared` and reran `cargo test --manifest-path apps/core-server-rs/Cargo.toml` + `make ci-web-smoke` (PASS).

Notes: 2026-01-23: Worker B delivered design proposals for episodic similarity scoring, multi-scale embeddings, Qdrant candidate generation safeguards, and a recall harness to seed TSE-0008/0009/0010/0015/0016/0017. 2026-01-23: Worker B added concrete schema sketches (candidates + why-ranked + preview payloads), Rust module layout, and benchmark hook outlines for TSE-0008/0009/0010/0011/0012/0020. 2026-01-23: Worker B proposed bounded compute strategies for correlation matrix, event matching, co-occurrence, and matrix profile (early-stop + caps + parameter defaults). 2026-01-23: Worker D drafted the ops/packaging plan for Qdrant bundling, launchd integration, permissions, and health strategy (TSE-0007). 2026-01-23: Worker D reviewed TSE-0007/0019/0022, confirmed the current Qdrant bundling/launchd wiring, and logged missing perms/path hardening plus a Tier‑A TSSE validation checklist. 2026-01-23: Worker D drafted a TSSE bench report template under `reports/` and enumerated remaining security test gaps (job caps, preview max window) plus Tier-A evidence expectations.

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Done
- ✅ TSSE-1 Master: Complete TSSE plan + Tier A validation
- ✅ TSSE-2 Requirements + success metrics + design ADR
- ✅ TSSE-3 Analysis Jobs framework (server-side jobs)
- ✅ TSSE-4 Analysis API surface (create/progress/result/preview)
- ✅ TSSE-5 Parquet analysis lake spec (90d hot, shards)
- ✅ TSSE-6 Postgres → Parquet replication (backfill + incremental + compaction)
- ✅ TSSE-7 DuckDB embedded query layer (Rust)
- ✅ TSSE-8 Qdrant local deployment + schema
- ✅ TSSE-9 Feature/embedding pipeline (robust, multi-scale signatures)
- ✅ TSSE-10 Candidate generation (Qdrant + filters + recall safeguards)
- ✅ TSSE-11 Exact episodic similarity scoring (robust + multi-window + lag)
- ✅ TSSE-12 Related Sensors scan job end-to-end (never error)
- ✅ TSSE-13 Preview/episode drilldown endpoints
- ✅ TSSE-14 Related Sensors job UX (dashboard)
- ✅ TSSE-15 Relationships / correlation matrix job
- ✅ TSSE-16 Events/Spikes matching job
- ✅ TSSE-17 Co-occurrence job
- ✅ TSSE-18 Matrix Profile job (scoped + safe)
- ✅ TSSE-19 Remove “series too large” chart failures (paged metrics)
- ✅ TSSE-20 Perf + scale benchmarks (bench suite + report)
- ✅ TSSE-21 Observability + “why ranked” + profiling hooks
- ✅ TSSE-22 NAS readiness (cold partitions)
- ✅ TSSE-23 Security hardening tests (analytics plane authz/abuse limits/path validation)
- ✅ TSSE-24 DuckDB query correctness tests (points + buckets)
- ✅ TSSE-25 Postgres <-> Parquet parity spot-check runbook + ops CLI
- ✅ TSSE-26 Fix lag search missing sharp correlation peaks
- ✅ TSSE-27 Parallelize/batch Related Sensors candidate scoring
- ✅ TSSE-28 Add significance filtering for correlations (p-value + min overlap)
- ✅ TSSE-29 Mitigate rolling Pearson drift in episode extraction
- ✅ TSSE-30 Surface correlation confidence intervals in API + dashboard
- ✅ TSSE-31 Replace scoring magic numbers with named constants
- ✅ TSSE-32 Fix significance UX + Spearman correctness + correlation CI labeling
- ✅ TSSE-33 Centralize correlation inference (shared Rust module)
- ✅ TSSE-36 TSSE stats Phase 3/4/5 — n_eff, lag correction, BH-FDR (matrix + related sensors)

---

## <a name="ios-app"></a>iOS App

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Deferred / Indefinite
- ⏸ IOS-30 BLE provisioning validation (moved to deferred tracker; preserved on `freeze/ios-watch-2026q1`)
- ⏸ IOS-31 End-to-end parity validation (moved to deferred tracker; preserved on `freeze/ios-watch-2026q1`)

### Done
- ✅ IOS-1 Bluetooth provisioning client flow (CoreBluetooth)
- ✅ IOS-20 Analytics/Backups/Settings parity + tests in demo mode
- ✅ IOS-32 Split iOS AppEntry monolith into modules
- ✅ IOS-33 Default iOS/watch clients to production mode after install (no demo injection + no localhost target)
- ✅ IOS-34 Login UX + token persistence for Rust core-server auth
- ✅ IOS-35 Wire watch app to iOS session (base URL + token)
- ✅ Hardened watch companion coverage (API decoding + env/persistence tests) (IOS-22)
- ✅ Wired watchOS targets + embedding (watch app + extension) (IOS-23)
- ✅ Maestro screenshot automation (manual_screenshots_ios; tabs + More destinations + key sheets) (IOS-24)
- ✅ Watch screenshot automation (manual_screenshots_watch; flake-hardened navigation + distinct frames + best-effort export) (IOS-25)
- ✅ Watch interactive controls (Outputs + Alarms) (IOS-26)
- ✅ iOS UI/UX polish (5-tab layout + More menu, interval labels, improved badges) (IOS-27)
- ✅ iOS tests aligned with generated SDK models (IOS-28)
- ✅ Swift 6 actor-isolation warning cleanup (IOS-29)
- ✅ Xcode project setup
- ✅ Connection manager with Bonjour discovery
- ✅ Demo mode with mock data
- ✅ Nodes, Sensors & Outputs, Users, Schedules, and Trends tabs
- ✅ Provisioning tab
- ✅ Persistent connection settings
- ✅ Update docs

---

## <a name="documentation"></a>Documentation

### In Progress
- [ ] No open items

### To Do
- [ ] No open items

### Done
- ✅ DOC-39 Execution stop-gate policy in `AGENTS.md` (no early turn end; explicit de-scope handshake; end-of-turn completion checks)
- ✅ DOC-38 Tier-A clean-worktree discipline (runbook + AGENTS)
- ✅ DOC-37 Tier-A runbook audit summary (steps + evidence checklist)
- ✅ DOC-31 Document auth + capabilities UX
- ✅ DOC-32 Reduce install/uninstall polish footguns
- ✅ DOC-33 Document macOS firewall prompts during dev/E2E
- ✅ DOC-34 Remove obsolete external delegation workflow instructions
- ✅ DOC-28 Replace container stack references with native stack guidance
- ✅ DOC-29 Document repeatable installer E2E on a single Mac
- ✅ Documented local-only test policy and added `make ci` aggregator plus the staged-path pre-commit selector
- ✅ Created a centralized project documentation hub
- ✅ Created a comprehensive guide for setting up a production environment
- ✅ Updated READMEs and other documentation to reflect the current state of the project
- ✅ Production setup runbook usability pass (DOC-25 through DOC-27)
- ✅ Added Emporia cloud API setup guide (DOC-22)
- ✅ Added Emporia deviceGid helper script (DOC-23)
- ✅ Added a repo-wide architecture diagram to the root README (DOC-21)
- ✅ Git hygiene guardrails (no restore without review; runtime configs use templates + gitignore)
- ✅ Removed post-commit simulator requirement from AGENTS
- ✅ Provided curated Codex skills list on request (DOC-7)
- ✅ Required tests for all code changes (DOC-8)
- ✅ Added Sim Lab step-by-step usage runbook (DOC-10)
- ✅ Documented Renogy Rover BT-2 + Pi 5 node assumptions (DOC-11)
- ✅ Added Renogy Pi 5 deployment runbook (DOC-12)
- ✅ Added Raspberry Pi 5 simulator runbook (DOC-13)
- ✅ Added Raspberry Pi 5 deployment tool runbook (DOC-14)

---

## <a name="setup-app-native-services"></a>Setup App & Native Services

**North-Star Outcome:**
- A non-expert can take a brand-new Mac mini, run a single installer, answer a few prompts, and end up with a running system plus a health UI and guided node onboarding.
- No manual edits, no copy/paste commands, and upgrades/backups are one-click (or one command).

**North-Star Acceptance Gate (Setup):**
- Single installer DMG auto-launches the wizard (no Terminal steps).
- Minimal prompts; bundle/farmctl auto-detect; advanced fields hidden by default.
- Launch plan generation shows no warnings on a clean machine (missing binaries are expected before first install).
- Services start at boot with no user logged in (LaunchDaemons/system launchd domain).
- Services run as a least-privilege service user (not root); only install/bootstrap requires admin.
- DB + MQTT + Redis are bundled/provisioned without manual dependency steps.
- Controller bundles are local-path DMGs (no remote bundle downloads).
- Setup Center health/install/upgrade/backup works without manually starting a setup service.
- `make e2e-installer-stack-smoke` validates DMG install/upgrade/rollback/uninstall in a clean temp root.
- `make e2e-installer-stack-smoke-quarantine` validates the same flow with a simulated quarantined-downloaded installer DMG (no manual `xattr`).
- Installer/E2E includes a clean uninstall/reset so repeated installs are safe.

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

### In Progress
- 🚧 SETUP-33 Fix installer DMG Gatekeeper/quarantine (prod)
- 🚧 SETUP-34 Headless Setup Center install/upgrade (prod; no auth prompt)
- 🚧 SETUP-38 Expose controller config + preflight in Setup Center
- Note: controller bundle `0.1.9.26` extends the core-server `/api/setup/controller/runtime-config` endpoint + config-file overrides so Setup Center advanced controller settings work even if the running setup-daemon schema lags (now also includes telemetry-sidecar tuning + Mapillary Street View token support + Forecast.Solar Public quota metadata).
- Note: `farmctl` upgrades now remove an existing `farmctl` binary before copy and treat config saves as best-effort when `config.json` is service-owned (reduces headless upgrade failures).

### To Do
- [ ] SETUP-36 Validate bootstrap admin + session-login UX (prod)
- [ ] SETUP-37 Verify uninstall cleanup repeatability (no orphans/ports)

### Done
- ✅ SETUP-39 Document Tier-A rebuild/refresh runbook (installed controller)
- ✅ SETUP-41 Fix Tier-A dirty-path allowlist parsing for porcelain lines w/ leading space
- ✅ SETUP-35 Pre-create bootstrap admin user (temp password) during production install
- ✅ SETUP-31 Installer launcher reliability (no ERR_CONNECTION_REFUSED)
- ✅ SETUP-32 Enforce “single public installer DMG” in releases
- ✅ SETUP Gate Status (North-Star) (installer UX hardening)
- ✅ SETUP-15 E2E DMG install/upgrade/rollback/uninstall validation
- ✅ SETUP-10 Single installer DMG + auto-launch wizard (native Swift launcher; no AppleScript)
- ✅ SETUP-11 Setup wizard auto-detect + advanced toggle
- ✅ SETUP-12 Managed DB/MQTT/Redis native services (LaunchDaemons + least-privilege user)
- ✅ SETUP-13 Rust setup daemon (replace Python setup app)
- ✅ SETUP-14 One-click backup/upgrade in Setup Center
- ✅ SETUP-16 Add explicit install profiles (prod vs e2e)
- ✅ SETUP-17 Implement farmctl uninstall/reset
- ✅ SETUP-18 Make `farmctl serve` delegate to the canonical installer codepath
- ✅ SETUP-19 Generate non-default credentials for bundled Postgres
- ✅ SETUP-20 Make the MQTT broker reachable to LAN nodes (bind fix)
- ✅ SETUP-21 Prevent/purge launchd override state pollution for E2E installs
-   (Note: historical `com.farmdashboard.e2e.*` launchd override keys can be purged one-time via `sudo python3 tools/purge_launchd_overrides.py --uid $(id -u) --apply --backup`.)
- ✅ SETUP-22 Ship a single public installer artifact (controller DMG embedded in app bundle)
- ✅ SETUP-23 Make `farmctl uninstall` resilient to missing service user
- ✅ SETUP-24 Replace AppleScript installer launcher with native Swift app (no AppleScript)
- ✅ SETUP-25 Fix preflight warning semantics (clean install = no warnings)
- ✅ SETUP-26 Quarantine-safe controller DMG mounting (no manual `xattr`)
- ✅ SETUP-27 Admin prompt only at LaunchDaemons install (no AppleScript)
- ✅ SETUP-28 Make MQTT host first-class in the wizard (auto-detect controller LAN IP)
- ✅ SETUP-29 Make preflight UX “normal” (configure-first + no warn for expected states)
- ✅ SETUP-30 Fix Launch plan warning semantics (clean install = no warnings)

## <a name="architecture-technical-debt"></a>Architecture & Technical Debt

**Status:** In Progress (ARCH-5 pulse fail-closed follow-up; ARCH-6B Tier B validation deferred)

**Description:** Track cross-stack technical debt by cataloguing and actively reducing mixed-scope files, dead/fallback branches, and deceptive stubs that can present as real production features.

### In Progress
- (none)

### To Do
- [ ] ARCH-5 Node-agent GPIO pulse counter fail-closed follow-up (remove production stub mode)
- [ ] ARCH-6B Tier B (clean-host) validation for ARCH-6 pruning pass (deferred indefinitely per user instruction)

### Done
- ✅ ARCH-7 Shrink generated SDK artifacts (drop docs/tests)
- ✅ ARCH-6 Repo-wide pruning pass (deleted legacy/redundant/unused code; Tier A validated; Tier B deferred as ARCH-6B)
- ✅ ARCH-1 Documented the dashboard-web, core-server-rs, and shared helpers that exceed 1,000 lines and mix responsibilities (AnalyticsOverview, MapPageClient, `lib/api.ts`, `forecast.rs`, `analytics.rs`, etc.)
- ✅ ARCH-2 Main-branch integrity pass for dead/fallback/stub code (removed `/api/dashboard/demo`; removed dashboard snapshot reconstruction fallback; validated core/web/farmctl smoke)
- ✅ ARCH-3 Refactor farmctl bundle “god file” into scoped modules (phase 1)
- ✅ ARCH-4 Stub/dead-code audit + CI guardrail baseline (`tools/production_token_guardrail.py`, allowlist, CI wiring; run log: `project_management/runs/RUN-20260206-installed-controller-smoke-main-integrity-cleanup.md`)
