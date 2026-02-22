# TSE-0008: Feature/embedding pipeline (robust, multi-scale signatures)

Priority: P0
Status: In Progress (tracked as TSSE-9 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Compute and continuously update robust multi-scale sensor signatures used for ANN candidate generation, without excluding true positives.

## Requirements
- Must support episodic relationships (bursty correlation).
- Must be outlier-robust.
- Must not assume correlation is stable across the full horizon.

## Scope
- Define feature sets at multiple window sizes (example): 5m, 1h, 6h, 1d, 7d, 30d, 90d.
- Robust stats per window:
  - median, MAD, robust z distribution summaries
  - quantiles (p05/p50/p95)
  - event/spike signatures (counts above robust threshold)
  - autocorrelation hints
  - optional derivative-based features
- Combine into one or multiple embeddings.
- Store embeddings in Qdrant with version tag.
- Update schedule:
  - incremental updates for recent windows
  - periodic full refresh

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable (Stats): define robust feature set + justification.
- Deliverable (Data): implement feature computation streaming from Parquet/DuckDB.
- Deliverable (ANN): embedding dimensionality, Qdrant index tuning, recall checks.

Visibility:
- Orchestrator writes a “Recall Safeguards” section: how we ensure embeddings don’t miss episodic matches.

## Acceptance Criteria
- Feature computation runs locally within resource budgets.
- Embeddings are versioned and can be rebuilt.
- Provide an evaluation script/harness (Rust) that measures candidate recall on a curated set of known-related pairs.
