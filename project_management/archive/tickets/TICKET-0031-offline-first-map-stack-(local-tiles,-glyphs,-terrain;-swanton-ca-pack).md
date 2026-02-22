# TICKET-0031: Offline-first map stack (local tiles, glyphs, terrain; Swanton CA pack)

**Status:** Done (Tier A validated installed `0.1.9.112`; Tier B deferred to `DW-97`)

## Description
Make the Map tab fully functional without internet access after setup by hosting all map assets locally (controller tile server + locally-hosted glyphs/sprites/terrain), and refactor the dashboard map rendering/editing stack so all interactive features (nodes, sensors, markup) are MapLibre GeoJSON sources + style layers (no HTML/DOM markers).

This is a foundational shift: the controller becomes the canonical map asset host, and the dashboard’s map UX becomes deterministic (no “blank canvas”, no “place mode does nothing”, no “saved but not visible”, no crash on complex polygons).

The first required offline dataset is **Swanton, California and surrounding areas**, with zoom levels sufficient for farm-scale editing (≈300′ “viewport height”).

## Scope
- Offline map assets (controller)
  - Local tile server that serves MBTiles datasets over LAN/localhost.
  - Basemaps served locally:
    - Streets/topo basemap (raster MBTiles acceptable).
    - Satellite basemap (raster MBTiles).
  - Locally-hosted glyphs for MapLibre text labels (`glyphs/{fontstack}/{range}.pbf`).
  - Optional locally-hosted sprites if used by styles.
  - Local terrain tileset (raster-dem, Terrarium) for hillshade/3D terrain (at least hillshade overlay supported).
- Map UX refactor (dashboard)
  - Nodes + sensors + markup rendered via GeoJSON sources and MapLibre style layers (no `new maplibregl.Marker()`).
  - One unified draw/edit interaction model for points/lines/polygons with consistent selection/move/reshape semantics.
  - Placement is immediate and visible: “Place/Move” shows the point instantly; reload preserves visibility.
  - Complex polygon drawing/editing is stable (no runaway renders/crash).
- Setup Center UX (dashboard)
  - “Offline maps” section to download/install the Swanton CA map pack (basemaps + glyphs + terrain) and show status/progress.
  - Clear guidance inline (operators do not read separate docs).
- Wiring / semantics
  - Node pins are the canonical location for location-based features (hyperlocal weather, Forecast.Solar/Open‑Meteo targets).
  - Sensors inherit node location unless explicitly overridden.
  - Markup (polygons/lines/markers) is documentation-only and does not affect weather/forecast targeting.

## Acceptance Criteria
- Offline correctness
  - With internet blocked at the browser level, Map tab still renders basemap tiles + labels and interactive overlays.
  - All map asset URLs (tiles/glyphs/sprites/terrain) point to controller-hosted endpoints.
- Functional UX
  - Base map never renders as a blank canvas when offline map pack is installed.
  - Clicking “Place/Move” for a node and then clicking the map visibly places/moves the node immediately.
  - Nodes/sensors/markup saved in the active map are visible after a page reload.
  - Drawing and editing a complex polygon does not crash the page.
- Persistence + integrations
  - Node placement drives hyperlocal weather/forecast locations and continues to work after the refactor.
  - Sensor override placement works; sensors without overrides inherit node placement.
- Validation
  - Tier A: Rebuilt/refreshed installed controller (no DB/settings reset) and captured + viewed Map screenshots proving offline map load + placement + markup stability. Artifacts stored under `manual_screenshots_web/` and referenced from a run log under `project_management/runs/`.
  - Tier B: deferred to `DW-97` clean-host validation cluster (if needed for full E2E).

## Notes
- This ticket is expected to introduce/extend map-related API endpoints (tile server + offline pack management). Ensure OpenAPI registration stays complete (CI drift/coverage gate should catch omissions).
- Swanton CA pack is the first production region; the design should support additional regions/packs later without rewriting the map stack.
