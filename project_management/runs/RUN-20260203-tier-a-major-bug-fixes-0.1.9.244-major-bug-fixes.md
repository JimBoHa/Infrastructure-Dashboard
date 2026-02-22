# RUN: Tier‑A validation — Major bug fixes (installed controller; no DB reset)

Date: 2026-02-03  
Operator: Codex

Scope:
- Core auth gaps: CS-90/CS-91/CS-94/CS-95/CS-96/CS-97
- Core functional correctness: CS-92/CS-93/CS-98
- Node-agent reliability/security: NA-66/NA-67/NA-68

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md`
- `project_management/TASKS.md`
- `project_management/feedback/2026-02-03_major-bug-audit-undocumented.md`

---

## Preconditions (hard gates)

- ✅ No DB/settings reset performed (Tier‑A rule).
- ✅ Setup daemon reachable:
  - `curl -fsS http://127.0.0.1:8800/healthz` → `{"status":"ok"}`
- ✅ Core server reachable:
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
- ✅ Repo worktree clean (Tier‑A bundle build hard gate):
  - `git status --porcelain=v1 -b` → `## main...origin/main`
  - `git diff --stat` → (empty)

---

## Build + refresh (Tier‑A)

### Version

- Installed version before:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - `current_version`: `0.1.9.243-dw215-sensor-picker-overflow`
  - `previous_version`: `0.1.9.242-dw210-related-selection`
- New version built + applied:
  - `0.1.9.244-major-bug-fixes`

### Commands run

- Build controller bundle (stable path):
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.244-major-bug-fixes --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.244-major-bug-fixes.dmg --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.244-major-bug-fixes.log`
- Point setup daemon at the bundle:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.244-major-bug-fixes.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Verify installed version:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - `current_version`: `0.1.9.244-major-bug-fixes`
  - `previous_version`: `0.1.9.243-dw215-sensor-picker-overflow`

### Health verification

- ✅ `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- ✅ Installed smoke:
  - `make e2e-installed-health-smoke` → PASS

---

## Evidence (Tier‑A)

### Auth gating sanity (no token → 401)

On installed controller (`http://127.0.0.1:8000`):
- ✅ `GET /api/dashboard/state` → `401`
- ✅ `GET /api/metrics/query` → `401`
- ✅ `POST /api/metrics/ingest` → `401`
- ✅ `GET /api/backups` → `401`
- ✅ `GET /api/backups/retention` → `401`
- ✅ `GET /api/restores/recent` → `401`
- ✅ `GET /api/setup/credentials` → `401`
- ✅ `GET /api/nodes` → `401`
- ✅ `POST /api/users` → `401`

With bearer token:
- ✅ `GET /api/dashboard/state` → `200`
- ✅ `GET /api/nodes` → `200`
- ✅ `GET /api/backups` → `200`
- ✅ `GET /api/backups/retention` → `200`
- ✅ `GET /api/restores/recent` → `200`
- ✅ `GET /api/setup/credentials` → `200`
- ✅ `GET /api/metrics/query` → `200`
- ✅ `POST /api/metrics/ingest` with empty payload → `400` (expected)

### Backups workflow sanity

- ✅ Triggered backups run:
  - `POST /api/backups/run` (auth) → `{"status":"ok","reason":null}`
- ✅ Verified backups list shows a newly created backup entry for `2026-02-03` (auth):
  - `GET /api/backups` → includes `.../2026-02-03.json` entries.

### UI evidence (captured AND viewed)

- Screenshot run:
  - `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
- Evidence path (VIEWED):
  - `manual_screenshots_web/20260203_032120/backups.png` (VIEWED)
  - `manual_screenshots_web/20260203_032120/setup.png` (VIEWED)

Notes:
- The screenshot harness emitted non-fatal warnings for some Trends “related sensors” setup steps (UI locator timeouts). The run still completed and produced screenshots.

---

## Result

- ✅ Tier‑A rebuild/refresh succeeded (installed controller upgraded to `0.1.9.244-major-bug-fixes`).
- ✅ Tier‑A health smoke passed.
- ✅ Auth gating verified on installed stack for the core surfaces listed above.
- ✅ UI screenshots captured and viewed.

