# RUN-20260124: TSSE Tier‑A validation (installed controller; no DB reset) — 0.1.9.206

Date: 2026-01-24

Operator: Codex (Orchestrator)

Goal: Record Tier‑A evidence for TSSE features on the **installed controller** without DB/settings reset. This run is intended to satisfy `TSSE-1` Tier‑A evidence requirements.

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md` (Tier‑A rebuild/refresh SOP)
- `project_management/TASKS.md` (TSSE-1 evidence requirements)

---

## Preconditions (hard gates)

- [x] **No DB/settings reset performed** (Tier‑A rule).
- [x] Installed setup daemon is reachable:
  - [x] `curl -fsS http://127.0.0.1:8800/healthz` → `{"status":"ok"}`
- [x] Installed core server is reachable:
  - [x] `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
- [x] Repo worktree is clean (Tier‑A hard gate for bundle build; `reports/**` may be dirty):
  - [x] `git status --porcelain=v1 -b` → only untracked `reports/**` artifacts

---

## Build + refresh (Tier‑A)

> Followed the SOP in `docs/runbooks/controller-rebuild-refresh-tier-a.md`. Recording exact commands + final PASS/FAIL only (no streaming logs).

### Version
- Installed version before:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.205` `previous_version=0.1.9.204`
- New version (bundle build):
  - Chosen: `0.1.9.206`

### Commands run (append-only)

- `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.206 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.206.dmg --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.206.log` → PASS (DMG created)
- `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.206.dmg"}'` → PASS
- `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` → PASS (`Upgraded to 0.1.9.206`)
- `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'` → PASS (`current_version=0.1.9.206` `previous_version=0.1.9.205`)
- `make e2e-installed-health-smoke` → PASS

### Refresh verification

- [x] `curl -fsS http://127.0.0.1:8000/healthz` → PASS
- [x] Installed version after upgrade recorded above → PASS

Rollback target ready (in case of failure):
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.205.dmg`

---

## TSSE-specific evidence (Tier‑A)

### 1) Bench report (TSE-0019)

Bench run completed (required for TSSE-1).

- Bench command:
  - `cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_bench -- --base-url=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt --focus-sensor-id=4b1e3dde7a4297de78d51a50 --runs=7 --interval-seconds=60 --candidate-limit=150 --min-pool=150 --report reports/tsse-bench-20260124_050332-0.1.9.206.md`
- Report path:
  - `reports/tsse-bench-20260124_050332-0.1.9.206.md`
- Result:
  - [x] PASS
  - [ ] FAIL (attach short failure summary + next action)

### 2) TSSE UI screenshots (must be captured AND viewed)

> Tier‑A UI evidence requires at least one screenshot that was opened and visually reviewed. Store under `manual_screenshots_web/` and reference the path(s) here.

- Screenshot command:
  - `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
- Output directory:
  - `manual_screenshots_web/20260124_045442/`
- Screenshot evidence (paths; confirm each was opened/viewed):
  - [ ] `manual_screenshots_web/20260124_045442/trends_related_sensors_large_scan.png` (VIEWED)
  - [ ] `manual_screenshots_web/20260124_045442/trends_cooccurrence.png` (VIEWED)

### 3) API sanity (auth + caps)

- [x] Confirm analysis endpoints are auth-gated (no token should 401/403):
  - [x] `POST /api/analysis/jobs` → `401` (no Authorization header)
  - [x] `POST /api/analysis/preview` → `401` (no Authorization header)

---

## Evidence summary (copy/paste into TSSE-1 run log)

- Installed version before: `0.1.9.205`
- Installed version after: `0.1.9.206`
- Installed health smoke: PASS
- Bench report path: `reports/tsse-bench-20260124_050332-0.1.9.206.md`
- Screenshot path(s) (VIEWED): (captured; pending view confirmation)
- Notes / anomalies: none so far
