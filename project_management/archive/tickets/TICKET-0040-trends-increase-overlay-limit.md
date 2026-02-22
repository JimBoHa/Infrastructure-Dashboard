# TICKET-0040: Trends: Increase overlay sensor limit (allow >10)

**Status:** Done

## Description

Trends currently caps the number of overlayed sensors at 10. This is too limiting for real analysis (for example, comparing multiple circuits + weather + derived context).

Increase the overlay limit to allow more than 10 sensors, while keeping the UI usable and performance predictable.

## Scope

* [x] Increase the maximum number of sensors selectable in Trends overlays (from 10 to a higher limit, e.g. 20).
* [x] Ensure the “selected count” and instructional copy reflect the new limit.
* [x] Ensure “Related sensors” / “Relationships” / “Matrix Profile” add-to-chart actions respect the new limit.
* [x] Validate that the chart remains usable (legend/tooltips, independent axes UX) with the new maximum.
* [x] Tier A validation on an installed controller (refresh bundle + viewed screenshots).

## Acceptance Criteria

* [x] Trends Sensor picker allows selecting more than 10 sensors (at least 20).
* [x] When the limit is reached, UX clearly explains it (no silent failure).
* [x] CI: `make ci-web-smoke` passes.
* [x] Tier A run log recorded under `project_management/runs/` with at least one viewed screenshot showing the updated max selection count.

## Validation

- Tier A: `project_management/runs/RUN-20260120-tier-a-dw177-trends-overlay-limit-0.1.9.188.md`
- Evidence: `manual_screenshots_web/20260120_133427/trends.png`

