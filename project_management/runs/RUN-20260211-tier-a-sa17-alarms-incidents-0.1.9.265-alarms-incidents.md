# RUN: Tier‑A SA‑17 — Alarms incidents + builder guidance/backtest (installed controller; no DB reset)

Date: 2026-02-11

Operator: FarmDashboard

Goal: Validate SA‑13..SA‑16 alarms/incident UX on the installed controller (no DB/settings reset) and record screenshot-review hard-gate evidence for SA‑17.

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md` (Tier‑A rebuild/refresh SOP)
- `project_management/TASKS.md` (SA‑17)
- `project_management/plans/PLAN-2026-02-11-alarms-operator-grade.md` (plan of record)

---

## Preconditions (hard gates)

- [x] **No DB/settings reset performed** (Tier‑A rule).
- [x] Installed setup daemon is reachable:
  - [x] `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server is reachable:
  - [x] `curl -fsS http://127.0.0.1:8000/healthz`

---

## Build + refresh (Tier‑A)

> Followed `docs/runbooks/controller-rebuild-refresh-tier-a.md`. Recorded exact commands + final PASS/FAIL only.

### Version

- Installed version before:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: current `0.1.9.264-alarms2`
- New version (bundle build):
  - Chosen: `0.1.9.265-alarms-incidents`

### Artifacts

- DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.265-alarms-incidents.dmg`
- Build log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.265-alarms-incidents.log`

### Commands run (append-only)

- `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.265-alarms-incidents --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.265-alarms-incidents.dmg --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.265-alarms-incidents.log`
- `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{\"bundle_path\":\"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.265-alarms-incidents.dmg\"}'`
- `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
- `make e2e-installed-health-smoke`

### Refresh verification

- [x] `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- [x] `make e2e-installed-health-smoke` (PASS)
- Installed version after:
  - Result: current `0.1.9.265-alarms-incidents` (previous `0.1.9.264-alarms2`)

---

## SA‑17 operator-flow evidence (Tier‑A)

> Validation performed using the dashboard UI on the installed controller. No DB reset performed. Temporary test rules created for evidence were deleted after validation to avoid leaving noise.

### Incidents (triage + workflow)

- Verified `/alarms` loads and defaults to **Incidents** tab with search + filters (status/severity/assigned/range).
- Verified incident grouping/rollup behavior:
  - fire → resolve transitions create alarm events and keep the incident grouped by rule+target.
  - close/reopen works and updates the incident list.
- Verified incident controls:
  - assign (unassigned → me)
  - snooze (set and clear)
  - ack-all events in incident
  - notes: create + persist + render with author/timestamp

Note: For notes, used a real DB user via `/api/auth/login` so `incident_notes.created_by` is a UUID (API tokens are not UUID user ids). Credentials/tokens were not written into this repo.

### Rule builder guidance (stats + bands + visualization)

- Verified Create alarm wizard Step 3 **Guidance**:
  - Stats render (n, min/max, mean/median, std dev, MAD, percentiles).
  - Classic ±1/2/3σ and robust bands are shown.
  - Visualization renders time-series preview with band overlays and histogram of values.

### Backtest (historical replay)

- Verified Create alarm wizard Step 4 **Backtest**:
  - Backtest runs successfully for the selected range/aggregation.
  - Summary renders (fired/resolved/time firing) and per-target breakdown.

### Investigation: Related signals + other events

- Verified incident detail shows Context chart around focus event.
- Verified Related signals scan runs controller-wide and supports filters + sorting (Combined / Significance / Proximity).
- Verified Other events surface renders schedule/action-log context within the same time window.

---

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260211_043204/alarms_incidents.png`
- [x] REVIEWED: `manual_screenshots_web/20260211_043204/alarms_rules.png`
- [x] REVIEWED: `manual_screenshots_web/20260211_043204/alarms_incident_detail_context.png`
- [x] REVIEWED: `manual_screenshots_web/20260211_043204/alarms_wizard_guidance.png`
- [x] REVIEWED: `manual_screenshots_web/20260211_043204/alarms_wizard_backtest.png`
- [x] REVIEWED: `manual_screenshots_web/20260211_043204/alarms_wizard_visualization_with_histogram.png`

### Visual checks (required)

- [x] PASS: `alarms_incidents.png` incidents list + filters render; “Open/All” toggles and search layout match dashboard conventions.
- [x] PASS: `alarms_rules.png` rule library and alarm events sections render with consistent card/table styling.
- [x] PASS: `alarms_incident_detail_context.png` incident detail layout is readable; context chart renders with threshold overlay and controls are not clipped.
- [x] PASS: `alarms_wizard_guidance.png` guidance stats table renders with units; classic/robust band ranges visible.
- [x] PASS: `alarms_wizard_visualization_with_histogram.png` time-series preview + histogram render without overlapping labels.

### Findings

- No severe/moderate issues found in reviewed screenshots; minor UX nit: “auto (reco…)” label truncation in narrow wizard header (still functional and readable in context).

### Reviewer declaration

I viewed each screenshot listed above.

---

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260211-tier-a-sa17-alarms-incidents-0.1.9.265-alarms-incidents.md`
- Result: PASS

---

## Evidence summary

- Installed version before: `0.1.9.264-alarms2`
- Installed version after: `0.1.9.265-alarms-incidents`
- Installed health smoke: PASS (`make e2e-installed-health-smoke`)
- Screenshot paths (VIEWED): `manual_screenshots_web/20260211_043204/...` (see hard-gate section)
- Notes / anomalies: none blocking.

