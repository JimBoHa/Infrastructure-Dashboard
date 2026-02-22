# RUN-20260119 — Tier A DW-158 Trends analysis keys (installed controller `0.1.9.170`)

## Goal
- Tier A validation (installed controller; **no DB/settings reset**): Verify Trends analysis “Keys” accompany their relevant panels/visualizations (no scroll-hunting), and variable definitions are visible in-context.

## Installed controller version
- `/usr/local/farm-dashboard/state.json` → `current_version: "0.1.9.170"`
- Setup bundle path (setup-daemon config): `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.170.dmg`

## Upgrade / refresh (Tier A)
- Built controller bundle DMG:
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.170 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.170.dmg --native-deps /usr/local/farm-dashboard/native`
- Pointed setup daemon at stable bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.170.dmg"}'`
- Upgraded via setup daemon (no admin; launchd KeepAlive restarts services):
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/healthz`
  - `make e2e-installed-health-smoke`

## Evidence (screenshots)
- **Related sensors** (Key open, events mode definitions present):
  - Viewed:
    - `manual_screenshots_web/tier_a_0.1.9.170_trends_auto_compare_2026-01-19_084944474Z/01_trends_auto_compare_events_key.png`
- **Matrix Profile explorer** (Key open adjacent to the visualization):
  - Viewed:
    - `manual_screenshots_web/tier_a_0.1.9.170_trends_matrix_profile_2026-01-19_085409295Z/01_trends_matrix_profile_curve_key.png`
- **Relationships** (matrix Key open adjacent to the matrix):
  - Viewed:
    - `manual_screenshots_web/tier_a_0.1.9.170_trends_relationships_2026-01-19_092549101Z/01_trends_relationships_matrix_key.png`

## Notes
- `make e2e-web-smoke` is intentionally **not** run on this host: the installed controller stack is expected to be running, but Tier B E2E requires a clean-host preflight (no running controller processes).
- Tier B clean-host validation remains tracked via the existing web cluster ticket(s) (e.g., `DW-98` / `DW-114`).

## Follow-up evidence (new per-section keys)

- Installed controller refreshed to `0.1.9.171` to validate the Sensor picker + Trend chart keys and updated variable wording.
- Screenshots (viewed):
  - `manual_screenshots_web/tier_a_0.1.9.171_trends_keys_2026-01-19_102303954Z/01_trends_sensor_picker_key.png`
  - `manual_screenshots_web/tier_a_0.1.9.171_trends_keys_2026-01-19_102303954Z/02_trends_trend_chart_key.png`
  - `manual_screenshots_web/tier_a_0.1.9.171_trends_auto_compare_2026-01-19_102407281Z/01_trends_auto_compare_events_key.png`
  - `manual_screenshots_web/tier_a_0.1.9.171_trends_relationships_2026-01-19_102710039Z/02_trends_relationships_pair_key.png`
  - `manual_screenshots_web/tier_a_0.1.9.171_trends_matrix_profile_2026-01-19_102631521Z/02_trends_matrix_profile_shape_key.png`
