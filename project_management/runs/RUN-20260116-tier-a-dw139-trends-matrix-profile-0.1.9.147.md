# RUN-20260116 — Tier A — DW-139 Trends Matrix Profile Explorer (0.1.9.147)

Tier A validation on the installed controller after adding a data-science-driven Trends visualization: a Matrix Profile explorer for motif discovery, anomaly windows, and a self-similarity heatmap.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.147.dmg`
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.147 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.147.dmg --native-deps build/native-deps`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.147.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health check:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.147`)

## Evidence (Tier A)

- Playwright screenshot run:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.147 npx playwright test playwright/trends-matrix-profile-tier-a.spec.ts --project=chromium-mobile`
- Screenshot captured + viewed:
  - `manual_screenshots_web/tier_a_0.1.9.147_trends_matrix_profile_2026-01-16_080807419Z/01_trends_matrix_profile.png`

## Notes

- The screenshot switches to the “Self-similarity” view so the heatmap is visible in the captured artifact.

