# Farm Dashboard — Related Sensors (Unified v2) — Critical Review Packet (Data Scientist)

You are reviewing the **Related Sensors** feature inside **Trends**. You are **not expected to read code**. This packet is designed to be **fully self‑contained** and **unambiguous** about how the system works internally.

If you think the *product intent* or *user contract* is wrong/underspecified, that is part of the review: propose concrete replacements. The **internal algorithm facts** below should be treated as ground truth.

---

## What we need from you (deliverables)

Please return a written review with:

1) **Product contract for “Related” (recommended wording + rules)**
   - A precise definition of what “related” is allowed to mean for operators.
   - Whether inverse relationships should count as “related” and how to represent them.
   - What mistakes are most costly (false positives vs false negatives) and why.

2) **Misleadingness / UX risk assessment**
   - What users will *incorrectly infer* from current labels like “Score” and “Confidence”.
   - Proposed copy changes (exact text you recommend).
   - Any UI changes needed to prevent probability / statistical‑significance misreadings.

3) **Method fit to real sensor data**
   - Where the current method matches the domain well.
   - Where it will systematically fail (periodicity, missingness gaps, quantization, boolean states, global events, derived metrics).
   - At least 5 concrete failure modes with “what it will look like in the UI” + mitigation.

4) **Recommendations (ranked)**
   - 5–10 changes, ranked by impact vs effort.
   - Each change should specify: what it fixes, expected impact, and downsides/risks.

5) **Minimal evaluation plan**
   - A lightweight way to evaluate quality monthly (even with little/no ground truth).
   - Metrics you recommend (e.g., precision@k, stability, operator time‑to‑diagnosis).

---

## Assumed intent & persona (you may revise)

### Primary goal (given by us; please refine)
**Troubleshooting / root‑cause discovery** for technical operators using an intuitive, guided, semi‑automatic workflow.

### Primary personas (assumed)
- **Technical operator / farm tech**: not a statistician; uses this as “investigation leads.”
- **Power user / engineer**: can use Advanced controls when needed.

### Intended interpretation contract (assumed)
- “Related” means: *this sensor shows change events that align in time with the focus sensor* (possibly with lag), and/or *tends to spike in the same time buckets as the focus sensor*.
- “Related” does **not** mean: causality, probability, or guaranteed mechanistic linkage.

If you disagree, propose the correct user contract and how the UI should communicate it.

---

## What the UI shows (current UX contract)

This section states the **observable UI text/behavior** that matters for misleadingness.

### Where the feature lives
**Dashboard → Trends → Related Sensors** panel (titled **“Related Sensors”**).

### Modes (operator vs expert)
- **Simple mode**
  - Button label: **“Refresh suggestions”**
  - Auto-runs quick suggestions when the panel becomes eligible (focus sensor + time window available).
  - Shows a **Related Sensors Correlation Matrix** block first (details below), then the ranked list + preview.
  - If the last run was “quick”, a second button appears: **“Refine results”** (runs with a larger candidate_limit/max_results).
- **Advanced mode**
  - Button label: **“Run analysis”**
  - Exposes full scoring controls (weights, thresholds, etc.).

### Scope & filters (visible in both modes)
- **Focus sensor** dropdown (must pick a focus sensor).
- **Scope** dropdown: `All nodes` vs `Same node`.
- Checkboxes:
  - `Same unit`
  - `Same type`
  - `Exclude provider sensors`

### Results list (what’s shown per candidate)
Each candidate row shows:
- `#rank`
- Sensor label + node context (if known)
- A right‑side label **“Score”** with the numeric value (for Unified: blended score)
- Up to 5 small pill badges, typically including:
  - `Blend: <number>`
  - `Confidence: high|medium|low`
  - `Events: <events_score>` (when present)
  - `Co-occur: <cooccurrence_score>` (when present)
  - `Lag: +Xm / −Xm / +Ys` (when present)

### Preview pane (what the user sees when clicking a result)
The preview pane includes:
- A “Why this sensor is related” card with:
  - **Event score** (events evidence; 2 decimals)
  - **Co-occurrence** (cooccurrence evidence; 1 decimal)
  - **Event overlap** (integer)
  - **Shared buckets** (integer)
  - Optional bullet “summary” lines from the backend (free text evidence)
  - Optional “Top co-occurrence timestamps” pills (up to 5)
