# TSSE/TSE Statistical Correctness & Meaningfulness — Tracker / Checklist

Created: 2026-01-25  
Owner: (unassigned)  
Status: Phase 0/1/2/3/4/5 complete (autocorr-adjusted inference + lag correction + FDR)

This document converts the TSSE/TSE statistics audit into an implementation-ordered, concrete checklist. It is intended to be *self-contained* (background + definitions + acceptance criteria + test evidence) so an agent can execute without re-reading long threads.

## Source / Context

- Audit writeup: `project_management/feedback/2026-01-25_tsse-tse-math-audit.md`
- Key codepaths referenced by the audit:
  - Correlation matrix job: `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs`
  - TSSE scoring core: `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`
  - Related sensors job: `apps/core-server-rs/src/services/analysis/jobs/related_sensors_v1.rs`
  - Robust stats: `apps/core-server-rs/src/services/analysis/tsse/robust.rs`
  - DuckDB bucketing: `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs`
  - Event detection: `apps/core-server-rs/src/services/analysis/jobs/event_utils.rs`
  - Event match job: `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs`
  - Matrix profile job: `apps/core-server-rs/src/services/analysis/jobs/matrix_profile_v1.rs`
  - Placeholders/stubs: `apps/core-server-rs/src/services/analysis/tsse/preview.rs`, `apps/core-server-rs/src/services/analysis/tsse/qdrant_client.rs`

## Why this exists (high-level)

TSSE/TSE currently exposes correlation p-values and confidence intervals (CIs) in multiple places. Even when formulas are implemented correctly, applying i.i.d. inference to autocorrelated time-series and computing p-values after “best-of-many-lags” selection can make the outputs statistically invalid and misleading.

This tracker prioritizes fixes that:
1) remove hidden behavioral constraints,
2) unify inference logic so matrix + TSSE scoring can’t diverge,
3) make correlation confidence semantics defensible (or explicitly heuristic),
4) ensure the UX communicates what the system is actually doing,
5) keep compute bounded for on-controller execution.

---

## Definitions (keep semantics consistent across backend + UI)

- **Bucketed series**: points are produced by DuckDB as one row per `(sensor_id, bucket_epoch)` with `avg(value)` and `count(samples)`.
- **`n` (overlap)**: number of aligned bucket points used to compute a correlation.
  - Correlation matrix: aligned by exact bucket timestamp matches.
  - Related sensors: aligned by exact bucket timestamp matches with a lag shift applied.
- **`min_overlap`**: minimum aligned buckets required to compute correlation at all.
- **`min_significant_n`**: minimum aligned buckets required to consider significance/p-value/CI meaningful (as currently implemented).
- **`alpha` / `significance_alpha`**: target p-value threshold (and CI label as `100*(1-alpha)%`).
- **`n_eff` (effective sample size)**: adjusted sample size to account for autocorrelation; should be used for inference if p-values/CIs are shown as “statistical”.
- **Lag search**: scanning many candidate lags and selecting the best correlation. This introduces **selection bias** (“look-elsewhere”).
- **Multiple comparisons**: computing many p-values across many pairs/candidates creates false positives unless corrected (e.g., BH-FDR).
- **`q_value`**: BH-FDR adjusted p-value computed across a job’s test set (matrix pairs or related-sensors candidates).
- **`m_lag`**: number of lag hypotheses evaluated during lag search (used for lag-selection correction).
- **Lag-corrected p-value (`p_lag`)**: per-candidate p-value corrected for selecting the best lag among `m_lag` tested lags (Sidak-style correction).

---

## Audit issues to address (must all be closed by this tracker)

**A. i.i.d. assumption mismatch**
- Current p-values and CIs treat time-series bucket points as independent; with autocorrelation, p-values become too small and CIs too narrow.

**B. Lag-search selection bias**
- Related sensors: lag is chosen to maximize |r|, then a p-value is computed at that selected lag without correction.

**C. Multiple comparisons**
- Correlation matrix runs many pairwise tests with no correction; related sensors may score/gate many candidates similarly.

