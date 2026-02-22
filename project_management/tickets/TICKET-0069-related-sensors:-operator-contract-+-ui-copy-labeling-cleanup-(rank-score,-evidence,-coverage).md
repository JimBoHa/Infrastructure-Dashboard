# TICKET-0069: Related Sensors: operator contract + UI copy-labeling cleanup (Rank score, Evidence, coverage)

**Status:** Done

## Description
Related Sensors Unified v2 is an “investigation leads” tool, but today the UI copy and labels (“Score”, “Confidence”, raw co-occurrence magnitudes) can be misread as probability, statistical significance, or causality.

This ticket codifies a strict **operator contract** for what “Related” means, and updates UI/copy so the panel is precision-biased, non-misleading, and self-disclosing about candidate coverage and effective interval.

## Scope
* [x] **Contract text**: add/align all operator-visible text to the following rules:
  - “Related” = time-aligned change evidence on bucketed series within the selected window/interval (optionally with lag).
  - Explicitly NOT: causality, correlation-of-levels (as a rank driver), probability, statistical significance, exhaustive search.
  - “Rank score” is pool-relative (not comparable across runs with different pools).
  - “Evidence” is heuristic coverage tier (not a probability).
* [x] **Panel subtitle + disclaimers (exact text)**:
  - Subtitle: `Sensors whose change events align with the focus sensor in this time range (optionally with lag). Not causality.`
  - Always-visible micro-disclaimer (under buttons): `Rankings are relative to the sensors evaluated in this run. Scores are not probabilities and can change when scope/filters change.`
* [x] **Coverage + interval disclosure (must be visible)**:
  - `Evaluated: <evaluated_count> of <eligible_count> eligible sensors (limit: <candidate_limit_used>).`
  - `Effective interval: <interval_seconds_eff> (requested: <interval_seconds_requested>).`
* [x] **Buttons (exact labels)**:
  - Simple primary: `Find related sensors`
  - Simple refine: `Refine (more candidates)`
  - Advanced run: `Advanced (configure scoring)`
* [x] **Results list row labels**:
  - Replace right-side `Score` label with `Rank score` (tooltip below).
  - Remove the redundant `Blend:` pill (no duplicate blended number).
  - Replace `Confidence: high|medium|low` with `Evidence: strong|medium|weak`.
* [x] **Tooltips (exact meaning)**:
  - Rank score tooltip: `0–1 rank score relative to the evaluated candidates in this run. Not a probability. Not comparable across different runs or scopes.`
  - Evidence tooltip: `Heuristic tier based on matched events and/or shared anomaly buckets. Not statistical significance.`
* [x] **Co-occurrence pills**:
  - Remove raw `Co-occur: <huge number>` pill from list rows.
  - Replace with:
    - `Shared buckets: <cooccurrence_count>`
    - `Co-occ strength: <coocc_norm_0_to_1>`
  - Co-occ strength tooltip: `Normalized to the strongest candidate in this run. Based on shared high-severity event buckets.`
* [x] **Events pill**:
  - Replace `Events: <score>` with:
    - `Event match (F1): <events_score> • matched: <events_overlap>`
  - Tooltip: `Event match uses F1 overlap of detected change events at the best lag. Matches allow a tolerance window (in buckets) after applying lag.`
  - Also surface event counts (either in the tooltip or preview metrics):
    - `Focus events: <n_F>`
    - `Candidate events: <n_C>`
* [x] **Lag pill sign semantics**:
  - `Lag: -8m (candidate earlier)`
  - `Lag: +50m (candidate later)`
  - Tooltip: `Lag is the candidate time offset that maximizes event-match overlap.`
* [x] **Preview pane**:
  - Rename “Why this sensor is related” → `Evidence summary`
  - Rename metrics:
    - `Event match (F1)`
    - `Matched events`
    - `Shared selected buckets`
    - `Co-occ strength` (0–1) (raw available only in Advanced tooltip/debug)
  - Add line under metrics: `All evidence is computed on bucketed data at effective interval <interval_seconds_eff>.`
  - Replace episode label `Peak` → `Peak |Δz|` with tooltip: `Peak absolute robust z-score of focus deltas within matched events in this episode.`
  - Replace sparse warning copy (exact template):
    - `Weak episode: only <num_points> matched events (<coverage_pct>% of focus events). Treat as low evidence. Try a different episode, expand the time range, or lower the event threshold.`
  - Guardrail banners:
    - If `<n_F> < 3`: `Too few focus events for stable ranking. Expand the time range or lower the event threshold.`
    - If `<n_F> > 2000`: `Focus sensor is very eventful; results may reflect noise. Increase the event threshold or raise the interval.`
* [x] **No-results state (exact copy)**:
  - `No candidates exceeded the evidence threshold in this time range. Evaluated <evaluated_count> of <eligible_count> eligible sensors. Try Refine (more candidates), expand the time range, lower the event threshold, or include weak evidence in Advanced.`
* [x] **Correlation matrix framing**:
  - Rename block title: `Correlation (bucketed levels, not used for ranking)`
  - Subtitle: `Pearson correlation on aligned bucket timestamps. Filtered by q ≤ 0.05 and |r| ≥ 0.2 (when enough overlap).`
  - Simple mode: collapsed by default; Advanced mode: expanded by default.

* [x] Update any in-panel “How it works” key text to match the new terms (Rank score, Evidence).
* [x] Update/add dashboard-web tests for copy strings + badge rendering.

## Acceptance Criteria
* [x] UI uses the new labels/terms consistently (no “Score/Confidence/Blend” duplication in Unified v2 surfaces).
* [x] Coverage disclosure and effective interval are always visible after a run.
* [x] Co-occurrence magnitude is no longer presented as an unbounded “magnitude-like” number in list rows.
* [x] Correlation block is clearly labeled as “not used for ranking” and is collapsed in Simple mode.
* [x] `make ci-web-smoke` passes.

## Notes
Primary UI files:
- `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx`
- `apps/dashboard-web/src/features/trends/utils/candidateNormalizers.ts`
- `apps/dashboard-web/src/features/trends/components/relationshipFinder/ResultsList.tsx`
- `apps/dashboard-web/src/features/trends/components/relationshipFinder/PreviewPane.tsx`

## Validation
- 2026-02-10: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
