# TSE-0014: Relationships / Correlation matrix as an analysis job

Priority: P1
Status: In Progress (tracked as TSSE-15 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Migrate the correlation matrix computation to the analysis job framework to avoid N×metrics queries and to support long ranges/high resolution without failures.

## Scope
- Define job type: `correlation_matrix_v1`.
- Use candidate selection rules appropriate for matrix.
- Return a bounded matrix (top-N sensors or selected subset).

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: API + schema.
- Deliverable: compute strategy (avoid O(N^2) explosion; use top-K + sparse approaches).
- Deliverable: UI integration.

## Acceptance Criteria
- Matrix renders without triggering “series too large”.
- Job progress visible.