**D. Hidden behavioral constraints**
- Related sensors lag search currently has a hard-coded minimum overlap (`MIN_OVERLAP = 10`) that can silently override user-configured thresholds.

**E. “Significance” ≠ “meaningful relationship”**
- Filtering by p-value alone allows tiny but statistically “significant” effect sizes (especially at large n) to dominate; users interpret “significant” as meaningful.

**F. Input representation undermines meaning**
- DuckDB bucketing uses `avg(value)` for all sensors; this can destroy event structure for discrete/spiky/counter-like sensors.

**G. Candidate generator limitations**
- Embeddings are mostly distribution-feature embeddings; they may not reflect temporal shape/lag relationships, affecting recall/precision tradeoffs.

**H. Placeholder modules**
- `tsse/preview.rs` and `tsse/qdrant_client.rs` are explicit `bail!(...)` stubs; ensure they are either implemented or provably unreachable in production paths.

---

## North-star acceptance criteria (global)

All items below must be true before declaring this tracker “Done”:
- No TSSE/TSE job silently overrides user significance/overlap settings; effective constraints are explicit in results and explainable in UI.
- Correlation inference (p-values/CIs) is computed via a single shared backend implementation (no drift between matrix and TSSE scoring).
- If the UI continues to show `p=` and `CI`, then:
  - inference accounts for time-series autocorrelation (via `n_eff` or an equivalent approach), and
  - lag-selection bias is corrected (or a corrected metric is shown instead), and
  - multiple comparisons are corrected (or an equivalent disclosure and correction is applied).
- If inference-grade corrections are not implemented, the product must *not* present p-values/CIs as inferential statistics (rename/relabel as heuristic and avoid “significance” claims).
- Tests exist for the new inference logic and for “no hidden thresholds” behavior.
- Performance remains bounded on controller hardware; `tsse_bench` (or equivalent) shows no unacceptable regressions.

---

## Implementation-ordered checklist (minimize context switching)

### Phase 0 — Lock semantics (prevents redo)

- [x] **TSSE-STAT-000 (P0): Decide: inference-grade vs heuristic-grade p/CI**
  - **Closes:** A, B, C, E (by establishing what “correct” means)
  - **Decision (locked):** **Inference-grade (approximate, time-series aware)**.
    - We will keep `p=` and `CI` in UI, but make them defensible by applying:
      - **autocorrelation adjustment** via `n_eff` (effective sample size),
      - **lag-selection correction** (Sidak-style) in TSSE related-sensors scoring (best-of-many-lags),
      - **multiple-comparisons correction** via BH-FDR (`q_value`) for correlation matrix (pairs) and related-sensors (candidates).
    - We will additionally require an **effect-size floor** (`min_abs_r`) so “tiny but significant” does not dominate.
    - UI wording must explicitly label these as:
      - “time-series adjusted (n_eff)”
      - “lag-corrected (m_lag)” where applicable
      - “FDR-adjusted (q)” where applicable
      - and must avoid implying causality.
  - **Rationale:** The product already exposes p-values/CIs and “significance” controls; therefore the system must not present i.i.d. inference on autocorrelated data as “significance”.

### Phase 1 — Centralize correlation inference (single source of truth)

- [x] **TSSE-STAT-010 (P0): Add a shared correlation inference module (Rust)**
  - **Closes:** drift risk across A/B/C/D/E
  - **Files (expected):**
    - new module under `apps/core-server-rs/src/services/analysis/` (exact location TBD)
    - referenced by `correlation_matrix_v1.rs` and `tsse/scoring.rs`
  - **Required API shape (minimum):**
    - compute `r` (Pearson + Spearman)
    - compute `p_value` and CI given `alpha` and chosen inference policy
    - compute/report `n` and (if applicable) `n_eff`
  - **Acceptance:** Correlation matrix + related sensors scoring call the shared functions; old duplicate helpers removed or delegated.
  - **Run log:**
    - 2026-01-26: Added `apps/core-server-rs/src/services/analysis/stats/correlation.rs` and wired:
      - `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs`
      - `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`
    - 2026-01-26: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass; existing warnings).
    - 2026-01-26: `make ci-web-smoke` (pass; existing lint warnings).

