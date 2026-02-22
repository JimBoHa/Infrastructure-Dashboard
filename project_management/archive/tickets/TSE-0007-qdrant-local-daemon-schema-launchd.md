# TSE-0007: Qdrant local deployment + schema (required ANN stage)

Priority: P0
Status: In Progress (tracked as TSSE-8 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Deploy Qdrant locally on the Mac mini as a managed service and define the vector schema required for candidate generation.

## Scope
- Qdrant runs locally (launchd), with data directory under controller `data_root` (default `/Users/Shared/FarmDashboard`).
- Define collections:
  - `sensor_similarity_v1` (or versioned)
- Define payload fields to support filtering:
  - `sensor_id`, `node_id`, `type`, `unit`, `interval_seconds`, `is_derived`, `is_public_provider`, etc.
- Define embedding versioning and migration plan.
- Ensure startup order and health checks.
- Bundle/installer integration (required for production):
  - Ensure the controller bundle includes the Qdrant binary (no runtime downloads).
  - Add a launchd service entry via `apps/farmctl/src/launchd.rs` so Qdrant is installed and started with the rest of the stack.
  - Store Qdrant data under `${data_root}/storage/qdrant` and logs under `${logs_root}`.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable (Ops): launchd plist, service user, file permissions.
- Deliverable (Schema): collection config (HNSW params) tuned for Mac mini.
- Deliverable (Bench): Qdrant query latency benchmarks vs embedded ANN baseline.

## Acceptance Criteria
- Qdrant runs as a local service and survives reboot.
- Health endpoint available locally.
- Collection schema created automatically (idempotent).
- Benchmarks show candidate query latency is negligible relative to raw series verification stage.
