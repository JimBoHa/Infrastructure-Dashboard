# TICKET-0075: Unified v2: delta correlation evidence channel (optional third signal)

**Status:** Done

## Description
Unified v2 currently ranks candidates by a blend of:
- event alignment (F1 overlap over detected delta-events), and
- co-occurrence (shared anomaly buckets).

For some continuous sensors, event detection can be noisy or sparse; a continuous **delta correlation** signal can improve ranking quality. However, correlation metrics are easy to misread as “statistical authority”, so this signal must be clearly labeled and **Advanced-only by default**.

## Scope
* [x] Backend (Advanced-only feature flag):
  - Add optional third evidence signal: `delta_corr` at best lag.
  - Compute signed Pearson correlation on aligned bucket deltas:
    - `Δ_F(t)` vs `Δ_C(t + best_lag)`
    - Use only timestamps where both deltas exist.
    - Gate on `n_pairs >= 10` (or `n_eff >= 10` if available) to avoid noisy signs.
  - Define `delta_corr_abs = |delta_corr|` in `[0,1]`.
* [x] Blending semantics (decision complete):
  - Add param `include_delta_corr_signal: bool` (default `false`).
  - When enabled, blend becomes:
    - `blended = w_events * events_norm + w_coocc * coocc_norm + w_delta * delta_corr_abs`
  - Add `w_delta` control in Advanced; renormalize weights to sum to 1 over enabled components.
* [x] UI:
  - Show `Δ corr` as an additional evidence metric in preview with explicit tooltip:
    - “Signed correlation on bucket deltas at best lag. Not statistical significance. Not used for ranking unless enabled.”
  - Add an “Include delta correlation signal” toggle (Advanced) with guardrail copy.
* [x] Tests:
  - Synthetic pairs where event match is weak but delta correlation is strong: enabling delta signal improves rank ordering.
  - Guardrail tests for `n_pairs` gating (field omitted when too sparse).

## Acceptance Criteria
* [x] Delta correlation signal is only used for ranking when explicitly enabled in Advanced mode.
* [x] UI labeling prevents misread as probability/significance.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Notes
This ticket is complementary to TICKET-0071 (directionality), which computes delta correlation for sign labeling; here we add a third *rank* component.

## Validation
- 2026-02-10: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsAdvancedMitigationControls.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
