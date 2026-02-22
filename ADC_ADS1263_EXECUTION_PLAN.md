## ADS1263 ADC HAT + Reservoir Depth — Execution Plan

Here’s a pragmatic, **no-wasted-time** path to finish ADS1263 + depth sensor end‑to‑end, with **hard safety gating** (no simulators in production), and with **review + tests + commit + rebuild/refresh installed app after each phase**. Each phase is sized for ~1–2 days of focused work.

---

## Hard constraints
- No DB/settings reset (Tier A runs on installed controller only).
- No simulator/stub can be enabled in production artifacts (must be build-flavor gated and fail closed).
- Tier‑A evidence includes at least one captured + viewed screenshot (store under `manual_screenshots_web/`).

---

## Phase handoff model (self-contained work packets)
Each phase below is intended to be executable as a standalone “work packet” that can be handed off to a developer/team. Every phase should include:
- **Start (PM):** update `project_management/TASKS.md` + `project_management/BOARD.md` + `project_management/EPICS.md` to reflect the phase scope and acceptance criteria (so if work pauses, PM docs remain authoritative).
- **Implement:** make the code changes for the phase only.
- **Validate:** run targeted tests and Tier‑A smoke checks.
- **Closeout:** commit + push, rebuild/refresh the installed app, capture + view Tier‑A screenshots, then update PM docs with short evidence and correct Tier‑B deferrals.

---

## Local reference implementation (tank-control on this machine)
Use this repo as the “known working on Pi” reference for ADS1263 patterns and test mocking:
- `/Users/FarmDashboard/tank-control/app/adc_hardware.py` (gpiozero + spidev wiring pattern; DRDY/RST/CS defaults; SPI0)
- `/Users/FarmDashboard/tank-control/app/ads1263.py` (ADS1263 register/command flow; single-ended vs differential)
- `/Users/FarmDashboard/tank-control/app/sensors/backends/adc.py` (example ADC sensor conversion backend patterns)
- `/Users/FarmDashboard/tank-control/tests/conftest.py` (test strategy: mock `spidev` via `sys.modules`, avoid shipping “fake ADC values” in production)
- Optional runbook/templates: `/Users/FarmDashboard/tank-control/docs/hardware.md`, `/Users/FarmDashboard/tank-control/verify_hardware.sh`

---

## Phase 0 — Stabilize work + split into phase commits (P0)
**Goal:** Avoid rework by turning the current “big diff” into clean, reviewable phase commits.

**Deliverables**
- **Start (PM):** clean up PM docs to reflect this plan as the active path forward (so `project_management/TASKS.md` remains authoritative if work pauses mid-phase).
- Create a safety **checkpoint commit** on a WIP branch (captures all current changes as-is).
- Split the checkpoint into **PH1/PH2/PH3…** commits (phase-prefixed commit messages), so each phase is reviewable and reproducible.
- Enforce a hard gate moving forward: **no rebuild/refresh from uncommitted changes**.
- ADC doc hygiene gate (avoid dev confusion during implementation):
  - Select canonical ADC docs:
    - `docs/development/analog-sensors-contract.md`
    - `docs/runbooks/reservoir-depth-pressure-transducer.md`
    - `docs/ADRs/0005-pi5-gpiozero-lgpio-and-fail-closed-analog.md`
  - For any legacy/conflicting ADC docs, add a top-level **Deprecated** banner pointing to the canonical docs, or remove them if stale.
  - Ensure all ADC docs explicitly state: “Production is fail‑closed; simulation is test/dev only and cannot be enabled via the dashboard.”

**Validation**
- Each phase commit is pushed and the working tree is clean before Tier‑A evidence is recorded.

---

## Phase 1 — Safety baseline: “No simulation in production” (P0)
**Goal:** Make it *impossible* for production artifacts to emit plausible analog values unless ADS1263 hardware is actually healthy.

**Start (PM)**
- Update PM docs to put Phase 1 in progress, with acceptance criteria emphasizing build-flavor gating and fail-closed behavior.

**Deliverables**
- Add a **build flavor** concept that is **baked into artifacts**, not a runtime toggle:
  - Node-agent has `BUILD_FLAVOR = "prod" | "dev" | "test"` as a **generated constant** during packaging (e.g., `app/build_info.py` written by the build step).
- Enforce **fail‑closed** for analog in prod:
  - If ADS1263 backend is not enabled/healthy → publish **no analog telemetry** (sensor offline/unavailable).
- Simulation remains allowed only for **tests/dev**:
  - Tests mock `spidev`/`gpiozero` (tank-control style).
  - No UI/config path can enable simulation.
  - In `BUILD_FLAVOR="prod"`, any attempt to use a simulator path hard-fails.

**Validation**
- Run node-agent unit tests with `BUILD_FLAVOR=prod` to ensure:
  - no simulated values ever appear,
  - analog is offline when backend is unhealthy.

**Commit + Rebuild/Refresh**
- Commit: “P0: build flavor + fail‑closed analog”
- Rebuild/refresh installed controller app (Tier A smoke only; no resets).

**Closeout (PM)**
- Update PM docs with Tier‑A evidence and any Tier‑B deferrals.

---

## Phase 2 — Remove “ADS1115” as a concept (P0)
**Goal:** Nobody can create or configure an “ADS1115” sensor ever again. This removes confusion permanently.

