I did a deeper, code-level audit (not just comments/docs) of the TSSE/TSE analysis stack, focused on (1) whether the math is implemented correctly, (2) whether it’s *applied* correctly, and (3) whether it’s meaningful given the product goals and time-series realities.

**Scope I reviewed (code, not docs)**
- Correlation matrix job + stats helpers: `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:403`
- Related sensors scoring core: `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:60`
- Related sensors job wiring/params: `apps/core-server-rs/src/services/analysis/jobs/related_sensors_v1.rs:320`
- Robust stats helpers: `apps/core-server-rs/src/services/analysis/tsse/robust.rs:57`
- Event-based jobs: `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs:15`, `apps/core-server-rs/src/services/analysis/jobs/cooccurrence_v1.rs:1`, and event detection `apps/core-server-rs/src/services/analysis/jobs/event_utils.rs:15`
- Matrix profile job: `apps/core-server-rs/src/services/analysis/jobs/matrix_profile_v1.rs:422`
- DuckDB bucketing (critical for “n” meaning): `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs:147`
- Noted placeholders (non-implemented): `apps/core-server-rs/src/services/analysis/tsse/preview.rs:5`, `apps/core-server-rs/src/services/analysis/tsse/qdrant_client.rs:11` (likely unused, but they exist)

---

## 1) Is the math *itself* correct?

**Correlation computation (mostly correct)**
- Pearson `r` in correlation matrix job is computed via standard one-pass sums on aligned bucket timestamps. See `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:403`.
- Spearman `rho` is computed as Pearson correlation of average-tied ranks. See `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:452` and `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:520`. This is a standard Spearman implementation.

**P-values / CIs (implemented formulas are “textbook-ish”, but mostly asymptotic/approximate)**
- Pearson p-value uses Fisher-z + Normal approximation. See `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:541` and similarly `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:364`.
  - This is not the classic t-test formulation; it’s an asymptotic approximation. It’s not a stub, but it’s not “exact for small n”.
- Spearman p-value uses the common t-approx form `t = r * sqrt((n-2)/(1-r^2))` with a Student-t CDF. See `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:596`.
  - This is also an approximation for Spearman (often used, but not exact; tie handling is not accounted for).
- CI uses Fisher-z transform (Pearson) and for Spearman explicitly reuses the Fisher-z CI as an approximation. See `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:559` and `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:615`.

**Matrix profile math is basically correct (for what it is)**
- Z-normalizes each window and uses the correlation→Euclidean-distance identity: `dist = sqrt(2m(1-corr))`. See `apps/core-server-rs/src/services/analysis/jobs/matrix_profile_v1.rs:422`.
- The “constant window” handling is heuristic (because z-normalization is undefined when std≈0), but it’s an explicit design choice, not a stub. See `apps/core-server-rs/src/services/analysis/jobs/matrix_profile_v1.rs:459`.

**Robust z + event detection are correct as robust-stat heuristics**
- MAD-based robust scale is implemented normally (MAD→σ via 1.4826; IQR fallback). See `apps/core-server-rs/src/services/analysis/tsse/robust.rs:57`.
- Event detection is based on robust-z of deltas and thresholding. See `apps/core-server-rs/src/services/analysis/jobs/event_utils.rs:15`.

---

## 2) Is the math applied correctly?

This is where the biggest issues are.

**(A) P-values/CIs are applied as if samples were i.i.d.**
All correlation p-values/CIs here treat the `n` bucket points as independent draws. That’s almost never true for sensor time series (autocorrelation, trend/seasonality, smoothing by bucketing). The immediate effect:
- p-values will often be *too small* (overstating “significance”)
- CIs will often be *too narrow* (overstating confidence)

This affects:
- correlation matrix job: `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:282`
- related sensors scoring: `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:118`

If you intend these as “heuristic confidence indicators”, it’s fine; if you intend them as statistical significance, they’re misleading.

**(B) “Look-elsewhere” / selection bias is not handled**
In TSSE related-sensors scoring:
- the system *searches over many lags* (`best_lag_search`) and then computes a p-value using the *selected best lag* correlation. See:
  - lag search: `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:427`
  - p-value after selection: `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:121`

