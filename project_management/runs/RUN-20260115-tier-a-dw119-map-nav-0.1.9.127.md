# RUN-20260115-tier-a-dw119-map-nav-0.1.9.127

## Summary

Tier A validation on the installed controller after fixing the intermittent Map → Sensors/Nodes navigation crash (“Application error” until manual refresh).

## Date

2026-01-15

## Environment

- Tier: A (installed controller; no DB/settings reset)
- Controller bundle: `0.1.9.127`
- Dashboard base: `http://127.0.0.1:8000`

## Steps

1. Upgrade installed controller to bundle `0.1.9.127`.
2. Open `/map`, wait for Map UI to render, then navigate to:
   - Sensors & Outputs (`/sensors`) via sidebar link.
   - Nodes (`/nodes`) via sidebar link.
3. Confirm no Next.js “Application error” screen and no manual refresh needed.

## Evidence

- Screenshots (captured + viewed):
  - `manual_screenshots_web/tier_a_0.1.9.127_dw119_map_nav_20260114_231331/map_to_sensors.png`
  - `manual_screenshots_web/tier_a_0.1.9.127_dw119_map_nav_20260114_231331/map_to_nodes.png`