- An Episodes list (if episodes exist)
- A chart with:
  - Default view: **Normalized** (z-score normalization for visualization)
  - Toggle: **Raw** (actual units; independent axes)
  - Toggle: **Align by lag** (shifts the candidate timeline by the detected lag for display)
  - Context window selector: Auto / Episode / ±1h / ±3h / ±6h / ±24h / ±72h / Custom
  - A footer showing **“Preview bucket size: <duration>”**

### In-panel “How it works” key (shown in UI)
The panel includes explanatory text stating (paraphrased):
- Blend score combines event alignment + co-occurrence into one rank score.
- Confidence tiers High/Medium/Low are based on score and evidence coverage.
- Interpretation: leads, not proof of causality; daily cycles/sparse data can mislead; check Raw preview + units.

---

## Internal workings (facts; no ambiguity)

This section is the **source of truth** for algorithm behavior.

### 0) Vocabulary
- **Bucket**: a fixed time interval of length `interval_seconds` (seconds).
- **Bucket timestamp**: the bucket’s start time, aligned to Unix epoch boundaries.
- **Delta**: difference between consecutive bucket values for a sensor.
- **Event**: a delta whose robust z-score magnitude exceeds `z_threshold`.

---

## 1) Candidate pool selection (what sensors are considered)

### 1.1 Candidate pool in the dashboard UI
Given a chosen focus sensor `F`, the UI forms a candidate list by:
1) Taking **all sensors** already loaded in the Trends page (excluding the focus sensor).
2) Applying the selected scope/filters:
   - If Scope = `Same node`, keep only sensors with the same `node_id` as focus.
   - If `Same unit`, keep only sensors with the same `unit` as focus.
   - If `Same type`, keep only sensors with the same `type` as focus.
   - If `Exclude provider sensors`, drop sensors with `config.source == "forecast_points"`.
3) Sorting remaining candidates by `sensor_id` (lexicographic).
4) Taking the **first N** sensors where `N = candidate_limit`.

Important consequence:
- Candidate inclusion is **not “smart”** (no embedding/ANN stage in Unified v2). If there are more eligible sensors than `candidate_limit`, the ones after the cutoff are **not evaluated**.

### 1.2 Candidate filtering on the backend
The backend also receives filters and candidate IDs. It re-sorts/deduplicates candidate IDs and truncates to `candidate_limit` again.

The backend supports additional filters (not necessarily surfaced in UI today):
- `is_derived` (source == `"derived"`)
- `is_public_provider` (source == `"forecast_points"`)
- `interval_seconds` exact match
- explicit `exclude_sensor_ids`

---

## 2) Bucketing / resampling (how time-series are made comparable)

All analysis stages operate on **bucketed values**.

### 2.1 Bucket boundaries (exact definition)
For a raw metric point with timestamp `ts` (epoch seconds), and bucket size `Δt = interval_seconds`:

bucket_epoch = floor(epoch(ts) / Δt) * Δt

Bucket timestamp is `bucket_epoch` converted back to a datetime.

This means buckets are aligned to **epoch zero**, not to the analysis window start.

### 2.2 Bucket aggregation (Related Sensors Unified v2)
For Related Sensors Unified v2 (events + co-occurrence), **bucket value is always the mean** of raw values in the bucket:

x_bucket = average(value)

No bucket values are forward-filled/interpolated. If a bucket has no raw points, **it does not exist** in the bucketed series.

### 2.3 Handling of missingness / gaps
- Missing buckets are not synthesized.
- Deltas/events are computed over **consecutive returned buckets**; therefore, a long time gap can produce a delta across the gap (if both endpoint buckets exist).

### 2.4 Quality flags
Raw metrics include a `quality` field in storage, but Related Sensors analysis uses only bucketed `value` and does **not** filter by quality flags.

### 2.5 Interval auto-increase for long windows (max_buckets clamp)
Some analysis jobs automatically increase the effective bucket size if the requested window would produce too many buckets.

Let:
- requested bucket size = `interval_seconds`
- window duration in seconds = `horizon_seconds = (end − start)`
- expected buckets = `ceil(horizon_seconds / interval_seconds)`
- maximum allowed buckets = `max_buckets`

If `expected_buckets > max_buckets`, the job sets:

interval_seconds := ceil(horizon_seconds / max_buckets)

This matters because several controls are expressed in **buckets** (e.g., `max_lag_buckets`, `tolerance_buckets`, `min_separation_buckets`), so their meaning in **seconds** depends on the effective interval.

