# RUN-20260112 — Tier A closeout loop (installed controller) — 0.1.9.100

**Environment:** Installed controller (no DB/settings reset). Bundle version reported by `/usr/local/farm-dashboard/state.json` was `0.1.9.100`.

**Tier A rule:** evidence includes at least one captured **and viewed** screenshot of the affected surface.

## Evidence (commands + artifacts)

### Health

```bash
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/healthz
```

Artifacts:
- `/tmp/tier_a_closeout_healthz.json`
- `/tmp/tier_a_closeout_setup_healthz.json`

### Screenshots (captured + viewed)

Captured with `apps/dashboard-web/scripts/web-screenshots.mjs` against the installed app with a Tier‑A API token.

Artifacts (folder + key pages):
- `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/map.png`
- `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/trends.png`
- `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/backups.png`
- `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/power.png`
- `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/analytics.png`
- Full manifest: `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/manifest.json`

## Cluster checks (Tier A)

### Map (DW-61/DW-71/DW-72/DW-73)

Checks:
- Map renders with Satellite base layer selected; markup list present (polygons/lines/markers visible in the right panel): `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/map.png`
- Active map save + center/zoom persisted: `GET /api/map/settings` → `/tmp/tier_a_closeout_map_settings.json`
- Markup persisted: `GET /api/map/features` → `/tmp/tier_a_closeout_map_features.json` (6 features)

Commands:
```bash
TOKEN="$(cat /tmp/tier_a_api_token.txt)"
curl -fsS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/map/settings > /tmp/tier_a_closeout_map_settings.json
curl -fsS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/map/features  > /tmp/tier_a_closeout_map_features.json
```

### Trends/COV/CSV (CS-68, DW-76/DW-83/DW-86/DW-87/DW-88/DW-93/DW-94)

Checks:
- Trends page loads with stable layout (no unbounded height growth) and shows the long-range presets + independent axes controls: `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/trends.png`
- Metrics query returns bucketed points for a real sensor using explicit `start/end` + `interval` (API contract): manual curl (see below)

Commands (example):
```bash
TOKEN="$(cat /tmp/tier_a_api_token.txt)"
SENSOR_ID="$(jq -r 'map(select(.latest_ts != null)) | .[0].sensor_id' /tmp/tier_a_closeout_sensors.json)"
START="$(python3 -c 'import datetime;print((datetime.datetime.now(datetime.UTC)-datetime.timedelta(hours=24)).replace(microsecond=0).isoformat())')"
END="$(python3 -c 'import datetime;print(datetime.datetime.now(datetime.UTC).replace(microsecond=0).isoformat())')"
curl -fsS -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8000/api/metrics/query?sensor_ids=${SENSOR_ID}&start=${START}&end=${END}&interval=60"
```

### Backups/Exports (DW-89/DW-90/DW-96)

Checks:
- Backups page renders Controller settings bundle + Database export panels: `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/backups.png`
- No restore/export action executed in Tier A (download-only on prod).

### Power/Analytics (CS-66, AN-32/33/34, DW-75/DW-79/DW-80/DW-81/DW-84)

Checks:
- Power page renders W/V/A graphs + voltage quality analysis panel: `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/power.png`
- Analytics page renders weather + PV forecast panels and “Power nodes” section without overflow: `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/analytics.png`
- Forecast provider status is healthy and shows Forecast.Solar public rate limits: `GET /api/forecast/status` → `/tmp/tier_a_closeout_forecast_status.json`
- `/api/analytics/power` returns expected series lengths (24h totals/grid 5‑min buckets; solar/battery 60‑sec buckets): `/tmp/tier_a_closeout_analytics_power.json`

Commands:
```bash
TOKEN="$(cat /tmp/tier_a_api_token.txt)"
curl -fsS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/forecast/status > /tmp/tier_a_closeout_forecast_status.json
curl -fsS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/analytics/power > /tmp/tier_a_closeout_analytics_power.json
```

### Core correctness (DW-74, SA-9)

Checks:
- Nodes/offline duration formatting visible across dashboard surfaces (Tier‑A UI pass): covered by the “Nodes/Power/Analytics” screenshots in the folder above.
- Schedules page loads and the create/edit surfaces render (Tier‑A UI pass): `manual_screenshots_web/tier_a_closeout_0.1.9.100_20260112_081049/schedules.png`, `.../schedules_new.png`, `.../schedules_edit.png`

