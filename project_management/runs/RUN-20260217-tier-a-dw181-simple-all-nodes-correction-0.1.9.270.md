# RUN-20260217 Tier A — DW-181 correction (Simple mode all-nodes + no refine path) — Installed controller 0.1.9.270

**Date:** 2026-02-17

## Scope

- DW-181 follow-up correction:
  - Simple mode must default to **All nodes** scope.
  - Simple mode must not expose the “Broaden to all nodes” shortcut button.
  - Simple mode must not expose the “Refine (more candidates)” button/path.
  - Simple mode run path must execute completeness-first full scan (no quick-suggest truncation path).

## Preconditions

- [x] No DB/settings reset performed (Tier‑A rule).
- [x] Installed setup daemon health OK: `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server health OK: `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Repo worktree clean before bundling: `git status --porcelain=v1 -b` (empty)

## Build + Upgrade (Installed Controller; NO DB reset)

- Rebuild/refresh command:
  - `python3 tools/rebuild_refresh_installed_controller.py --post-upgrade-health-smoke`
- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.270.dmg`
- Build log:
  - `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.270.log`
- Installed smoke log (post-upgrade smoke):
  - `/Users/Shared/FarmDashboardBuilds/logs/installed-health-smoke-0.1.9.270.log`
- Pre-upgrade installed version:
  - `current_version=0.1.9.269`
- Post-upgrade installed version:
  - `current_version=0.1.9.270` (previous: `0.1.9.269`)

## Validation

- [x] Installed smoke: `make e2e-installed-health-smoke` (PASS)
- [x] Related Sensors targeted local validation:
  - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx tests/relatedSensorsProviderAvailability.test.tsx tests/relatedSensorsWorkflowImprovements.test.tsx tests/relatedSensorsPinnedSemantics.test.tsx` (PASS)
  - `cd apps/dashboard-web && npm run build` (PASS)
- [x] Screenshots captured (installed controller; no core/web start):
  - Command: `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
  - Screenshot folder: `manual_screenshots_web/20260217_120526/`

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260217_120526/trends.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_120526/trends_related_sensors_large_scan.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_120526/trends_related_sensors_scanning.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_120526/trends_cooccurrence.png`

### Visual checks (required)

- [x] PASS: `trends.png` Trends page renders baseline layout without regressions in sensor picker/chart settings cards.
- [x] PASS: `trends_related_sensors_large_scan.png` Simple mode shows `Scope = All nodes` by default and does not show a “Broaden to all nodes” shortcut button.
- [x] PASS: `trends_related_sensors_large_scan.png` Simple mode primary action is only `Find related sensors`; no `Refine (more candidates)` button is present.
- [x] PASS: `trends_related_sensors_scanning.png` Related Sensors scan/results render correctly with full-scope disclosure (`Candidate source: All sensors in scope`) and no layout breakage.
- [x] PASS: `trends_cooccurrence.png` Trends related/co-occurrence area continues to render after the DW-181 correction.

### Findings

- No blocking issues found in reviewed Tier‑A screenshots for the DW-181 correction.
- The previously reported UX regression is resolved on installed controller `0.1.9.270`.

### Reviewer declaration

I viewed each screenshot listed above.

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-dw181-simple-all-nodes-correction-0.1.9.270.md`
- Result: PASS
