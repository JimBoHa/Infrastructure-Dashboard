# TSE-0016: Co-occurrence analysis as an analysis job

Priority: P1
Status: In Progress (tracked as TSSE-17 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Move co-occurrence computations into the job framework and unify with episodic outputs.

## Scope
- Define job type: `cooccurrence_v1`.
- Output includes episode ranges where co-occurrence is strongest.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: algorithm spec.
- Deliverable: implementation.
- Deliverable: perf/validation.

## Acceptance Criteria
- Co-occurrence does not require large client-side series fetches.