### Phase 2 — Remove hidden constraints + parameter consistency

- [x] **TSSE-STAT-020 (P0): Remove hard-coded `MIN_OVERLAP=10` from TSSE lag search**
  - **Closes:** D
  - **Files:** `apps/core-server-rs/src/services/analysis/tsse/scoring.rs`
  - **Acceptance:**
    - lag search overlap threshold is derived from request params (or a documented invariant that is surfaced to the caller)
    - result metadata/why_ranked includes the effective overlap threshold(s)
  - **Tests:** add/extend a unit test proving `min_significant_n=3` can evaluate lags with `n >= 3` (or that the enforced minimum is explicitly surfaced and documented).
  - **Run log:**
    - 2026-01-26: Updated lag-search to accept `min_overlap` from the caller (derived from `min_significant_n` clamped to `>=3`) instead of a hidden constant.
    - 2026-01-26: Added regression test `lag_search_min_overlap_is_param_driven` (allows `min_overlap=3`).
    - 2026-01-26: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass).

- [x] **TSSE-STAT-021 (P0): Ensure “effective params” are echoed back in all results**
  - **Closes:** D, E (UX clarity)
  - **Files:** `correlation_matrix_v1.rs`, `related_sensors_v1.rs`, and any other job that clamps/adjusts params
  - **Acceptance:** UI can always display the *actual* overlap thresholds and alpha used (including clamping).
  - **Run log:**
    - 2026-01-26: `correlation_matrix_v1` now echoes effective params (`method`, `interval_seconds`, `max_sensors`, `max_buckets`, `min_overlap`, `min_significant_n`, `significance_alpha`).
    - 2026-01-26: `related_sensors_v1` now echoes effective params (`interval_seconds`, `candidate_limit`, `min_pool`, `lag_max_seconds`, `min_significant_n`, `significance_alpha`).
    - 2026-01-26: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass).

### Phase 3 — Time-series aware inference (i.i.d. issue)

- [x] **TSSE-STAT-030 (P0): Implement `n_eff` (effective sample size) for autocorrelation**
  - **Closes:** A
  - **Decision (locked):** Keep compute bounded by defaulting to a **lag-1 autocorrelation** adjustment, computed per-series and reused across pairs/candidates.
    - Default formula: compute per-series autocorr `rho1` (lag-1) on the aligned bucket values (after any required normalization for the job).
    - For a pair, approximate: `n_eff = n / (1 + 2 * rho1_x * rho1_y)`, bounded to `[3, n]`.
    - (Optional later) extend to small K lags using `1 + 2 * sum_{k=1..K} rho_x(k) rho_y(k)` if performance allows.
  - **Acceptance:**
    - inference uses `n_eff` (not raw n) when producing p-values/CIs *if inference-grade semantics are chosen*
    - result surfaces `n_eff` (e.g., per-cell and/or via score components)
  - **Tests:** synthetic AR(1)-like series test where `n_eff < n` and p-values become less extreme.
  - **Run log:**
    - 2026-01-26: Added lag-1 autocorr + `n_eff` helpers in `apps/core-server-rs/src/services/analysis/stats/correlation.rs` and applied them to:
      - `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs` (`cell.n_eff`)
      - `apps/core-server-rs/src/services/analysis/tsse/scoring.rs` (`score_components.n_eff`)
    - 2026-01-26: Added unit tests covering `n_eff` bounds + p-value monotonicity under reduced `n_eff`.
    - 2026-01-26: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass).

- [x] **TSSE-STAT-031 (P1): Decide and document Pearson/Spearman p/CI formulas under `n_eff`**
  - **Closes:** A (interpretability)
  - **Acceptance:** Choose one consistent approach (t-test vs Fisher-z, etc.) and document approximation limits (ties, small n).
  - **Run log:**
    - 2026-01-26: Standardized inference in `apps/core-server-rs/src/services/analysis/stats/correlation.rs`:
      - Pearson: Fisher-z normal approximation for p-values + CI (using `n_eff` when available).
      - Spearman: t-approx p-value; Fisher-z CI approximation (documented as approximate).

