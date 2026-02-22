# RUN-20260217 Tier A — DW-181 Related Sensors completeness-first defaults — Installed controller 0.1.9.269

**Date:** 2026-02-17

## Scope

- DW-181: Trends → Related Sensors must search all eligible sensors by default (completeness-first), not a random/truncated subset.

## Preconditions

- [x] No DB/settings reset performed (Tier‑A rule).
- [x] Installed setup daemon health OK: `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server health OK: `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Repo worktree clean before bundling: `git status --porcelain=v1 -b` (empty)

## Build + Upgrade (Installed Controller; NO DB reset)

- Rebuild/refresh command:
  - `python3 tools/rebuild_refresh_installed_controller.py --post-upgrade-health-smoke`
- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.269.dmg`
- Build log:
  - `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.269.log`
- Installed smoke log (from scripted post-upgrade smoke):
  - `/Users/Shared/FarmDashboardBuilds/logs/installed-health-smoke-0.1.9.269.log`
- Pre-upgrade installed version:
  - `current_version=0.1.9.268-arch6-prune`
- Post-upgrade installed version:
  - `current_version=0.1.9.269` (previous: `0.1.9.268-arch6-prune`)

## Validation

- [x] Installed smoke: `make e2e-installed-health-smoke` (PASS)
- [x] Screenshots captured (installed controller; no core/web start):
  - Command: `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
  - Screenshot folder: `manual_screenshots_web/20260217_113553/`

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260217_113553/trends.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_113553/trends_related_sensors_large_scan.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_113553/trends_related_sensors_scanning.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_113553/trends_cooccurrence.png`

### Visual checks (required)

- [x] PASS: `trends.png` Trends base layout renders with sensor picker, chart settings, and chart panel structure intact (no clipping/overlap).
- [x] PASS: `trends_related_sensors_large_scan.png` Related Sensors defaults show full-scope candidate source (`All sensors in scope`) and evaluate-all behavior disclosure in Simple mode.
- [x] PASS: `trends_related_sensors_scanning.png` Related Sensors run state and result cards render without layout breakage while scan is active.
- [x] PASS: `trends_cooccurrence.png` Trends page still renders the co-occurrence panel area and surrounding controls without blocking visual regressions.

### Findings

- No blocking issues found in reviewed Tier‑A screenshots for DW-181.

### Reviewer declaration

I viewed each screenshot listed above.

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-dw181-related-sensors-all-scan-0.1.9.269.md`
- Result: PASS
