# RUN-20260131 — Tier A — DW-198 Trends chart analysis toolbar (0.1.9.229)

- **Date:** 2026-01-31
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.229
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds_TierA/FarmDashboardController-0.1.9.229.dmg`

## Scope

- DW-198: Replace the default Highcharts Stock Tools left-side GUI on the Trends chart with a polished, dashboard-native “Chart analysis” toolbar (Lines / Measure / Annotate / Navigate / Erase), including the best-fit regression tool.
- Wire toolbar buttons to Highcharts navigation bindings without enabling the Stock Tools GUI.
- Ensure toolbar-created Highcharts annotations persist via `/api/chart-annotations` and render on refresh.

## Web smoke + build (repo)

```bash
make ci-web-smoke
cd apps/dashboard-web && CI=1 NEXT_PUBLIC_API_BASE=http://127.0.0.1:8000 npm run build
```

Result: `PASS`.

## Core smoke (repo)

```bash
make ci-core-smoke
```

Result: `PASS`.

## Refresh installed controller (Upgrade)

Preflight health checks:

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
```

Installed version before: `0.1.9.228`.

Rebuild + refresh:

```bash
python3 tools/rebuild_refresh_installed_controller.py \
  --output-dir /Users/Shared/FarmDashboardBuilds_TierA \
  --allow-dirty \
  --version 0.1.9.229
```

Bundle build output:
- DMG: `/Users/Shared/FarmDashboardBuilds_TierA/FarmDashboardController-0.1.9.229.dmg`
- Log: `/Users/Shared/FarmDashboardBuilds_TierA/logs/bundle-0.1.9.229.log`

Installed version after: `0.1.9.229` (previous: `0.1.9.228`).

Installed stack smoke:

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier‑A screenshots (captured + viewed)

Evidence directory:
- `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529`

Evidence (opened and visually reviewed):
- `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trends_page_with_toolbar.png`
  - Verified: Chart analysis toolbar is visible; default Stock Tools left-side GUI is not present.
- `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trend_chart_with_annotation.png`
  - Verified: Toolbar tool creates a Highcharts annotation and persists via `/api/chart-annotations`.
- `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trend_chart_after_reload.png`
  - Verified: The persisted annotation renders after refresh (note: sensor selection is re-applied after reload; selection is not currently persisted).
- `manual_screenshots_web/tier_a_0.1.9.229_dw198_trends_toolbar_20260131_053529/trend_chart_after_delete.png`
  - Verified: “Clear all” removes the annotation and calls delete via `/api/chart-annotations/:id`.