**Start (PM)**
- Update PM docs to put Phase 2 in progress, explicitly stating ADS1115 is deprecated/removed and not user-configurable.

**Deliverables**
- Replace any user-facing or config-facing “ads1115” with:
  - `driver_type = "analog"` and backend = `ads1263`.
- Delete/rename “ads1115 driver” code and presets.
- Optional: keep a *tiny* read-only alias mapper (`ads1115` → `analog`) **only** to avoid crashing if stale records exist. No more than that; no new writes of `ads1115`.

**Validation**
- API rejects creation/update with driver_type `ads1115`.
- UI never shows “ADS1115”.
- Repo grep confirms `ads1115` only appears in docs/history/legacy notes (or nowhere except a legacy mapper).

**Commit + Rebuild/Refresh**
- Commit: “Remove ADS1115 naming; analog=ADS1263 only”
- Rebuild/refresh installed app.

**Closeout (PM)**
- Update PM docs with Tier‑A evidence and any Tier‑B deferrals.

---

## Phase 3 — ADS1263 node-agent hardware backend (Pi5) (P0/P1)
**Goal:** ADS1263 works on Pi5 using **gpiozero + spidev**, with a deterministic health check.

**Start (PM)**
- Update PM docs to put Phase 3 in progress, with acceptance criteria around: chip ID/DRDY health, clear error surface, and fail-closed behavior.

**Deliverables**
- Implement ADS1263 backend matching tank-control approach:
  - `spidev` SPI0 (bus 0, dev 0), correct mode + speed
  - `gpiozero` for DRDY + RST/CS (and explicit lgpio pin factory if needed)
  - Read flow gated by DRDY; robust timeout + error surface
- Add **hardware health**:
  - chip id read
  - DRDY sanity
  - sample conversion sanity
- Ensure backend publishes:
  - `analog_backend` and `analog_health` in node status (MQTT path)
  - include last error string for debugging without SSH

**Validation**
- On Node1: confirm `/dev/spidev0.0` exists and ADS1263 chip ID reads successfully.
- Node1 status shows `analog_health.ok = true`.

**Commit + Deploy + Rebuild/Refresh**
- Commit: “Pi5 ADS1263 backend + health”
- Deploy updated node-agent to Node1 (no reinstall; just update + restart service)
- Rebuild/refresh installed controller app.

**Closeout (PM)**
- Update PM docs with Tier‑A evidence and any Tier‑B deferrals.

---

## Phase 4 — End-to-end “Add hardware sensor” from dashboard (Pi-only) (P0)
**Goal:** A hardware engineer can add a real sensor from the dashboard and see real values; **no file copying**.

**Start (PM)**
- Update PM docs to put Phase 4 in progress, with acceptance criteria for the UI flow and API apply semantics.

**Deliverables**
- Core-server:
  - Endpoint to apply node sensor config to node-agent (Pi nodes only)
  - Upsert core sensor registry so telemetry ingest accepts new sensors immediately
  - Persist desired config + applied status
  - OpenAPI always updated (no drift)
- Dashboard:
  - “Add sensor” flow only enabled for **Pi nodes**
  - Shows backend health prominently (ads1263 OK / SPI disabled / not detected)
  - Apply workflow: edit → validate → apply → readback/verify status
  - Clear messaging: “No data until ADS1263 healthy”

**Validation (Tier A)**
- UI flow: add sensor on Pi Node1, apply, see it listed and updating.
- API checks: node sensor config GET/PUT; `/api/sensors` shows `latest_value/latest_ts` updating.
- Playwright screenshots captured **and viewed**.

**Commit + Rebuild/Refresh**
- Commit: “Pi sensor config push (ADS1263) end-to-end”
- Rebuild/refresh installed app.

**Closeout (PM)**
- Update PM docs with Tier‑A evidence and any Tier‑B deferrals.

---

## Phase 5 — Reservoir depth transducer (AIN0 vs AINCOM + 163Ω shunt) (P0)
**Goal:** The “Reservoir Depth” sensor is real, correct, and trustworthy.

**Start (PM)**
- Update PM docs to put Phase 5 in progress, with acceptance criteria around calibration bounds, fault modeling, and Tier‑A verification on Node1.

**Deliverables**
- Add a dedicated preset (or finalized config) for reservoir depth:
  - current-loop conversion (voltage across shunt → current → depth)
  - explicit ranges + fault handling (open loop, short, out-of-range)
- Document the contract + wiring:
  - AIN0 vs AINCOM for this install
  - expected voltage/current bounds (sanity checks)
- Tier‑A evidence:
  - node config applied
  - sensor trend shows plausible readings
  - screenshots captured + viewed

**Validation**
- Verify with Node1 hardware values and expected bounds.
- Confirm data is not simulated (backend reports ADS1263 healthy).

**Commit + Rebuild/Refresh**
- Commit: “Reservoir depth sensor verified on Node1”
- Rebuild/refresh installed app.

**Closeout (PM)**
- Update PM docs with Tier‑A evidence and any Tier‑B deferrals.

---

## Phase 6 (Deferred) — Purge legacy/simulated data from real DB (optional follow-on)
**Goal:** Cleanup only after everything is stable; do not mix with hardware bring-up.

**Deliverables**
- One-time migration or admin tool to remove/mark simulated sensors/data.
- Guardrails to avoid deleting real telemetry.
