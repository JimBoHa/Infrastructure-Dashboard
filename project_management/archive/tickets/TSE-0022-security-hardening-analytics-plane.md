# TSE-0022: Security hardening for analytics plane (paths, perms, authz)

Priority: P1
Status: Done (tracked as TSSE-23 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Ensure the new analytics plane (Parquet lake, DuckDB, Qdrant, jobs) is secure and safe on a single-node controller.

## Scope
- File permissions:
  - Parquet lake readable only by service user
  - Qdrant data dir perms
- Path safety:
  - prevent path traversal / arbitrary file reads
  - validate and canonicalize configured paths
- AuthZ:
  - ensure only authorized users can run heavy scans
- Abuse limits:
  - per-user job limits
  - max concurrent jobs
  - max preview window sizes

## Collab Harness (REQUIRED)
- Worker A: threat model + mitigations.
- Worker B: implementation.
- Worker C: tests.

## Acceptance Criteria
- Security review checklist completed.
- Automated tests cover path validation and authz enforcement.
