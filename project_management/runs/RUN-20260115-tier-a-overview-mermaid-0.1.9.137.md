# RUN-20260115-tier-a-overview-mermaid-0.1.9.137

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.137
- **Purpose:** Validate Overview “Where things live” Mermaid diagram renders with correct arrows (no filled/wedged edges).

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Build + Upgrade
1. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.137 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.137.dmg --native-deps build/native-deps`
2. Upgrade installed controller:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.137.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
3. Confirm installed version:
   - `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
Run Playwright to capture `/overview` screenshot and assert Mermaid edges render with `fill: none`:

- `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_TIER_A_VERSION=0.1.9.137 npx playwright test playwright/overview-mermaid-render.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshots (viewed):** `manual_screenshots_web/tier_a_0.1.9.137_overview_mermaid_2026-01-15_102528102Z/`
  - `01_overview_mermaid.png`

## Result
- **PASS**

