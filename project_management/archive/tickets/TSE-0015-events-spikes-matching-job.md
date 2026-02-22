# TSE-0015: Events/Spikes matching as an analysis job

Priority: P1
Status: In Progress (tracked as TSSE-16 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Migrate “events (spikes)” similarity to the job framework with episodic interpretation.

## Scope
- Define job type: `event_match_v1`.
- Robust spike detection (z/MAD) and episodic scoring.
- Conditioning support (bins) moved server-side.

## Single-Agent (REQUIRED)
Single agent owns all deliverables; no multi-agent Collab Harness required.
- Deliverable: robust event detection design.
- Deliverable: implementation + perf.
- Deliverable: UI updates.

## Acceptance Criteria
- Event matching works at high-res long ranges without errors.
