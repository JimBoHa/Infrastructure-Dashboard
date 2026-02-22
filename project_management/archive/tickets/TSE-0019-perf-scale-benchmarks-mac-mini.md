# TSE-0019: Performance + scale benchmarks on Mac mini (gates + regressions)

Priority: P0
Status: In Progress (tracked as TSSE-20 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Create a repeatable benchmark suite that measures TSSE performance and prevents regressions.

## Scope
- Dataset generator (Rust) that can simulate:
  - 1k sensors
  - 1s–30s intervals
  - 90d horizon (with configurable density)
- Benchmarks:
  - candidate generation latency
  - exact scoring throughput
  - end-to-end job latency for typical scans
  - CPU/RAM/disk IO budgets
- Report artifacts stored under `reports/` (or outside repo if preferred).

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: data generator.
- Deliverable: bench harness and metrics.
- Deliverable: performance targets and acceptance thresholds.

## Acceptance Criteria
- Bench suite runs on the Mac mini and produces a stable report.
- Defined pass/fail thresholds for p50/p95 latencies.
