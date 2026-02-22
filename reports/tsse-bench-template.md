# TSSE Benchmark Report (Template)

Copy this file to a new report name (example: `reports/tsse-bench-YYYYMMDD_HHMM.md`)
and fill in the fields below. This template is the intended output format for
TSE-0019 perf + scale benchmarks on the Mac mini.

## Run metadata
- Date (UTC):
- Operator:
- Host (model/RAM/disk):
- macOS:
- Repo commit:
- Core-server version:
- Qdrant version:
- Dataset generator version:

## Dataset
- Sensors: 1000
- Interval range: 1s-30s
- Horizon: 90d
- Density:
- Shards:
- Storage paths:
  - Hot lake:
  - Cold lake (if any):
  - Temp:

## Commands
- Dataset generator: `cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_generate -- ...`
- Bench harness: `cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_bench -- ...`
- DuckDB bench (optional):

## Thresholds (targets + pass/fail)
| Metric | Target | Source | Result | Pass |
| --- | --- | --- | --- | --- |
| Candidate generation latency p50 | <= 250 ms | ADR 0006 |  |  |
| Candidate generation latency p95 | <= 750 ms | TSE-0019 (initial gate) |  |  |
| Preview endpoint latency p50 | <= 250 ms | ADR 0006 |  |  |
| Preview endpoint latency p95 | <= 750 ms | TSE-0019 (initial gate) |  |  |
| Exact scoring throughput | TBD | TSE-0019 |  |  |
| End-to-end related sensors job latency p50 | TBD | TSE-0019 |  |  |
| End-to-end related sensors job latency p95 | TBD | TSE-0019 |  |  |
| CPU peak | TBD | TSE-0019 |  |  |
| RAM peak | TBD | TSE-0019 |  |  |
| Disk IO (read/write) | TBD | TSE-0019 |  |  |

## Results
### Candidate generation
- p50:
- p95:
- Notes:

### Preview endpoint
- p50:
- p95:
- Notes:

### Exact scoring throughput
- Rows/sec:
- Notes:

### End-to-end job latency
- p50:
- p95:
- Notes:

### Resource usage
- CPU peak:
- RAM peak:
- Disk IO:

## Artifacts
- Raw logs:
- JSON summary:
- Screenshots (if any):
