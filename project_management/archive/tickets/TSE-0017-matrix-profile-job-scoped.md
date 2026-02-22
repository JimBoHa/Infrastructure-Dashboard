# TSE-0017: Matrix profile-like analysis as a job (scoped + safe)

Priority: P1
Status: In Progress (tracked as TSSE-18 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Redesign matrix profile analysis to run as a job with strong safeguards, since full matrix profile at 1s over long ranges can be computationally extreme.

## Scope
- Define job type: `matrix_profile_v1`.
- Scope controls:
  - explicit max compute budget
  - early stopping
  - operate on a user-selected subset or top candidates
- Episode-like motif outputs.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: algorithm choice and constraints.
- Deliverable: implementation.
- Deliverable: UX contract.

## Acceptance Criteria
- Never “locks up” the controller; job is cancelable and bounded.
