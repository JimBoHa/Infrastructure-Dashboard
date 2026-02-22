# TSE-0013: Dashboard-web refactor — Related Sensors (job-based UX)

Priority: P1
Status: In Progress (tracked as TSSE-14 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Replace the current client-side scan loop with a job-based UX:
- start scan
- show progress (counts + phase)
- show ranked results + episodes
- preview a relationship (episode zoom)

## Scope
- Update Trends → Related sensors panel to call analysis job endpoints.
- Replace the following current client-side behavior:
  - `apps/dashboard-web/src/features/trends/components/AutoComparePanel.tsx` query loop using
    `apps/dashboard-web/src/features/trends/utils/metricsBatch.ts` (`fetchMetricsSeriesBatched`)
  - browser-side scoring in `apps/dashboard-web/src/features/trends/utils/relatedSensors.ts` and `eventMatch.ts`
- Display:
  - “computed through” watermark
  - episode summaries
  - “why ranked” explanations
  - clear handling of partial/no results
- Remove user-facing “series too large” errors.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: UI component refactor.
- Deliverable: integrate progress streaming/polling.
- Deliverable: UX review of episodes and explanations.

## Acceptance Criteria
- No more “Scanning candidates …” driven by hundreds of metrics requests.
- UI remains responsive and cancel works.
- Previews load only when requested.
