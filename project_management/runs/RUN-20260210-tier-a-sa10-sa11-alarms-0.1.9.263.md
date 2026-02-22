# RUN-20260210 Tier A — SA-10/SA-11/SA-12 (0.1.9.263)

## Context

- **Date:** 2026-02-10 (PST)
- **Tasks:**
  - **SA-10** — Rule-based conditional alarms engine + APIs (Rust core-server)
  - **SA-11** — Dashboard Alarms page + guided/advanced alarm authoring
  - **SA-12** — Tier A validate conditional alarms on installed controller
- **Goal:** Validate conditional alarm rules and the new `/alarms` dashboard workflow on the installed controller with no DB/settings reset.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Rollback target before refresh:
  - `current_version`: `0.1.9.262-dw249-missingness`
  - `bundle_path`: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.262-dw249-missingness.dmg`
- Worktree gate: clean (Tier A hard gate)

## Build + Refresh

- **Version:** `0.1.9.263`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.263.dmg`
- **Bundle log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.263.log`

Final installed status after refresh:

- `current_version`: `0.1.9.263`
- `previous_version`: `0.1.9.262-dw249-missingness`

(From `http://127.0.0.1:8800/api/status`)

## Validation

### Health / platform checks

- `make e2e-installed-health-smoke` → **PASS**
- `farmctl health --json` (installed config) → **all checks ok** (`core_api`, `dashboard`, `mqtt`, `database`, `redis`)
- Post-upgrade health:
  - `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
  - `curl -fsS http://127.0.0.1:8000/healthz` → **ok**

### Alarm-rule runtime scenarios (live installed controller)

Validated against live sensor `5e7be23ca4894114a8c7ca33` (Battery Voltage, ~13.2V):

1. **Threshold condition** (`gt 12.0`) triggered + resolved
   - Triggered alarm: `alarm_id=39`, `target_key=sensor:5e7be23ca4894114a8c7ca33`
   - Resolved after updating condition to non-firing (`gt 100.0`)
2. **Rolling window condition** (`avg over 120s gte 10.0`) triggered + resolved
   - Triggered alarm: `alarm_id=40`, `target_key=sensor:5e7be23ca4894114a8c7ca33`
   - Resolved after updating condition to non-firing (`gt 100.0`)
3. **Consecutive periods condition** (`period=eval`, `count=2`, child `gt 12.0`) triggered + resolved
   - Triggered alarm: `alarm_id=41`, `target_key=sensor:5e7be23ca4894114a8c7ca33`
   - Resolved after updating child condition to non-firing (`gt 100.0`)

Preview API checks on installed controller:

- `POST /api/alarm-rules/preview` with `deviation` condition → `targets_evaluated=1`
- `POST /api/alarm-rules/preview` with `range` condition → `targets_evaluated=1`

History/transition evidence:

- `GET /api/alarms/history?limit=80` includes fired/resolved transitions for alarms `39/40/41` with `rule_id` and `transition` fields populated.

### UI evidence (captured + viewed)

Screenshot directory:

- `manual_screenshots_web/tier_a_0.1.9.263_sa12_alarms_20260210_192137/`

Viewed screenshots:

- `manual_screenshots_web/tier_a_0.1.9.263_sa12_alarms_20260210_192137/alarms_overview.png`
- `manual_screenshots_web/tier_a_0.1.9.263_sa12_alarms_20260210_192137/alarms_wizard_step1.png`
- `manual_screenshots_web/tier_a_0.1.9.263_sa12_alarms_20260210_192137/alarms_wizard_step2.png`
- `manual_screenshots_web/tier_a_0.1.9.263_sa12_alarms_20260210_192137/alarms_wizard_step3.png`

Reviewed outcomes:

- Alarms page renders rule library, active alarms, rule health, and history sections.
- Wizard flow renders all 3 steps (Basics, Condition, Advanced) and condition-specific inputs.
- Navigation integration is visible and consistent with dashboard shell.

## Notes

- A prior interrupted refresh attempt caused overlapping `farmctl upgrade` calls and `hdiutil ... Resource busy`; validation was rerun carefully with a single in-flight upgrade path and completed successfully.
- No rollback was required after the final validated refresh.
