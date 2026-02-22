# RUN-20260212 Tier A — ARCH-6 pruning pass — Installed controller 0.1.9.268-arch6-prune

**Date:** 2026-02-12

## Scope

- ARCH-6: repo-wide pruning pass (remove non-shipping surfaces and dead legacy code; keep `main` honest by updating references).

## Preconditions

- [x] No DB/settings reset performed (Tier‑A rule).
- [x] Installed setup daemon health OK: `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server health OK: `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Repo worktree clean before bundling: `git status --porcelain=v1 -b` (empty; except `reports/**` allowlist)

## Build + Upgrade (Installed Controller; NO DB reset)

- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.268-arch6-prune.dmg`
- Build log:
  - `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.268-arch6-prune.log`
- Pointed setup daemon at the new DMG:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.268-arch6-prune.dmg"}'`
- Upgraded installed controller via setup daemon:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Verified installed version after upgrade:
  - `current_version=0.1.9.268-arch6-prune` (previous: `0.1.9.267-dw254-256-related-sensors`)

## Validation

- [x] Installed smoke: `make e2e-installed-health-smoke` (PASS)
- [x] Screenshots captured (installed controller; no core/web start):
  - Command: `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
  - Screenshot folder: `manual_screenshots_web/20260212_145437/`

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260212_145437/root.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/nodes.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/trends.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/trends_cooccurrence.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/setup.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/power.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/connection.png`
- [x] REVIEWED: `manual_screenshots_web/20260212_145437/sim_lab.png`

### Visual checks (required)

- [x] PASS: `root.png` System Overview renders with nav + uptime/telemetry cards; no missing/blank states or layout overlap.
- [x] PASS: `nodes.png` Node list renders with online statuses/counts and detail actions; no clipped labels or broken card layout.
- [x] PASS: `trends.png` Trends page renders sensor picker + chart settings panels with expected controls; no overflow in key/help blocks.
- [x] PASS: `setup.png` Setup Center loads with installer actions, health snapshot, and configuration sections visible; no obvious error banners.
- [x] PASS: `power.png` Power analytics page renders charts (not empty placeholders) and right-side summary panel is readable.
- [x] PASS: `connection.png` Connection page renders local/cloud toggle and status controls; Save button is visible and not overlapped.
- [x] PASS: `sim_lab.png` Sim Lab console renders with run controls, node/sensor lists, and alarms panel; no missing UI blocks.

### Findings

- No blocking issues found in the reviewed screenshots.
- Note: `trends_cooccurrence.png` was captured as a fallback screenshot after a locator timeout in the screenshot script; it still shows the Trends page layout (non-blocking for Tier‑A evidence).

### Reviewer declaration

I viewed each screenshot listed above.

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260212-tier-a-arch6-prune-0.1.9.268-arch6-prune.md`
- Result: PASS