That p-value is not valid as a “significance” probability because the lag was chosen to maximize |r|. You’d need some correction (Bonferroni-ish by number of lags searched) or a different approach (permutation/bootstrapping, holdout windows, etc.) for it to be meaningful.

Similarly, correlation matrix runs many pairwise tests (potentially hundreds or thousands) with no multiple-comparisons correction. Even if i.i.d. held (it doesn’t), raw alpha=0.05 would produce many false positives.

**(C) Related-sensors lag search silently enforces a hard minimum overlap of 10**
This is a concrete “applied incorrectly” behavior issue:

- `best_lag_search` uses `const MIN_OVERLAP: usize = 10;` and rejects lag candidates with fewer than 10 aligned points. See `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:434` and `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:477`.

But the job now exposes `min_significant_n` down to 3 and passes it into scoring (you can see it wired in `apps/core-server-rs/src/services/analysis/jobs/related_sensors_v1.rs:320`).

Meaning: if a user sets `min_significant_n = 3`, the system will still never consider lags with `n < 10` during lag search. That’s a silent, non-UI-explained constraint. It’s the same class of issue as the earlier “silent min_overlap escalation”, just in a different place.

---

## 3) Is it meaningful (product-wise), not just “mathematically implemented”?

**Correlation matrix “significance filtering” is not aligned with how people interpret it**
- The matrix hides `r` unless `p_value <= alpha` (status `ok`). See `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs:289`.
- With large `n`, even tiny effect sizes become “significant” (especially with autocorrelation inflating significance). So “significance filtering” alone does not reliably mean “meaningful relationship”.

If the product goal is “reduce noisy relationships”, an effect-size floor (e.g. require `|r| >= r_min`) is often more meaningful than p-values here.

**TSSE scoring’s use of p-values is mostly a heuristic gate**
- It’s gating candidate acceptance using `lag_p_value <= alpha`. See `apps/core-server-rs/src/services/analysis/tsse/scoring.rs:129`.
- But because of (B) selection bias + (A) autocorrelation, this gate is not a meaningful statistical guarantee; it’s just another nonlinear threshold on `|r|` and `n`.

If the UI shows `p=` and a CI, users will read this as statistically grounded. Right now it’s not grounded enough to support that interpretation without disclaimers or corrections.

**Embeddings are distribution-feature embeddings (not shape embeddings)**
- `compute_sensor_embeddings` is largely built from robust distribution stats per window + spike rates, then L2 normalized. See `apps/core-server-rs/src/services/analysis/tsse/embeddings.rs:103` and `apps/core-server-rs/src/services/analysis/tsse/embeddings.rs:224`.
- This can be meaningful as a *candidate generator*, but it is not encoding temporal alignment/phase/lag relationships. That’s OK if the follow-up scorer is strong (it is, but see above), but it means ANN recall may be biased toward sensors with similar levels/distributions, not necessarily correlated shape.

**DuckDB bucketing chooses `avg(value)`**
- Buckets are `avg(value)` per sensor per epoch-aligned bucket. See `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs:220`.
- That’s a meaningful default for many analog sensors, but it can be actively misleading for discrete/binary sensors or spiky signals (where avg destroys event structure). That flows into *all* correlation-based math, so “correctness” depends heavily on sensor type.

---

## Bottom line (audit verdict)

- The code is not “stubbed”; the math is real and generally implemented as intended.
- The biggest correctness problems are not formula typos anymore — they’re **statistical validity and interpretation**:
  - i.i.d. assumptions applied to autocorrelated time series
  - p-values computed after searching across lags (selection bias)
  - no multiple-testing correction (pairs/candidates)
  - at least one remaining **silent behavioral constraint** (`MIN_OVERLAP=10`) that conflicts with configurable significance params

If you want, next turn I can produce a prioritized “fix plan” (still review-only, no edits) that focuses on the minimum changes needed to make p-values/CIs *meaningful* (or to remove/rename them so they’re not misleading).