Concrete defaults used by the shipped jobs:
- Unified v2 event/co-occurrence stages set `max_buckets` to **8,000** when `quick_suggest=true`, otherwise **16,000**.
- Correlation matrix job default `max_buckets` is **10,000** (UI does not override it today).

### 2.6 Derived sensors and provider sensors (forecast_points)
- **Raw sensors** are read directly from the analysis lake (Parquet) and bucketed via DuckDB.
- **Derived sensors** are computed at query time from other sensors’ bucketed values:
  - A derived output bucket exists only when required input buckets exist (no implicit forward-fill/interpolation).
  - Transitive derived dependencies are expanded (cycle-detected) up to a fixed maximum depth.
- **Provider/forecast sensors** with `config.source == "forecast_points"` are not stored in the analysis lake:
  - When requested as direct outputs in analysis jobs, they are **silently skipped** (no bucket rows), so they generally cannot contribute evidence to Related Sensors ranking.
  - They may still appear indirectly if used as inputs to derived sensors.

---

## 3) Event detection (core transformation)

For each sensor, we compute events from bucketed values by:

### 3.1 Delta series
Given bucketed values `(t1, x1), (t2, x2), …, (tN, xN)` for one sensor:

Δᵢ = xᵢ − xᵢ₋₁  for i = 2..N

Each delta is timestamped at the **current** bucket time `tᵢ` (i.e., “the transition into bucket i”).

Non-finite values are skipped: if `xᵢ` or `xᵢ₋₁` is not finite, that delta is ignored.

### 3.2 Robust center & scale of deltas
Let `Δ` be the list of delta values for the sensor in the window.

Center:
  c = median(Δ)

MAD:
  MAD = median(|Δ − c|)

Scale selection:
- If MAD > 1e-9:
  s = 1.4826 * MAD
- Else (degenerate MAD), use IQR fallback:
  IQR = Q₀.₇₅(Δ) − Q₀.₂₅(Δ)
  s = IQR / 1.349   (if IQR > 1e-9)
- Else (still degenerate):
  s = 1.0

### 3.3 Robust z-score and event threshold
For each delta:

zᵢ = (Δᵢ − c) / s

Event condition:
|zᵢ| ≥ z_threshold

Polarity filter:
- `both`: keep events for positive and negative z
- `up`: keep only z ≥ 0
- `down`: keep only z ≤ 0

### 3.4 De-duplication by minimum separation
Events are sorted by time. If two events occur within:

min_sep_seconds = min_separation_buckets * interval_seconds

…then only the event with the larger |z| is retained.

This is a greedy forward pass: it compares each event to the last kept event.

### 3.5 Max-events truncation
If more than `max_events` remain:
- keep the `max_events` events with largest |z|
- then re-sort by time

---

## 4) Evidence signal #1 — Event alignment scoring (F1 overlap with best lag)

This stage asks: “Do the focus sensor’s events line up with the candidate’s events if we allow a lag?”

### 4.1 Event time sets
For focus sensor F:
  T_F = set of event timestamps (epoch seconds)
For candidate C:
  T_C = set of event timestamps (epoch seconds)

### 4.2 Overlap at a given lag
Lag is expressed in buckets `L`:
  lag_sec = L * interval_seconds

Overlap count:
  overlap(L) = | { t ∈ T_F : (t + lag_sec) ∈ T_C } |

Important: This is **exact timestamp equality** after shifting by `lag_sec`.

### 4.3 F1-style score at lag L
Let n_F = |T_F| and n_C = |T_C|:

F1(L) = 2 * overlap(L) / (n_F + n_C)

If one side has 0 events, score is 0 (or undefined if both are 0; treated as no evidence).

### 4.4 Best-lag search
Evaluate all integer lags:
  L ∈ [−max_lag_buckets, +max_lag_buckets]

Pick the lag with maximum F1(L). Tie-breaker: higher overlap count wins.

Outputs per candidate:
- `events_score` = best F1(L)
- `events_overlap` = overlap at best lag
- `best_lag_sec` = best lag in seconds

### 4.5 Episodes (explainability)
Matched focus events (those that have a candidate match at best lag) are grouped into episodes:
- Sort matched focus events by time.
- Split when gap between consecutive matched events exceeds:
  gap_seconds = episode_gap_buckets * interval_seconds

