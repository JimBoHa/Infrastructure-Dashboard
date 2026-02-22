# TSE-0009: Candidate generation (Qdrant + filters + recall safeguards)

Priority: P0
Status: In Progress (tracked as TSSE-10 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Given a focus sensor and a max horizon, produce a high-recall candidate set quickly using Qdrant, then hand off to exact episodic verification.

## Scope
- Candidate query modes:
  - same node vs all nodes
  - same unit/type filters (configurable)
  - exclude selected sensors
- Recall safeguards (required):
  - union of multiple embedding searches (value + derivative + event)
  - minimum candidate pool sizes
  - adaptive widening within the ANN stage when results are low-confidence (do **not** introduce a separate “scan Postgres/Timescale” fallback path)
- Output:
  - stable ordered candidate list with reasons (which embedding matched)

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable (API): candidate generation endpoint contract.
- Deliverable (ANN): query strategy + HNSW params + recall tests.
- Deliverable (Perf): measure candidate generation time under load.

## Acceptance Criteria
- Candidate generation returns in <250ms for typical queries (local).
- Candidate list size is configurable and defaults to a safe high-recall value.
- Recall evaluation meets agreed target on curated test pairs.
