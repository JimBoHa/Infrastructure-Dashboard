# TSSE Candidate Recall Eval

- Date: 2026-01-24T11:48:20.381819+00:00
- input_mode: `Api`
- Qdrant: `http://127.0.0.1:6333`
- api_base_url: `http://127.0.0.1:8000`
- Window: `2025-10-26T11:48:17.585424+00:00` → `2026-01-24T11:48:17.585424+00:00`
- Interval: `30` seconds
- match_interval: `true`
- candidate_limit: `250` (min_pool `250`)
- same_unit_only: `true`
- same_type_only: `false`
- is_derived: `—`
- is_public_provider: `—`
- focus_sample: `3`
- skipped_focuses: `0`

## Recall@K summary

| K | Mean | P10 | P50 | P90 |
| ---: | ---: | ---: | ---: | ---: |
| 10 | 1.000 | 1.000 | 1.000 | 1.000 |
| 25 | 1.000 | 1.000 | 1.000 | 1.000 |
| 50 | 1.000 | 1.000 | 1.000 | 1.000 |
| 100 | 1.000 | 1.000 | 1.000 | 1.000 |
| 250 | 1.000 | 1.000 | 1.000 | 1.000 |

## Pass/Fail gate

- mean recall@K=250 : **1.000** (min required 0.600) → **PASS**
- p10/p50/p90: `1.000` / `1.000` / `1.000`

## Candidate generation wall time (client-side)

- p50: `32` ms
- p95: `34` ms

## Sampled focus recalls

| Focus sensor | Recall |
| --- | ---: |
| `4b1e3dde7a4297de78d51a50` | `1.000` |
| `5e7be23ca4894114a8c7ca33` | `1.000` |
| `ca845b715f0f44d85264a438` | `1.000` |

## Notes

- Ground truth is derived from `--pairs-file` if provided, otherwise by grouping `--sensor-id-prefix-####` into clusters of size `--cluster-size`.
- This harness evaluates ANN candidate recall only; episodic scoring is validated separately via `tsse_bench`.
