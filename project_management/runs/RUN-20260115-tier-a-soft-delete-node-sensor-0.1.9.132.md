# RUN-20260115-tier-a-soft-delete-node-sensor-0.1.9.132

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.132
- **Purpose:** Validate soft delete for nodes + sensors (retain data, hide everywhere, names reusable)

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Validation Steps
1. Upgrade installed controller to `0.1.9.132` via setup daemon.
2. Run Playwright smoke that:
   - Creates a test node + two sensors via API.
   - Adds map features for node + sensor.
   - Deletes one sensor via Sensor detail drawer UI.
   - Verifies name reuse by recreating sensor with same name.
   - Deletes node via Node detail page UI (admin-only).
   - Verifies node disappearance + name reuse.
   - Confirms map features API no longer includes deleted entities.

## Notes / Side Effects
- The Playwright scenario uses the existing API token for backend setup, but it needs an **admin** browser session to click “Delete node” (UI is admin-only). If the provided token is not `role=admin`, the test creates a one-off admin user (randomized email) and logs in via `/api/auth/login`. This leaves that admin user in the DB.

## Commands
- Upgrade:
  - `POST http://127.0.0.1:8800/api/config` (bundle `FarmDashboardController-0.1.9.132.dmg`)
  - `POST http://127.0.0.1:8800/api/upgrade`
- Sanity checks:
  - `curl -H "Authorization: Bearer <token>" http://127.0.0.1:8000/api/outputs`
- Playwright:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=<token> FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.132 npx playwright test playwright/soft-delete-node-sensor.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshots (viewed):** `manual_screenshots_web/tier_a_0.1.9.132_soft_delete_2026-01-15_084300631Z/`
  - `01_nodes_before.png`
  - `03_sensor_drawer_before_delete.png`
  - `04_sensor_deleted.png`
  - `06_node_deleted.png`

## Result
- **PASS** (Playwright scenario passed; screenshots captured and reviewed).
