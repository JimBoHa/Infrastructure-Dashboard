# TICKET-0045: Sim Lab and onboarding bloat reduction

**Status:** Open

## Description
Sim Lab is actively used in CI/E2E and is valuable for validating adoption and dashboard flows without physical hardware. However, it increases repo and dependency footprint (Python + Playwright + node-agent local simulation), and the default onboarding path currently pulls in more than many developers need on day one.

We need a “paved path” for new developers:
- Day 1: work on `apps/core-server-rs` and/or `apps/dashboard-web` with minimal setup.
- Explicit opt-in: Sim Lab, node-agent local simulation, full installer-path E2E.

This ticket reduces bloat/overwhelm without removing Sim Lab (unless we explicitly decide it is no longer required).

## Scope
* [ ] Split dependency/bootstrap commands so developers can install only what they need (core vs web vs node vs sim-lab).
* [ ] Make the “Sim Lab” path clearly optional in docs (but still first-class for CI/E2E).
* [ ] Update CI to avoid installing unnecessary deps for jobs that don’t require Sim Lab.
* [ ] Provide a short onboarding map that explains what runs where:
  - Controller runtime (macOS): Rust core-server + Rust sidecar + native deps
  - Nodes (Pi): Python node-agent + systemd services
  - Sim Lab (dev/CI): Python services + local simulated nodes

## Acceptance Criteria
* [ ] Repo has explicit, minimal bootstrap targets (example: `make bootstrap-core`, `make bootstrap-web`, `make bootstrap-node`, `make bootstrap-sim-lab`), and docs recommend the minimal target first.
* [ ] “Run Sim Lab” instructions are clearly labeled “dev/CI only” and do not imply production controller usage.
* [ ] CI workflows only install the heavy Sim Lab deps in jobs that actually execute Sim Lab/E2E.
* [ ] A new dev can run `make core` + `make web` without installing node-agent deps unless they opt in.

## Notes
- If we later decide to move Sim Lab to a separate repo/package, that should be tracked as a separate follow-up ticket after measuring CI/dev impact.