### Phase 4 — Lag-search bias correction (look-elsewhere)

- [x] **TSSE-STAT-040 (P0): Correct p-values used after lag selection (related sensors)**
  - **Closes:** B
  - **Decision (locked):** Use a conservative, bounded correction:
    - compute `p_raw` using `n_eff` at the selected best lag,
    - compute `p_lag = 1 - (1 - p_raw)^{m_lag}` (Sidak-style) using the actual number of lags evaluated.
  - **Acceptance:**
    - the p-value used for gating is `p_lag` (lag-corrected) and is exposed in `why_ranked.score_components`
    - results expose `m_lag` (lag tests considered) and `n_eff`
  - **Tests:** case where uncorrected p passes but corrected p fails for many-lag search.
  - **Run log:**
    - 2026-01-26: Added lag-evaluation counting (`m_lag`) to TSSE lag search and exposed `lag_p_raw`, `lag_p_lag`, `m_lag` via `why_ranked.score_components`.
    - 2026-01-26: Added regression test `lag_selection_correction_can_flip_significance`.

### Phase 5 — Multiple comparisons correction

- [x] **TSSE-STAT-050 (P0): Add multiple-testing correction to correlation matrix**
  - **Closes:** C
  - **Decision (locked):** BH-FDR (`q_value`) across all computed off-diagonal pairs within the matrix job run.
  - **Acceptance:**
    - matrix computes and returns both `p_value` (time-series adjusted, per-pair) and `q_value` (BH-FDR adjusted across pairs)
    - `status` is driven by `q_value <= alpha` (and `|r| >= min_abs_r`) rather than by raw `p_value`
  - **UI impact:** tooltip displays `p` and `q`; cell styling/status uses `q` to avoid false discoveries from many comparisons.
  - **Run log:**
    - 2026-01-26: Added BH-FDR helper `apps/core-server-rs/src/services/analysis/stats/fdr.rs` (+ unit tests).
    - 2026-01-26: `correlation_matrix_v1` now:
      - returns `cell.q_value` and `cell.n_eff`
      - drives `cell.status` from `q_value <= alpha`
    - 2026-01-26: Dashboard tooltip updated to display `q` and `n_eff`.

- [x] **TSSE-STAT-051 (P1): Add candidate-set correction for related sensors**
  - **Closes:** C
  - **Decision (locked):** BH-FDR across candidates using their lag-corrected `p_lag`.
  - **Acceptance:**
    - related-sensors scoring produces `p_lag` for each candidate, then computes `q_value` across candidates
    - candidate significance status uses `q_value <= alpha` (and `|r| >= min_abs_r`) so the returned list is meaningful under multiple comparisons
  - **Run log:**
    - 2026-01-26: `related_sensors_v1` now:
      - computes per-candidate lag-corrected `p_lag` (Phase 4)
      - computes BH-FDR `q_value` across candidates
      - filters returned candidates by `q_value <= alpha`
      - includes `q_value` in `why_ranked.score_components`
    - 2026-01-26: Dashboard Related Sensors preview updated to show `p_raw`, `p_lag`, `q`, `n_eff`, and `m_lag`.

### Phase 6 — Make results meaningful (effect size + presentation)

- [x] **TSSE-STAT-060 (P0): Add an effect-size floor (`min_abs_r`)**
  - **Closes:** E
  - **Scope:** correlation matrix + related sensors gating should require both:
    - `|r| >= min_abs_r`
    - significance criteria (if inference-grade semantics remain)
  - **Acceptance:** tiny correlations don’t dominate output even at large n.
  - **Run log:**
    - 2026-02-06: Added `min_abs_r` params (default `0.2`) to `correlation_matrix_v1` and `related_sensors_v1`; significance now requires both `q <= alpha` and `|r| >= min_abs_r`.
    - 2026-02-06: Added unit coverage:
      - `services::analysis::jobs::correlation_matrix_v1::tests::effect_size_floor_blocks_tiny_correlation_even_with_good_q`
      - `services::analysis::tsse::scoring::tests::min_abs_r_gate_can_reject_even_when_p_is_significant`

