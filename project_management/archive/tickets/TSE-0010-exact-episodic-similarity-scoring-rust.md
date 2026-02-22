# TSE-0010: Exact episodic similarity scoring (robust stats + multi-window + lag)

Priority: P0
Status: Done (tracked as TSSE-11 in `project_management/TASKS.md`; audited 2026-01-24)

## Goal
Implement the core similarity algorithm that produces ranked results and episodic explanations over the user’s max horizon without requiring interval changes.

## Required Behaviors
- Episodic: repeated high-similarity windows must rank highly even if interspersed with long weak periods.
- Outlier-robust: outliers must not dominate.
- Multi-window: user range is maximum; engine searches smaller windows automatically.
- Lag-aware: allow meaningful lag search without combinatorial explosion.

## Scope
- Robust preprocessing:
  - robust center/scale (median + MAD)
  - winsorize/Huber clamp
  - optional derivative mode
- Multi-window scan:
  - evaluate a set of window sizes within max horizon
  - compute window-level similarity stats
- Episode extraction:
  - threshold + merge adjacent windows into episodes
  - compute episode metrics (strength, coverage, repeatability)
- Ranking:
  - combine episode metrics into final score
  - penalize single one-off spikes
- Lag:
  - coarse lag estimate
  - refine lag for top candidates/episodes

## Collab Harness (REQUIRED)
- Worker A (Stats): scoring formulation + parameter defaults.
- Worker B (Implementation): Rust implementation optimized for streaming and bounded memory.
- Worker C (Perf): profiling + optimization plan on Mac mini.

## Acceptance Criteria
- Produces deterministic ranked outputs for fixed inputs.
- Returns episodes with correct timestamps and lags.
- Includes “why ranked” fields explaining score components.
- Performance benchmark (see TSE-0019) meets agreed targets.