For each episode:
- start/end timestamps
- `num_points` (matched events)
- `score_mean` = mean(|z|) of focus events in the episode
- `score_peak` = max(|z|)
- `coverage` = num_points / total_focus_events

Episodes are sorted by `score_peak` descending and truncated to `max_episodes`.

---

## 5) Evidence signal #2 — Co-occurrence scoring (shared anomaly buckets)

This stage asks: “Do the focus sensor and the candidate sensor tend to have events in the same time buckets (within tolerance)?”

### 5.1 Bucket index for each event
For an event time `t` (epoch seconds):

b = floor(t / interval_seconds)

### 5.2 Tolerance expansion (±τ buckets)
With tolerance τ = `tolerance_buckets`, an event contributes to bucket indices:

{ b + o : o ∈ [−τ, +τ] }

Multiple events from the same sensor that land in the same expanded bucket index are collapsed: the event with larger |z| is kept for that sensor in that bucket index.

### 5.3 Candidate “group buckets”
For each bucket index, we have a set of sensors that had an event (after tolerance expansion).
Keep only buckets where:
- number of sensors ≥ `min_sensors`
- and (when focus is specified) the focus sensor is included

### 5.4 Bucket severity score (used to pick top buckets)
For a bucket containing sensors S with event z-scores z_s:

severity_sum = Σ_{s ∈ S} |z_s|
pair_weight = C(|S|, 2) = |S|(|S|−1)/2
bucket_score = pair_weight * severity_sum

The co-occurrence job selects the top `max_results` buckets by bucket_score, with suppression:
after selecting a bucket at index b, it suppresses indices in [b−τ, b+τ] from being selected.

### 5.5 Focus-weighted per-sensor aggregation (Unified v2 evidence)
Unified v2 aggregates co-occurrence evidence for each candidate sensor C using only selected buckets that include the focus sensor F:

For each selected bucket that contains F and C:
  cooccurrence_score(C) += |z_F| * |z_C|
  cooccurrence_count(C) += 1

It also records up to 10 most-recent bucket timestamps for UI context.

Outputs per candidate:
- `cooccurrence_score`
- `cooccurrence_count`
- `top_bucket_timestamps`

---

## 6) Unified v2 blend (final ranking)

Unified v2 merges the two evidence signals above and ranks candidates.

### 6.1 Component normalization (pool-relative)
Event alignment scores are in [0, 1]. Co-occurrence scores are unbounded positive.
Unified v2 normalizes each component by the **maximum positive finite score within the current candidate pool**:

E_max = max(events_score over candidates where events_score > 0)
K_max = max(cooccurrence_score over candidates where cooccurrence_score > 0)

events_norm(C) = clamp(events_score(C) / E_max, 0, 1)  (if E_max <= 0 ⇒ 0)
coocc_norm(C) = clamp(cooccurrence_score(C) / K_max, 0, 1)  (if K_max <= 0 ⇒ 0)

Important consequence:
- The blended score is **relative to the candidate pool**. Changing the pool can change E_max/K_max and thus the normalized scores.

### 6.2 Weights
Weights are user-controlled:
- `eventsWeight` (default 0.6)
- `cooccurrenceWeight` (default 0.4)

The backend renormalizes weights to sum to 1:
  w_E = eventsWeight / (eventsWeight + cooccurrenceWeight)
  w_K = cooccurrenceWeight / (eventsWeight + cooccurrenceWeight)

### 6.3 Blended score
blended(C) = w_E * events_norm(C) + w_K * coocc_norm(C)

Candidates with blended(C) <= 0 or non-finite are dropped.

### 6.4 Confidence tiers (heuristic, not statistical)
Confidence is derived from blended score and minimal evidence counts:

- High if blended ≥ 0.75 AND (events_overlap ≥ 2 OR cooccurrence_count ≥ 2)
- Medium if blended ≥ 0.35 AND (events_overlap ≥ 1 OR cooccurrence_count ≥ 1)
- Low otherwise

If `include_low_confidence` is false, Low candidates are omitted.

### 6.5 Sorting / ranking
Candidates are sorted by:
1) blended_score descending
2) confidence tier (High > Medium > Low)
3) cooccurrence_count descending
4) events_overlap descending
5) sensor_id ascending (stable tie-break)

Top `max_results` are returned; remainder sensor IDs are recorded as truncated.

### 6.6 Backend clamps (important edge conditions)
The backend applies guards/clamps even if the UI provides values. Key ones:

