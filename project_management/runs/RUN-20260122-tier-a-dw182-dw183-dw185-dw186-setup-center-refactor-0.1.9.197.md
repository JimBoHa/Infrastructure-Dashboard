# RUN-20260122 Tier A — DW-182/DW-183/DW-185/DW-186 (Setup Center refactor) — 0.1.9.197

## Goal

Validate the Setup Center refactor (split `SetupPageClient` into section components + shared setup-daemon API/parsers/validation) on the **installed controller** without resetting DB/settings (Tier A).

Tickets:
- DW-182 (setup-daemon API consolidation)
- DW-183 (validation helpers)
- DW-185 (section extraction)
- DW-186 (Integrations capability guard)

## Host / Preconditions (Tier A)

- Setup daemon: `curl -fsS http://127.0.0.1:8800/healthz`
- Core server: `curl -fsS http://127.0.0.1:8000/healthz`

## Repo state (required Tier-A gate)

- Repo: `/Users/FarmDashboard/farm_dashboard`
- Commit: `ddd7425`
- Worktree: clean (`git status --porcelain=v1 -b`)

## Installed version

- Before: `0.1.9.196`
- After: `0.1.9.197`

Verify:
- `curl -fsS http://127.0.0.1:8800/api/status` → `current_version` / `previous_version`

## Build controller bundle DMG

Output paths:
- DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.197.dmg`
- Log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.197.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.197 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.197.dmg \
  --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.197.log
```

## Configure setup daemon bundle path

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.197.dmg"}'
```

## Upgrade (refresh installed controller)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

## Validation (Tier A)

- Installed smoke:
  - `make e2e-installed-health-smoke` (PASS)

- UI screenshots captured:
  - Command:
    - `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
  - Output dir:
    - `manual_screenshots_web/20260121_210307/`
  - Setup Center screenshot:
    - `manual_screenshots_web/20260121_210307/setup.png`

**Note:** Tier‑A policy requires screenshots to be **captured and viewed**; open at least `setup.png` in Finder/Preview to satisfy the “viewed” requirement.

