# TICKET-0064: Related Sensors: online instrumentation + UX metrics

**Status:** Done

## Description
Offline metrics aren’t enough; we need online signals to validate whether Related Sensors is producing usable troubleshooting leads.

This ticket adds lightweight instrumentation for Related Sensors interactions and defines a small metric suite.

## Scope
* [ ] Define event taxonomy (decision complete):
  - panel_opened
  - run_started (quick/refine/advanced + params summary)
  - run_completed (counts, timings, result_count)
  - candidate_opened (rank, score, evidence tier)
  - episode_selected
  - add_to_chart_clicked
  - refine_clicked
  - pin_toggled (if implemented)
  - jump_to_timestamp_clicked (if implemented)
* [ ] Define online metrics computed from events:
  - time-to-first-action (open → candidate click/add/jump)
  - lead acceptance rate (add-to-chart / pin)
  - refine rate
  - backtrack rate (many candidates opened without action)
  - stability proxy (overlap@10 between quick vs refine in-session)
* [ ] Implement event emission in dashboard-web (privacy-safe; no raw sensor values).
* [ ] Add a local dev-only log sink (or structured console output) for validation.

## Acceptance Criteria
* [ ] Events are emitted for the defined user actions with stable schemas.
* [ ] Events include enough context to compute the metric suite without leaking raw telemetry values.
* [ ] `make ci-web-smoke` passes.

## Notes
Implementation location: `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx` and related components.
