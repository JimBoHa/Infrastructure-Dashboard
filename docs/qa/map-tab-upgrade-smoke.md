# Map Tab Upgrade Smoke Checklist

## Scope
Post-upgrade manual smoke for the Map tab refactor (DW-189/DW-190/DW-191). Focus on layers, overlays, saved maps, custom feature CRUD, device placement, and offline packs.

## Preconditions
- Admin user with `config.write`.
- At least one node with at least one sensor.
- At least one existing saved map (or create one during this checklist).
- Swanton offline pack available (installed or ready to download).

## Checklist (quick)
1. Open **Map** tab after upgrade. Map canvas + sidebar render without error banners.
2. **Base map layers:** Toggle Streets/Satellite/Topo. Map updates immediately. If offline pack is installed, offline variants are selectable; otherwise they show as disabled with "Install pack".
3. **Overlays:** In **Overlays**, add or edit an overlay (WMS/ArcGIS/XYZ). Toggle enabled, adjust opacity, move up/down. Confirm draw order changes on the map. Delete any test overlay if created.
4. **Saved maps:**
   - Click **Save view**, refresh the page, and confirm the view persists.
   - Click **Save as...**, name a new map, then select it from **Saved map**.
   - Switch back to a prior save and confirm the view/markup placements restore.
5. **Markup CRUD:**
   - Create a Marker, Polygon, and Line from **Markup**.
   - Verify each appears on the map and in the list.
   - Edit name/color/kind; confirm updates render.
   - Delete each and confirm removal from map + list.
6. **Node placement:**
   - Pick a node, click **Place/Move**, click the map, and confirm placement + coordinates update.
   - Use **Clear** to remove placement, then re-place once to confirm flow still works.
7. **Sensor placement override:**
   - On a sensor under a node, click **Override**, place it, and confirm the badge shows **Custom**.
   - Click **Reset** to inherit the node location again (badge returns to **Node/Needs node** as appropriate).
8. **Offline pack:**
   - In **Offline map pack**, confirm status renders.
   - If not installed, click **Download**, wait for **Installed**, then select an offline base layer.
   - If already installed, toggle to an offline base layer and pan/zoom to confirm tiles load.

## Notes
- This is a smoke checklist only; deeper regressions are covered by the Tier-B Map cluster (DW-97).
