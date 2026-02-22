# TICKET-0041: Trends: Co-occurring anomalies (multi-sensor)

**Status:** Done

## Problem

Today, Trends can help find:
- **Continuous relationships** (correlation/lag) and
- **Pairwise spike/event alignment** (Related sensors → Events mode) and
- **Single-series anomalies** (Matrix Profile).

But operators still lack a fast way to answer:

> “What weird things happened at the same time across multiple sensors?”

Examples:
- A pump turns on and several circuits spike in current simultaneously.
- A valve change causes pressure, flow, and power changes together.
- A weather transition causes multiple derived + hardware sensors to show a coordinated discontinuity.
- A single sensor spikes (e.g., camera light, irrigation valve, alarm trigger) and operators need to quickly find **which other sensors spiked at the same time**, even if they weren’t already on the chart.

## Goal

Add a dedicated Trends panel that:
- Detects anomalies/events **per series** (robustly),
- Finds **co-occurrences** across selected sensors (same Interval buckets), and can also operate as a **focus scanner** (one selected sensor → scan all sensors),
- Adds **extra weight** when multiple anomalies coincide (2+ sensors),
- Makes the result easy to **scan, sort, and visually connect** to the Trend chart.

## Scope

* [x] Add a new Trends panel: “Co-occurring anomalies”.
* [x] Use robust per-series change-event detection (not a simple moving average).
* [x] Compute per-bucket co-occurrence groups across selected sensors.
* [x] Focus-scan mode: choose a focus sensor (can be the only selected sensor) and scan **all sensors** to surface timestamps where the focus sensor has an event and other sensors have events at the same time (with tolerance).
* [x] Score/weight events by group size (and optionally severity) so “more sensors at once” ranks higher.
* [x] Provide a clear, plain-English Key/Glossary near the panel (no “scroll away” explanations).
* [x] Highlight co-occurrence timestamps directly on the Trend chart (vertical markers).
* [x] Validation: `make ci-web-smoke` + Tier A (installed controller refresh + viewed screenshots).

## Acceptance Criteria

* [x] Trends shows a “Co-occurring anomalies” panel when 1+ sensors are selected.
* [x] The panel lists time buckets where **≥2 sensors** have an anomaly/event in the same Interval bucket.
* [x] With **1 selected sensor**, the panel supports a focus mode that scans all sensors and lists timestamps where ≥2 sensors (including the focus) have events within the tolerance window.
* [x] Results are sorted with extra weight for larger groups (e.g., 3 sensors > 2 sensors).
* [x] Users can adjust detection sensitivity (e.g., z-threshold) and minimum group size.
* [x] UI is cohesive (CollapsibleCard, consistent spacing/typography, Key in-section).
* [x] `make ci-web-smoke` passes.
* [x] Tier A run log recorded under `project_management/runs/` with screenshots showing the new panel + chart markers.

## Validation

- Tier A: `project_management/runs/RUN-20260121-tier-a-dw178-trends-cooccurring-anomalies-0.1.9.193.md`
- Evidence: `manual_screenshots_web/20260121_073030/trends_cooccurrence.png`