- [x] **TSSE-STAT-061 (P1): Return computed `r` even when not significant (matrix)**
  - **Closes:** E (UX/interpretation)
  - **Acceptance:** matrix cells can show r while still indicating “not significant” via status/styling; avoids “blank matrix” confusion.
  - **Run log:**
    - 2026-02-06: `correlation_matrix_v1` now preserves computed `r` on non-significant cells; status remains `not_significant` when FDR/effect-size gates fail.
    - 2026-02-06: Dashboard heatmap now renders those cells and tooltip includes status + p/q semantics.

- [x] **TSSE-STAT-062 (P1): UI wording and labels match semantics**
  - **Closes:** A/B/C/E (misinterpretation)
  - **Files:** `apps/dashboard-web/src/features/trends/components/RelationshipsPanel.tsx`, `AutoComparePanel.tsx`
  - **Acceptance:**
    - UI labels explicitly indicate:
      - `n` (raw overlap) vs `n_eff` (effective sample size)
      - `p` (time-series adjusted, per-test) vs `q` (FDR-adjusted) in matrix and related sensors
      - `m_lag` and “lag-corrected” for related sensors
    - UI avoids causal language; treats results as discovery/exploration aids.
  - **Run log:**
    - 2026-02-06: Updated Trends TSSE surfaces (`CorrelationMatrix`, `CorrelationPreview`, `PreviewPane`, `RelationshipFinderPanel`, `candidateNormalizers`) to explicitly label `p(raw)`, `p(lag)`, `q`, `n`, `n_eff`, and `m_lag`, with tooltip semantics aligned to inference policy.

### Phase 7 — Fix input representation pitfalls (bucketing)

- [x] **TSSE-STAT-070 (P1): Add bucket aggregation modes beyond `avg(value)`**
  - **Closes:** F
  - **Files:** `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs` (+ job params/types)
  - **Acceptance:** correlation-oriented jobs can request an aggregation suitable for sensor type (avg/last/sum/min/max).
  - **Evidence:** examples showing discrete/binary sensors no longer get “smeared” correlations from averaging.
  - **Run log:**
    - 2026-02-06: Added aggregation modes (`avg|last|sum|min|max`) to DuckDB bucket reads and exposed TSSE job params `bucket_aggregation_mode` in backend/frontend contracts.
    - 2026-02-06: Added coverage test `services::analysis::parquet_duckdb::tests::bucket_aggregation_mode_sql_expr_is_stable`.

- [x] **TSSE-STAT-071 (P2): Choose defaults by sensor type/unit**
  - **Closes:** F
  - **Acceptance:** sensible defaults require no user tuning, but remain overrideable.
  - **Run log:**
    - 2026-02-06: Implemented `bucket_aggregation_mode=auto` in bucket-reader with per-sensor defaults:
      - `sum` for pulse/flow/rain/counter-like sensors.
      - `last` for state/status/bool/switch/contact/mode-like sensors.
      - fallback `avg` otherwise.
    - 2026-02-06: Added tests:
      - `test_auto_aggregation_mode_uses_sum_for_counter_like_types`
      - `test_auto_aggregation_mode_uses_last_for_state_like_types`

### Phase 8 — Improve candidate generation meaning (embeddings)

- [ ] **TSSE-STAT-080 (P2): Add shape-aware features to embeddings or add a cheap pre-score filter**
  - **Closes:** G
  - **Acceptance:** improves precision/recall tradeoff without large compute increase.
  - **Validation:** `tsse_bench` shows no major regressions; report artifact saved under `reports/**`.

### Phase 9 — Resolve/quarantine stubs (production safety)

