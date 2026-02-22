# Development Guardrails (Read Before Declaring Work “Done”)

- Read `AGENTS.md` and this guardrail list before you start and before you report status.
- Run the exact end-to-end workflow you are claiming is done (e.g., `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke` then `make e2e-web-smoke`, `make demo-live` + manual flow). If you didn’t run it, say “not run yet” and why.
- If a feature is blocked on hardware access, split it into **Implement …** + **Validate … on hardware** tasks; mark the validation task `Blocked: hardware validation (...)` rather than leaving implementation work `In Progress`.
- If a command fails, **stop and decide the fix yourself** (patch migration/config vs. reset state) before continuing. Don’t push forward on a broken state.
- Surface failures immediately with remediation; don’t mark items complete until the workflow succeeds.
- Document whether a workflow was actually run. Never imply success without execution.
- Update `project_management/TASKS.md`, `BOARD.md`, and `EPICS.md` when work status changes.