- Unified v2:
  - `candidate_limit` is clamped to **[10, 1000]**.
  - In **Simple** mode with `quick_suggest=false` (e.g., “Refine results”), `candidate_limit` is additionally capped at **300**.
  - `max_results` is clamped to **[5, 300]**.
- Event match:
  - `max_lag_buckets` is clamped to **[0, 360]**.
  - `max_events` is clamped to **[100, 20,000]**.
  - `max_episodes` is clamped to **[1, 200]**.
- Co-occurrence:
  - `tolerance_buckets` is clamped to **[0, 60]**.
  - `max_results` is clamped to **[1, 256]**.
- Unified v2 co-occurrence stage also caps total sensors considered:
  - It sets `max_sensors = min(candidate_limit + 1, 500)`.
  - If more sensors are provided, the co-occurrence job drops the remainder after sorting by sensor_id.

---

## 7) Correlation matrix block (shown in Related Sensors panel)

The panel also computes a **Correlation Matrix** (Pearson by default) for:
- the focus sensor, plus
- up to 25 candidates with blended_score ≥ `matrixScoreCutoff` (default 0.35).

This matrix is a separate analysis job and uses different math from Unified v2 ranking.

### 7.0 Parameters used by the dashboard UI (exact)
When the matrix is computed from the Related Sensors panel, the UI submits:
- method: **Pearson**
- min_overlap: **10**
- min_significant_n: **10**
- significance_alpha: **0.05**
- min_abs_r: **0.2**
- bucket_aggregation_mode: **auto**
- max_sensors: **26** (focus + up to 25 candidates)
- interval_seconds: the Trends interval (requested; may be auto-increased by max_buckets)

### 7.1 Bucket aggregation mode (Auto)
The correlation job uses `bucket_aggregation_mode = auto`, which chooses per-sensor aggregation by sensor type string:
- If sensor type contains: `pulse`, `flow`, `rain`, `counter` ⇒ **Sum**
- If sensor type contains: `state`, `status`, `bool`, `switch`, `contact`, `mode` ⇒ **Last**
- Else ⇒ **Avg**

### 7.2 Alignment & overlap
Correlations are computed only on timestamps where both sensors have a bucket at the **exact same** bucket timestamp.
If overlap n < `min_overlap`, correlation is not computed.

### 7.3 Pearson correlation (exact)
Given aligned values (xᵢ, yᵢ), i=1..n:

r = ( n Σ(xy) − Σx Σy ) / sqrt( (n Σ(x²) − (Σx)²) * (n Σ(y²) − (Σy)²) )

### 7.4 Effective sample size n_eff (autocorrelation-adjusted)
The job computes lag‑1 autocorrelation for each series using a Pearson correlation between adjacent values:

Let the time-ordered series be v[0..m−1]. Form pairs:
- x_prev = [v[0], v[1], …, v[m−2]]
- x_curr = [v[1], v[2], …, v[m−1]]
(dropping any pair where either value is non-finite)

Then ρ₁ is Pearson(x_prev, x_curr), clamped into (−1, +1) for numerical safety.

Effective sample size is then computed conservatively as:

n_eff = floor( n / max(1, (1 + 2 * ρ₁(x) * ρ₁(y))) )

with a final clamp:

n_eff := clamp(n_eff, 3, n)

This guarantees n_eff ≤ n (it never “over-credits” negative autocorrelation).

### 7.5 Statistical filtering (FDR + effect size)
For pairs with n ≥ `min_significant_n`, it computes a p-value, then applies Benjamini–Hochberg FDR to produce q-values.
A cell is marked “OK” only if:
- q_value ≤ `significance_alpha` (default 0.05), AND
- |r| ≥ `min_abs_r` (default 0.2)

Otherwise status becomes `NotSignificant` or `InsufficientOverlap` (or `NotComputed`).

### 7.6 Pearson p-value computation (exact)
Pearson p-values are computed via a Fisher-z normal approximation using effective sample size:

z = 0.5 * ln((1 + r) / (1 − r))
se = 1 / sqrt(n_eff − 3)
z_score = |z / se|
p = 2 * SF_Normal(z_score)

where SF_Normal is the standard normal survival function. Very small p-values are floored to a tiny positive value for display clarity (not exact 0).

### 7.7 Benjamini–Hochberg q-values (exact)
Given m p-values sorted ascending as p(1) ≤ … ≤ p(m):

