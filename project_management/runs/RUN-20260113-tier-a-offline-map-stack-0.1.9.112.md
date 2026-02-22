# RUN-20260113 — Tier A Offline-first Map stack (installed controller `0.1.9.112`)

## Goal
- Tier A validation (installed controller; **no DB/settings reset**): Map renders and remains usable with **no internet after setup** by serving offline tiles + glyphs + terrain locally, and rendering interactive features (nodes/markup) as MapLibre layers.

## Installed controller version
- `/usr/local/farm-dashboard/state.json` → `current_version: "0.1.9.112"`
- Setup bundle path (setup-daemon config): `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.112.dmg`

## Offline map pack install / status
- Verified Swanton pack is installed:
  - `curl -fsS http://127.0.0.1:8000/api/map/offline/packs | jq '.[] | select(.id=="swanton_ca") | {id,status,progress,error,updated_at}'`
  - `status: "installed"` and terrain layer reports `downloaded == total` in `progress.layers.terrain`.
- Ensured the active basemap uses offline tiles:
  - `curl -fsS http://127.0.0.1:8000/api/map/settings | jq '{active_base_layer_id,active_save_id}'`
  - `curl -fsS http://127.0.0.1:8000/api/map/layers | jq '[.[] | {id,system_key,name}]'`

## Spot-check: tiles + glyphs are fully downloaded
- Offline pack progress shows `downloaded == total` and `failed == 0` for every layer:
  - `curl -fsS http://127.0.0.1:8000/api/map/offline/packs/swanton_ca | jq '{status,error,progress}'`
  - Satellite: `53668/53668` (z10–18), Streets: `13591/13591` (z10–17), Topo: `3471/3471` (z10–16), Terrain: `71/71` (z10–13)
- On-disk MBTiles exist and are non-trivial size (controller local storage):
  - `/Users/Shared/FarmDashboard/storage/map/tiles/swanton_ca/satellite.mbtiles` (~560 MB)
  - `/Users/Shared/FarmDashboard/storage/map/tiles/swanton_ca/streets.mbtiles` (~67 MB)
  - `/Users/Shared/FarmDashboard/storage/map/tiles/swanton_ca/topo.mbtiles` (~38 MB)
  - `/Users/Shared/FarmDashboard/storage/map/tiles/swanton_ca/terrain.mbtiles` (~5.7 MB)
- Served endpoints return `200` from the installed controller (LAN/localhost):
  - `GET /api/map/tiles/swanton_ca/satellite/18/42088/101988` → `image/jpeg`
  - `GET /api/map/tiles/swanton_ca/streets/17/21044/50994` → `image/png`
  - `GET /api/map/tiles/swanton_ca/topo/16/10522/25497` → `image/jpeg`
  - `GET /api/map/tiles/swanton_ca/terrain/13/1315/3187` → `image/png`
  - `GET /api/map/glyphs/Noto%20Sans%20Regular/0-255` → `application/x-protobuf`

## Playwright screenshots (captured + viewed; internet blocked)
- Token created (not committed): `/tmp/farmdashboard_screenshot_token.txt`
- Command:
  - `cd apps/dashboard-web && node scripts/web-screenshots.mjs --no-core --no-web --block-external --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/tmp/farmdashboard_screenshot_token.txt --out-dir=manual_screenshots_web/20260113_003758_offline_map_installed`
- Output folder (viewed):
  - `manual_screenshots_web/20260113_003758_offline_map_installed/`
- Evidence screenshots (viewed):
  - `manual_screenshots_web/20260113_003758_offline_map_installed/map.png` (offline basemap renders; node label visible; banner shows offline pack installed).
  - `manual_screenshots_web/20260113_003758_offline_map_installed/setup.png` (offline pack install UI + status).
