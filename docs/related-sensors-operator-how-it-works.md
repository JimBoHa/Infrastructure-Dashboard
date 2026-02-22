# Related Sensors (Trends) — How it works (Operator Guide)

## In plain language (≤120 words)
Related Sensors finds investigation leads: sensors that “move” around the same times as your focus sensor. We start with raw samples, bucket them into your selected Interval, compute bucket-to-bucket deltas, and turn large deltas into change events. Two signals are combined: (1) event alignment (events match after allowing a small lag/tolerance) and (2) co-occurrence (sensors that light up in the same anomaly buckets). Candidates are scored and sorted into a 0–1 Rank score and an Evidence tier (strong/medium/weak). Rank score is only relative to the sensors evaluated in this run—changing scope/candidate limit can change ranks. Evidence is heuristic coverage, not probability or statistical significance.

## Pipeline diagram
```text
Raw samples
  ↓ bucket (Interval)
Buckets (avg/last/…)
  ↓ delta
Δ series
  ↓ threshold + gap rules
Change events
  ↓ event-match (lag/tolerance) + co-occurrence (shared buckets)
Rank score + Evidence tier
```

## Warning (read this)
> WARNING:
> - **Rank score is not a probability.** It is a 0–1 score relative to the candidates evaluated in this run.
> - **Evidence is not statistical significance.** It is a heuristic “coverage” tier.
> - Results depend on the evaluated candidate pool and the effective Interval (bucket size).

## Advanced parameter tooltips (copy)
- **z threshold**: Minimum `|Δz|` for a bucket-to-bucket change to count as an event. Higher = fewer events (less noise, fewer false positives). Lower = more events (more sensitivity, more spurious matches).
- **Max lag (buckets)**: How far candidates are allowed to lead/lag the focus sensor when matching events. Larger values can surface delayed mechanisms, but increase search space and can introduce accidental matches.
- **Tolerance (buckets)**: How close two events must be (after applying lag) to be counted as “the same moment”. Larger tolerance increases overlap but can inflate evidence for loosely related sensors.
- **Candidate limit**: Max number of candidate sensors evaluated for ranking. Higher = more coverage but slower. Rank score is **pool-relative**, so changing the limit/scope can change ranks.
- **Weights (events / co-occurrence / Δ corr)**: How much each evidence channel contributes to the final Rank score. Increase **events** to emphasize aligned change events; increase **co-occurrence** to emphasize shared anomaly buckets; enable **Δ corr** for context evidence (not required for good results).