q_raw(i) = p(i) * m / i
q(i) = min_{j ≥ i} q_raw(j)   (monotone adjustment from the end)

q is then clamped into [0, 1].

This statistical machinery is **only** for the correlation matrix, not for Unified v2 blended ranking.

---

## 8) Preview series endpoint (what the chart actually plots)

The preview chart is loaded from an API endpoint that returns bucketed series for:
- focus sensor
- candidate sensor
- and optionally an aligned candidate series (lag shift)

### 8.1 Preview bucketing
Preview buckets are computed with the same epoch-aligned bucketing as above, using **Avg** aggregation.
Preview bucket size `bucket_seconds` is chosen to keep the number of plotted points under `max_points`:

bucket_seconds = ceil(window_seconds / max_points)  clamped to [1, 3600]

The UI requests `max_points = 800`.

Preview windows are clamped by server config:
- `CORE_ANALYSIS_PREVIEW_MAX_WINDOW_HOURS` defaults to **168 hours (7 days)**.

### 8.2 Lag alignment (display-only)
If a lag is provided, the backend returns `candidate_aligned` by shifting candidate timestamps by:

t_aligned = t_original − lag_seconds

The UI has a checkbox **Align by lag**. If enabled, it plots `candidate_aligned` unless alignment would make the series too sparse (then it falls back to raw).

### 8.3 Normalized vs Raw chart mode (display-only)
**Normalized** mode z-score normalizes each displayed series (mean/std computed over the displayed preview points):

z = (x − mean(x)) / std(x)   (sample std with denominator n−1; std floored to 1)

This visualization normalization is **not** the same as the robust delta z-scores used for event detection.

### 8.4 Endpoint fallback behavior (for completeness)
If a caller does **not** provide `episode_start_ts` / `episode_end_ts`, the preview endpoint tries to auto-select a “best episode” by scoring the last 7 days of bucketed series, and uses that as the preview window. (The dashboard UI normally provides an episode-derived window.)

---

## Default parameters (as shipped in the dashboard UI)

These are the current UI defaults (Advanced controls). Simple mode uses the same underlying values, but hides most controls.

Unified v2 defaults:
- candidateLimit: 200
- maxResults: 60
- eventsWeight: 0.6
- cooccurrenceWeight: 0.4
- includeLowConfidence: false
- polarity: both
- zThreshold: 3
- minSeparationBuckets: 2
- maxLagBuckets: 12
- maxEvents: 2000
- maxEpisodes: 24
- episodeGapBuckets: 6
- toleranceBuckets: 2
- minSensors: 2
- matrixScoreCutoff: 0.35

Simple mode “quick suggest” behavior in the UI:
- candidateLimit is reduced to min(80, candidateLimit)
- maxResults is reduced to min(20, maxResults)

---

## Review checklist (what to focus on)

1) **Semantics**
   - Is “related” closer to: shared driver, same system, same episode, correlation, causal chain?
   - Should inverse relationships be explicitly detected/marked?

2) **Misleadingness**
   - Do “Score” and “Confidence” read like probability/significance to operators?
   - Does the correlation matrix block introduce “statistics overload” or false authority?
   - Does pool-relative normalization need explicit disclosure?

3) **Method fit**
   - Event detection on deltas: good for step changes, but what about periodic sensors or quantized counters?
   - Missingness gaps creating “big deltas” across downtime: does that produce false event matches?
   - Co-occurrence bucket scoring: does it over-rank system-wide/global events?
   - Derived sensors: do they create tautological matches that should be handled separately?

4) **Operational UX**
   - Are the defaults safe for non-statisticians?
   - Is the workflow sufficiently guided and semi-automatic for troubleshooting?

---

## Output format (please follow)

Please structure your response as:

1) **Proposed user contract** (definition of “related”, inverse handling, false pos/neg priority)
2) **UI/copy changes** (exact recommended text)
3) **Top failure modes** (≥5, with symptom + cause + mitigation)
4) **Ranked recommendations** (impact vs effort)
5) **Evaluation plan** (minimal, practical)

---

## (For internal reference only) Where these facts come from

This packet describes behavior implemented in:
- Rust analysis jobs: `related_sensors_unified_v2`, `event_match_v1`, `cooccurrence_v1`, `correlation_matrix_v1`
- Bucketing: DuckDB epoch-aligned buckets; avg/last/sum aggregation modes
- Dashboard UI: Trends → RelationshipFinderPanel + PreviewPane
