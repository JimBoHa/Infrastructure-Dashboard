# RUN: TSSE Tier‑A validation (installed controller; no DB reset) — 0.1.9.211

Date: 2026-01-24

Operator: Codex CLI (Orchestrator)

Goal: Record Tier‑A evidence for TSSE features on the **installed controller** without DB/settings reset. This run is intended to satisfy `TSSE-1` Tier‑A evidence requirements (with screenshot VIEWED gate pending until images are opened/reviewed).

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md` (Tier‑A rebuild/refresh SOP)
- `project_management/TASKS.md` (TSSE-1 evidence requirements)
- Bundle build log (local): `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.211.log`

---

## Preconditions (hard gates)

- [x] **No DB/settings reset performed** (Tier‑A rule).
- [x] Installed setup daemon is reachable:
  - [x] `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server is reachable:
  - [x] `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Installed Qdrant is reachable:
  - [x] `curl -fsS http://127.0.0.1:6333/healthz`
- [x] Repo worktree is clean for bundle builds (Tier‑A hard gate; `reports/**` may be dirty).

---

## Build + refresh (Tier‑A)

> Follow the SOP in `docs/runbooks/controller-rebuild-refresh-tier-a.md`. Record exact commands + final PASS/FAIL only (no streaming logs).

### Version
- Installed version before:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.210`
- New version (bundle build):
  - Chosen: `0.1.9.211`
- Installed version after:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.211` (`previous_version=0.1.9.210`)

### Commands run (append-only)

- `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.211 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.211.dmg --native-deps /usr/local/farm-dashboard/native` (PASS; DMG created, logged at `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.211.log`)
- `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.211.dmg"}'` (PASS)
- `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` (PASS)
- `make e2e-installed-health-smoke` (PASS; rerun confirmed on 2026-01-24)

### Refresh verification

- [x] `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- [x] Installed version after upgrade recorded above (PASS)

Rollback targets present (local DMGs):
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.210.dmg`
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.209.dmg`
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.208.dmg`

---

## TSSE-specific evidence (Tier‑A)

### 1) Bench report (TSE-0019)

- Report path:
  - `reports/tsse-bench-20260124_083042-0.1.9.211.md`
- Result:
  - [x] PASS

### 2) TSSE UI screenshots (must be captured AND viewed)

Screenshot command (example used):
- `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`

Screenshot evidence (paths; confirm each was opened/viewed):
- [ ] `manual_screenshots_web/tier_a_0.1.9.211_trends_auto_compare_2026-01-24_162819580Z/01_trends_auto_compare_key.png` (VIEWED)
- [ ] `manual_screenshots_web/tier_a_0.1.9.211_trends_relationships_2026-01-24_162820805Z/01_trends_relationships_key.png` (VIEWED)
- [ ] `manual_screenshots_web/tier_a_0.1.9.211_trends_event_match_2026-01-24_162819581Z/02_trends_event_match_preview.png` (VIEWED)
- [ ] `manual_screenshots_web/tier_a_0.1.9.211_trends_cooccurrence_2026-01-24_162819580Z/01_trends_cooccurrence_key.png` (VIEWED)
- [ ] `manual_screenshots_web/tier_a_0.1.9.211_trends_matrix_profile_2026-01-24_162820468Z/01_trends_matrix_profile_key.png` (VIEWED)

Playwright Tier‑A validation (desired desktop Chromium project):
- `npm run test:playwright -- --project=chromium-desktop ...tier-a.spec.ts` (PASS; log: `reports/playwright-tier-a-tsse-20260124_082818-0.1.9.211.log`)

### 3) API sanity (auth + caps)

- [x] Confirm analysis endpoints are auth-gated (no token / missing caps should 403):
  - [x] `POST /api/analysis/jobs` requires `analysis.run`
  - [x] `POST /api/analysis/preview` requires `analysis.view`

---

## Evidence summary (copy/paste into TSSE-1 run log)

- Installed version before: `0.1.9.210`
- Installed version after: `0.1.9.211`
- Installed health smoke: PASS
- Bench report path: `reports/tsse-bench-20260124_083042-0.1.9.211.md`
- Screenshot path(s) (VIEWED): (pending)
- Notes / anomalies:
  - Screenshot files were captured under `manual_screenshots_web/` but must be opened/reviewed to satisfy the TSSE-1 VIEWED gate.

