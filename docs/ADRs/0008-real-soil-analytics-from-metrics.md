# 0008. Real soil analytics from metrics

* **Status:** Accepted
* **Date:** 2026-01-31
* **Implementation:** AN-35 (`project_management/archive/tickets/TICKET-0048-analytics-overview-soil-moisture-graph-flatline.md`)

## Context
The Analytics Overview page includes a “Soil moisture” fleet-level chart. It was showing a straight, flat line at `0%` even while real soil moisture sensors were ingesting correct values (verified via the Trends page).

Root cause: the Rust core-server endpoint `GET /api/analytics/soil` was still a stub that returned a pre-filled time series of zeroes (`value: 0.0`) regardless of DB contents. The dashboard correctly rendered what it was given, which looked like “all soil moisture is 0%”.

## Decision
Implement `GET /api/analytics/soil` as a real aggregation over the controller DB (`sensors` + `metrics`) so the Analytics Overview soil chart reflects real moisture telemetry.

- **Sensor selection**
  - Include active sensors with `unit="%"` and `type ∈ {"moisture", "percentage"}`.
  - Ignore `deleted_at IS NOT NULL` sensors.

- **Fleet-level series**
  - For the last 168 hours, build hourly buckets using Timescale `time_bucket`.
  - Compute a per-sensor average per bucket, then aggregate across sensors to produce:
    - `series_avg`: average of the per-sensor averages
    - `series_min`: min of the per-sensor averages
    - `series_max`: max of the per-sensor averages
  - Keep `series` for backwards compatibility (alias of `series_avg`).

- **Field summaries**
  - Build a small `fields[]` summary based on latest sensor readings (last point per sensor), grouped by:
    - `config.field` or `config.location` or `config.ws_field`, otherwise
    - parse a label in parentheses from the sensor name (e.g., `Soil moisture (Field 7 / Zone A)`), otherwise
    - fallback to the sensor name.
  - For each group, compute `min/max/avg` across sensors (latest values).

## Consequences
**Benefits**
- Analytics Overview soil moisture chart matches real telemetry (no more fabricated zero flatline).
- Fleet-level `min/max/avg` lines become available without duplicating aggregation logic in the dashboard.
- Backwards compatible response shape for existing clients (`fields` + `series` remain; `series_*` are additive).

**Tradeoffs / risks**
- Adds DB work to `GET /api/analytics/soil` (multiple queries + bucketing). Mitigated by coarse 1-hour buckets and bounded 168h lookback.
- Sensor selection is policy: if future moisture sensors use different `type`/`unit` values, they may not be included until the policy is extended.

**Alternatives considered**
- Client-side aggregation in dashboard-web by fetching all soil sensors and aggregating in the browser. Rejected: duplicates logic, increases payload size, and makes summary correctness depend on UI code paths.
