# RUN: TSSE Tier‑A validation (installed controller; no DB reset) — 0.1.9.212

Date: 2026-01-24

Operator: Codex CLI (Orchestrator)

Goal: Fix installed **Trends** scans failing with `403 Missing capabilities: analysis.run`, and close the TSSE‑1 Tier‑A “screenshots VIEWED” hard gate on the installed controller (**no DB/settings reset**).

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md` (Tier‑A rebuild/refresh SOP)
- `project_management/TASKS.md` (TSSE‑1 evidence requirements)
- Prior Tier‑A TSSE run: `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.211.md` (screenshots captured; VIEWED gate remained)

---

## Preconditions (hard gates)

- [x] **No DB/settings reset performed** (Tier‑A rule).
- [x] Installed setup daemon is reachable:
  - [x] `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server is reachable:
  - [x] `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Installed Qdrant is reachable:
  - [x] `curl -fsS http://127.0.0.1:6333/healthz`
- [x] Repo worktree was clean for bundle build (Tier‑A hard gate; `reports/**` allowed):
  - [x] `git status --porcelain=v1 -b`

---

## Build + refresh (Tier‑A)

### Version
- Installed version before:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.211`
- New version (bundle build):
  - Chosen: `0.1.9.212`
- Installed version after:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.212` (`previous_version=0.1.9.211`)

### Commands run (append-only)

- `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.212 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.212.dmg --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.212.log` (PASS)
- `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.212.dmg"}'` (PASS)
- `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` (PASS)
- `make e2e-installed-health-smoke` (PASS)

### Refresh verification
- [x] `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- [x] Installed version after upgrade recorded above (PASS)

Rollback targets present (local DMGs):
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.211.dmg`
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.210.dmg`

---

## Fix verification: Trends scans no longer 403

### Root cause
Existing `admin` users created before TSSE authz/capabilities landed were missing `analysis.view` / `analysis.run`, causing:
- `POST /api/analysis/jobs` → `403 Missing capabilities: analysis.run`

### Fix shipped
Added an idempotent SQL migration to backfill missing analysis capabilities for `admin` users:
- `infra/migrations/033_admin_analysis_capabilities.sql`

### Evidence

1) Post-upgrade DB verification (psql bundled with the installed controller):
- Command:
  - `/usr/local/farm-dashboard/releases/0.1.9.212/native/postgres/bin/psql \"postgresql://postgres:***@127.0.0.1:5432/iot\" -c \"select email,role,capabilities from users order by email;\"`
- Result:
  - `admin@example.com` capabilities include both `analysis.view` and `analysis.run` (and all other existing caps were preserved).

2) API verification (real login token; not an API token):
- `POST /api/auth/login` as `admin@example.com` (password omitted here) succeeds.
- `GET /api/auth/me` shows `analysis.view` + `analysis.run`.
- `POST /api/analysis/jobs` for `related_sensors_v1` using **Reservoir Depth** (`sensor_id=ea5745e00cb0227e046f6b88`) returns `200` and job reaches `completed` (no 403).

---

## TSSE UI screenshots (captured + VIEWED)

Screenshot command:
- `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt --out-dir=manual_screenshots_web/tier_a_0.1.9.212_tsse_20260124_183258Z`

Evidence (opened and visually reviewed):
- [x] `manual_screenshots_web/tier_a_0.1.9.212_tsse_20260124_183258Z/trends_related_sensors_scanning.png` (VIEWED)
  - Verified: Related Sensors panel renders; no `403 Missing capabilities: analysis.run` banner/toast; “Run analysis” CTA is present.
- [x] `manual_screenshots_web/tier_a_0.1.9.212_tsse_20260124_183258Z/trends_related_sensors_large_scan.png` (VIEWED)
  - Verified: panel renders without authz errors under a wide range configuration.

---

## Evidence summary (TSSE‑1)

- Installed version before: `0.1.9.211`
- Installed version after: `0.1.9.212`
- Installed health smoke: PASS (`make e2e-installed-health-smoke`)
- Screenshot path(s) (VIEWED):
  - `manual_screenshots_web/tier_a_0.1.9.212_tsse_20260124_183258Z/trends_related_sensors_scanning.png`
  - `manual_screenshots_web/tier_a_0.1.9.212_tsse_20260124_183258Z/trends_related_sensors_large_scan.png`
- Notes / anomalies:
  - This run intentionally focused on the production bug (admin caps backfill) + VIEWED screenshot gate; TSSE bench/recall/parity evidence remains recorded in the prior TSSE Tier‑A run artifacts referenced above.

