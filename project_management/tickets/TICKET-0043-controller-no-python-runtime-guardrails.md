# TICKET-0043: Controller no-Python runtime guardrails

**Status:** Open

## Description
We need a hard, easy-to-verify guarantee that the **production controller (Mac mini)** runs **zero Python services** as part of the core stack.

Today the controller stack is intended to be Rust binaries + native deps (Postgres/Redis/Mosquitto) managed by launchd via the installer/farmctl path. However, the repo still contains Python services/tooling (node-agent, Sim Lab, legacy core tooling), which is creating repeated confusion (“core-server is still Python”).

This ticket adds explicit guardrails + proof so:
- A new developer can quickly confirm what runs on the controller.
- The installer/runbooks do not accidentally regress into “Python-on-controller” paths.
- CI/E2E checks enforce the “no Python on controller services” rule.

## Scope
* [ ] Define “production controller runtime” precisely (what counts as “running locally on the controller” vs artifacts shipped for remote nodes).
* [ ] Add a canonical verification checklist for an installed controller (launchd + process checks).
* [ ] Add automated guardrails so regressions are caught early (installer/health checks/CI).
* [ ] Update any docs that imply Python/uvicorn runs on the controller in production.

## Acceptance Criteria
* [ ] Production runbooks include a short “prove no Python services” section with concrete commands and expected output:
  - `launchctl print system/com.farmdashboard.core-server` shows a **binary** `ProgramArguments` (not `python`/`uvicorn`).
  - `ps aux | rg "python|uvicorn"` shows no controller services running under Python.
* [ ] `farmctl health` (or an equivalent installer-path check) surfaces an explicit warning/error if any `com.farmdashboard.*` service is running under Python/uvicorn.
* [ ] CI/E2E includes a lightweight assertion that the installed `core-server` service is the Rust binary (and does not require a Python runtime on the controller host).
* [ ] A short “policy statement” exists in docs: **Python is allowed for tests/dev tooling and remote node-agent, but not for production controller services.**

## Notes
- This ticket is about the **controller runtime on macOS**. It does *not* forbid Python on Raspberry Pi nodes (node-agent) or in dev-only simulators, but those must be clearly isolated/documented.
- Follow-up cleanup work that removes/renames legacy Python directories should be tracked separately:
  - TICKET-0044 is complete (ARCH-6) and archived: `project_management/archive/tickets/TICKET-0044-core-server-python-tooling-rename-and-prune.md`.
  - TICKET-0045 remains active: `project_management/tickets/TICKET-0045-sim-lab-and-onboarding-bloat-reduction.md`.
