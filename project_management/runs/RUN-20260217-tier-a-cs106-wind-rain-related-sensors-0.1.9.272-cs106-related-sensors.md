# RUN-20260217 Tier A — CS-106 wind/rain related sensors — Installed controller 0.1.9.272-cs106-related-sensors

**Date:** 2026-02-17 (PST; 2026-02-18 UTC)

## Scope

- Validate CS-106 on the installed controller (Tier A; **no DB/settings reset**):
  - Related Sensors for **Reservoir Depth** over **7 days** now surfaces **Rain (daily)** + **Wind direction** as related candidates.

## Preconditions

- [x] No DB/settings reset performed (Tier‑A rule).
- [x] Installed setup daemon health OK: `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server health OK: `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Installed version recorded:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.272-cs106-related-sensors` (previous: `0.1.9.271`)

## Build + Upgrade (Installed Controller; NO DB reset)

> Rebuild/refresh SOP: `docs/runbooks/controller-rebuild-refresh-tier-a.md`

- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.272-cs106-related-sensors.dmg`
- Build log:
  - `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.272-cs106-related-sensors.log`

### Upgrade anomaly (resolved)

- Initial upgrade attempt failed during Postgres migrations due to disk exhaustion:
  - Error: `No space left on device` while applying migration `005_sensor_id_varchar.sql`
- Resolved by freeing disk:
  - Command: `cargo clean --manifest-path apps/core-server-rs/Cargo.toml`
  - Re-attempted upgrade after cleanup; upgrade completed successfully.

### Post-upgrade verification

- [x] `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- [x] Installed smoke: `make e2e-installed-health-smoke` (PASS)

## Related Sensors Tier‑A evidence (Reservoir Depth; last 7 days)

### Command used (screenshots)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt \
  --related-focus-sensor-id=ea5745e00cb0227e046f6b88 \
  --related-range-hours=168 \
  --related-interval-seconds=1800 \
  --related-scope=all_nodes \
  --related-timeout-ms=420000 \
  --out-dir=manual_screenshots_web/20260217_202735
```

### Captured evidence (paths must exist)

- `manual_screenshots_web/20260217_202735/trends_chart_settings_7d.png`
  - Range: Last 7 day
  - Interval: 30 min
- `manual_screenshots_web/20260217_202735/trends_related_sensors_reservoir_depth_7d_rain_daily.png`
- `manual_screenshots_web/20260217_202735/trends_related_sensors_reservoir_depth_7d_wind_direction.png`

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260217_202735/trends_chart_settings_7d.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_202735/trends_related_sensors_reservoir_depth_7d_rain_daily.png`
- [x] REVIEWED: `manual_screenshots_web/20260217_202735/trends_related_sensors_reservoir_depth_7d_wind_direction.png`

### Visual checks (required)

- [x] PASS: `trends_chart_settings_7d.png` shows Range = Last 7 day and Interval = 30 min.
- [x] PASS: `trends_related_sensors_reservoir_depth_7d_rain_daily.png` shows **Rain (daily)** surfaced as a related candidate for Reservoir Depth (ranked result present).
- [x] PASS: `trends_related_sensors_reservoir_depth_7d_wind_direction.png` shows **Wind direction** surfaced as a related candidate for Reservoir Depth (ranked result present).

### Findings

- Rain + wind direction now surface as related candidates for Reservoir Depth over a 7‑day window on installed controller `0.1.9.272-cs106-related-sensors`.

### Reviewer declaration

I viewed each screenshot listed above.

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260217-tier-a-cs106-wind-rain-related-sensors-0.1.9.272-cs106-related-sensors.md`
- Result: PASS
