# RUN-20260217 Tier A — DW-181 evaluate-all-eligible correction — Installed controller 0.1.9.271

**Date:** 2026-02-17

## Scope

- Fix remaining DW-181 completeness regression reported in production UX:
  - Related Sensors Simple mode must evaluate **all eligible sensors**, not a reduced subset (for example 75/367).
  - Co-occurrence + event paths must both support full eligible pool sizing for evaluate-all runs.

## Preconditions

- [x] No DB/settings reset performed (Tier‑A rule).
- [x] Installed setup daemon health OK: `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server health OK: `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Repo worktree clean before bundle build: `git status --porcelain=v1 -b` (clean)

## Build + Upgrade (Installed Controller; NO DB reset)

- Rebuild/refresh attempt:
  - `python3 tools/rebuild_refresh_installed_controller.py --post-upgrade-health-smoke`
- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.271.dmg`
- Build log:
  - `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.271.log`
- Upgrade follow-up:
  - Initial upgrade attempt hit mounted-DMG contention (`hdiutil ... Resource busy`) while polling.
  - Resolved by detaching stale mounted image and re-triggering upgrade via setup-daemon.
- Pre-upgrade installed version:
  - `current_version=0.1.9.270`
- Post-upgrade installed version:
  - `current_version=0.1.9.271` (previous: `0.1.9.270`)

## Validation

- [x] Local targeted backend validation:
  - `cargo test --manifest-path apps/core-server-rs/Cargo.toml related_sensors_unified_v2` (PASS)
- [x] Installed smoke:
  - `make e2e-installed-health-smoke` (PASS)
  - Log: `/Users/Shared/FarmDashboardBuilds/logs/installed-health-smoke-0.1.9.271.log`
- [x] Screenshots captured (installed controller; no local core/web start):
  - `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
  - Screenshot folder: `manual_screenshots_web/20260217_124653/`

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260217_124653/trends.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_124653/trends_related_sensors_large_scan.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_124653/trends_related_sensors_scanning.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_124653/trends_cooccurrence.png`

### Visual checks (required)

- [x] PASS: `trends_related_sensors_large_scan.png` shows Simple mode with default `Scope = All nodes`.
- [x] PASS: `trends_related_sensors_large_scan.png` now reports full completeness: `Evaluated: 367 of 367 eligible sensors (limit: 367)`.
- [x] PASS: `trends_related_sensors_scanning.png`/result flow renders without layout regressions after evaluate-all backend correction.
- [x] PASS: `trends_cooccurrence.png` still renders with the expanded candidate pool path.

### Findings

- The reported regression is fixed on installed controller `0.1.9.271`: evaluate-all runs now evaluate the full eligible pool in the observed case.

### Reviewer declaration

I viewed each screenshot listed above.

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-dw181-evaluate-all-eligible-0.1.9.271.md`
- Result: PASS
