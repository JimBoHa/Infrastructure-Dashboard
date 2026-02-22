# RUN-20260126 — Tier A refresh: TSSE stats Phase 1 (0.1.9.214)

Date: 2026-01-26  
Tier: A (installed controller refresh; **no DB/settings reset**)  
Installed host: local controller at `http://127.0.0.1:8000`  

## Goal

Rebuild and refresh the installed controller to pick up the TSSE “Phase 1” refactor (centralize correlation inference helpers) and validate the installed stack still serves healthy UI/API.

## Preconditions (runbook)

Runbook followed: `docs/runbooks/controller-rebuild-refresh-tier-a.md`.

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` (ok)
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` (ok)
- Repo worktree clean (Tier‑A hard gate for `farmctl bundle`): `git status --porcelain=v1 -b` (clean)

## Version + rollback target

- Previous installed version: `0.1.9.213`
- New installed version: `0.1.9.214`
- Rollback target (previous): `0.1.9.213` bundle DMG at `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.213.dmg`

## Build bundle DMG

Stable output path (not `/Volumes/...`):

- DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.214.dmg`
- Bundle build log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.214.log`

Command (per runbook):

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.214 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.214.dmg \
  --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.214.log
```

## Configure setup daemon + upgrade

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.214.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Verify version:

```bash
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

Observed:
- `current_version`: `0.1.9.214`
- `previous_version`: `0.1.9.213`

## Validation

- Health: `curl -fsS http://127.0.0.1:8000/healthz` (ok)
- Installed smoke: `make e2e-installed-health-smoke` (PASS)
- UI screenshots captured via:

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Screenshots saved to:
- `manual_screenshots_web/20260125_175955`

**Viewed gate:** Screenshots must be manually opened and reviewed to satisfy Tier‑A requirements.