- [x] **TSSE-STAT-090 (P1): Handle `preview.rs` placeholder**
  - **Closes:** H
  - **Acceptance:** either implement preview, or ensure it is unreachable and cannot be invoked in production routes/jobs.
  - **Run log:**
    - 2026-02-06: Removed dead placeholder module `apps/core-server-rs/src/services/analysis/tsse/preview.rs` and its export from `tsse/mod.rs`; production preview remains routed via `routes/analysis.rs`.

- [x] **TSSE-STAT-091 (P1): Handle `qdrant_client.rs` placeholder**
  - **Closes:** H
  - **Acceptance:** either implement, remove, or ensure unreachable (and document which Qdrant client is actually used).
  - **Run log:**
    - 2026-02-06: Removed dead placeholder module `apps/core-server-rs/src/services/analysis/tsse/qdrant_client.rs` and its export from `tsse/mod.rs`; production codepath uses `apps/core-server-rs/src/services/analysis/qdrant.rs`.

### Phase 10 — Validation & regression gates

- [ ] **TSSE-STAT-100 (P0): Add unit tests for inference module**
  - **Closes:** A/B/C/D/E regression risk
  - **Tests should cover:**
    - Pearson vs Spearman basics (+ ties)
    - `n_eff` behavior under autocorrelation
    - lag-search bias correction
    - multiple-testing correction (matrix)
    - effect-size floor gating

- [ ] **TSSE-STAT-101 (P1): Add integration-level validation for “no hidden thresholds”**
  - **Closes:** D
  - **Acceptance:** tests/assertions that effective params are always reflected in results.

- [ ] **TSSE-STAT-102 (P1): Dashboard build/smoke + targeted UI tests**
  - **Closes:** UI mislabeling risk
  - **Acceptance:** `make ci-web-smoke` passes and at least one targeted test asserts labels/tooltip semantics.

- [ ] **TSSE-STAT-103 (P1): Bench/perf evidence**
  - **Closes:** performance regression risk
  - **Acceptance:** `tsse_bench` (or equivalent) demonstrates bounded runtime; artifact logged under `reports/**`.

---

## Suggested execution batching (avoid context thrash)

1) Phase 0 decision + Phase 1 shared module (single focused backend context)  
2) Phase 2 parameter/threshold cleanup (stay in `tsse/scoring.rs` + job params)  
3) Phase 3/4/5 inference validity improvements (same inference module + callers)  
4) Phase 6 output/UX alignment (backend result fields + dashboard panels)  
5) Phase 7 bucketing improvements (DuckDB query layer + job params)  
6) Phase 8 embeddings (only after inference/thresholds are stable)  
7) Phase 9 stubs cleanup  
8) Phase 10 tests + perf evidence

---

## Phase 0 decisions (locked)

- **Interpretation of “significance”:** statistical inference **(approximate)**, not a pure UX heuristic.
  - We keep p-values/CIs in UI, but they must be time-series aware and corrected for selection/multiple testing to be meaningful.
- **Autocorrelation adjustment (on-controller):**
  - Implement `n_eff` using a lag-1 autocorrelation correction by default (cheap + reusable); surface `n_eff` to UI.
  - Keep the design extensible to multi-lag `n_eff` if needed later, but do not block correctness on that extension.
- **Lag-search correction (related sensors):**
  - Use Sidak-style lag correction with `m_lag = (# lags evaluated)` and compute `p_lag`.
  - Surface `p_raw`, `p_lag`, and `m_lag` in `why_ranked.score_components`.
- **Multiple comparisons correction (matrix + related sensors):**
  - Use BH-FDR and expose `q_value`.
  - Drive “significant” status from `q_value <= alpha`, not raw `p_value`.
- **UI policy:**
  - Show `p` and `q` (tooltips), label them clearly, and avoid causal language.
  - Keep CI label tied to `alpha` (per-test CI) but explicitly state that “significance” uses FDR `q` at the same threshold value.
  - Add an effect-size floor (`min_abs_r`) so “significant but tiny” does not dominate.
