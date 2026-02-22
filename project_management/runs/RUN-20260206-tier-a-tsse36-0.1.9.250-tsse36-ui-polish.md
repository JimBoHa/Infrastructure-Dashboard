# RUN-20260206 — Tier A refresh: TSSE-36 statistical correctness + UI polish (`0.1.9.250-tsse36-ui-polish`)

## Summary

- Goal: close `TSSE-36` by validating the installed controller refresh (Tier A, no DB/settings reset) and capturing/viewing Trends TSSE evidence for `p/q/n/n_eff/m_lag` semantics.
- Result: PASS. Installed controller upgraded to `0.1.9.250-tsse36-ui-polish`; installed health smoke passed; TSSE screenshot evidence captured and viewed.

## Preconditions

- `curl -fsS http://127.0.0.1:8800/healthz` -> `{"status":"ok"}`
- `curl -fsS http://127.0.0.1:8000/healthz` -> `{"status":"ok"}`
- No DB/settings reset performed.
- Rollback target prepared from current installed config before refresh:
  - version: `0.1.9.249-derived-builder-guardrails`
  - bundle path: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.249-derived-builder-guardrails.dmg`

## Build + refresh (Tier A)

### Installed versions

- Before: `current_version=0.1.9.249-derived-builder-guardrails`, `previous_version=0.1.9.248-derived-of-derived`
- After: `current_version=0.1.9.250-tsse36-ui-polish`, `previous_version=0.1.9.249-derived-builder-guardrails`

### Commands

- `python3 tools/rebuild_refresh_installed_controller.py --version 0.1.9.250-tsse36-ui-polish --output-dir /Users/Shared/FarmDashboardBuilds_tsse36 --allow-dirty --post-upgrade-health-smoke`
  - Built DMG: `/Users/Shared/FarmDashboardBuilds_tsse36/FarmDashboardController-0.1.9.250-tsse36-ui-polish.dmg`
  - Bundle log: `/Users/Shared/FarmDashboardBuilds_tsse36/logs/bundle-0.1.9.250-tsse36-ui-polish.log`
  - Note: setup-daemon `/api/upgrade` HTTP call timed out at 120s, but underlying upgrade continued and completed; version polling/health checks confirmed success.
- `make e2e-installed-health-smoke`
  - Result: `e2e-installed-health-smoke: PASS`

## TSSE UI evidence (captured + viewed)

- Captured under:
  - `manual_screenshots_web/tier_a_0.1.9.250-tsse36-ui-polish_20260206c/`
- Viewed evidence:
  - `manual_screenshots_web/tier_a_0.1.9.250-tsse36-ui-polish_20260206c/tsse_relationship_panel_correlation_stats_key.png`
    - Verified visible semantics text in Related Sensors/Correlation panel:
      - `Stats: p is per-test, q is FDR-adjusted, n is overlap, n_eff adjusts for autocorrelation`

## Process/tooling improvements made during run

- Hardened Tier-A helper script:
  - `tools/rebuild_refresh_installed_controller.py`
  - Added wrapped setup-daemon status parsing (`result` envelope handling).
  - Added robust version inference for suffix versions and fallback `dev-<timestamp>`.
  - Added explicit installed-version polling after upgrade trigger (no reliance on long-lived HTTP response).
  - Added optional installed health smoke execution.
  - Added retention-based external artifact pruning options.
  - Added speed-oriented options for repeat refreshes:
    - `--reuse-existing-bundle` (skip rebuild if target DMG already exists)
    - `--farmctl-skip-build` (pass through `farmctl bundle --skip-build`)
- Updated screenshot automation to target current Trends UI structure:
  - `apps/dashboard-web/scripts/web-screenshots.mjs`
  - Replaced stale `<details>/<summary>` assumptions with `CollapsibleCard`/heading selectors.
  - Added panel-scoped screenshot support and resilient fallback behavior.

## One-time external artifact cleanup (requested)

- Before cleanup:
  - `/Users/Shared/FarmDashboardBuilds`: `25G`
  - `/Users/Shared/FarmDashboardBuilds_tsse36`: `316M`
- Cleanup performed:
  - Deleted old controller DMGs in `/Users/Shared/FarmDashboardBuilds`, keeping newest 20 (`deleted_controller_dmgs=59`).
  - Deleted old logs in `/Users/Shared/FarmDashboardBuilds/logs` older than 14 days (`7` files).
- After cleanup:
  - `/Users/Shared/FarmDashboardBuilds`: `7.0G`
  - `/Users/Shared/FarmDashboardBuilds_tsse36`: `316M`

### Additional one-time external cleanup (same closeout pass)

- Removed stale sibling artifact directories and root-level orphan DMGs that were outside the active bundle path:
  - Deleted: `/Users/Shared/FarmDashboardBuilds_TierA/`
  - Deleted: `/Users/Shared/FarmDashboardBuildsDirty/`
  - Deleted: `/Users/Shared/FarmDashboardController-0.1.9.14.dmg`
  - Deleted: `/Users/Shared/FarmDashboardController-0.1.8.4.dmg`
- Post-cleanup external artifact inventory:
  - `/Users/Shared/FarmDashboardBuilds`: `7.0G`
  - `/Users/Shared/FarmDashboardBuilds_tsse36`: `316M`
  - (No remaining stale FarmDashboard build dirs under `/Users/Shared` outside these active paths.)

## Closeout revalidation (post-cleanup)

- `cargo test --manifest-path apps/core-server-rs/Cargo.toml` -> PASS
- `make ci-web-smoke` -> PASS

## Evidence summary

- Installed version before: `0.1.9.249-derived-builder-guardrails`
- Installed version after: `0.1.9.250-tsse36-ui-polish`
- Installed health smoke: PASS
- Screenshot (VIEWED):
  - `manual_screenshots_web/tier_a_0.1.9.250-tsse36-ui-polish_20260206c/tsse_relationship_panel_correlation_stats_key.png`
