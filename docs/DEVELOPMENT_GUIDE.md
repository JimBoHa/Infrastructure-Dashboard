# Development Guide

This guide provides a comprehensive overview of the development process for the Farm Dashboard project. It is intended for a single developer who is responsible for all aspects of the project, from development and testing to deployment and maintenance.

## 1. Project Overview

The Farm Dashboard is a platform for monitoring and controlling a farm. It consists of a backend server, a web dashboard, and an agent for IoT devices. The goal of the project is to provide a comprehensive and easy-to-use platform for farm management.

## 2. Development Workflow

The development workflow is designed to be simple and efficient for a single developer.

1.  **Set up the environment:**
    *   Clone the repository.
    *   Install the necessary dependencies for each application (see the `README.md` files in the respective `apps` directories).
    *   Ensure the native services (Postgres/Mosquitto/Redis) are running via the installer/launchd.

2.  **Pick a task:**
    *   Choose a task from the "To Do" column in the `project_management/BOARD.md` file.
    *   Move the task to the "In Progress" column.

3.  **Implement the task:**
    *   Create a new branch for the task.
    *   Write the code to implement the task.
    *   For dashboard-web UI changes, follow the UI/UX guardrails in `apps/dashboard-web/AGENTS.md` (page patterns + tokens, IA, hierarchy, component variants).
    *   Write or update unit and integration tests for the new code.
    *   Run the relevant tests (required for any code change), including the E2E flow for the affected component.

4.  **Submit the task for review:**
    *   Push the branch to the repository.
    *   Create a pull request.
    *   Review the pull request yourself to ensure that the code is clean and well-documented.

5.  **Merge the task:**
    *   Merge the pull request into the `main` branch.
    *   Move the task to the "Done" column in the `project_management/BOARD.md` file only after the E2E validation succeeds.

## 3. Project Management

The project is managed using a centralized system in the `project_management` directory.

*   **`BOARD.md`:** A high-level project board with "To Do", "In Progress", and "Done" columns.
*   **`EPICS.md`:** A file that describes the high-level epics for the project.
*   **`TASKS.md`:** A file that provides a detailed description of each task.

## 4. Testing

The project has a comprehensive test suite that includes unit, integration, and end-to-end tests.

*   **Unit tests:** Unit tests are written using the `pytest` framework for Python and the `vitest` framework for TypeScript.
*   **Integration tests:** Integration tests are written using the `pytest` framework.
*   **End-to-end tests:** End-to-end tests are written using the `Playwright` framework.

Any code change must be tested. Run the relevant suite for the affected component and the E2E flow before marking work complete. The pre-commit hook uses `tools/git-hooks/select-tests.py` to choose the correct smoke target based on staged paths.

### Clean-state preflight (mandatory for any test run)

Before running **any** smoke/E2E test (and especially before `make e2e-setup-smoke`), verify the machine is clean of orphaned Farm services/processes. If anything is still running, do not trust prior results; clean up first and only then start the test.

```bash
launchctl list 2>/dev/null | grep -i farm
ps aux | grep -E "core-server|telemetry-sidecar|farm|mosquitto|setup-daemon" | grep -v grep
hdiutil info | rg -i FarmDashboard
```

Recommended (state pollution; should be empty after one-time purge):

```bash
launchctl print-disabled gui/$(id -u) 2>/dev/null | grep -E "com\\.farmdashboard\\.e2e"
```

If either command outputs anything:
- Stop the orphaned jobs via launchd first (`launchctl remove <label>` for each `com.farmdashboard.*` job found).
- If any matching processes remain, terminate them and re-run the checks until clean.
- If `launchctl print-disabled` shows persistent `com.farmdashboard.e2e.*` override keys, purge them (one-time, requires admin): `sudo python3 tools/purge_launchd_overrides.py --uid $(id -u) --apply --backup`.
- Treat earlier test results as invalid and track/fix the underlying cleanup bug (see `project_management/archive/tickets/TICKET-0017-farmctl-uninstall-orphaned-launchd-jobs.md`).

### macOS firewall prompt (dev)

If the macOS Application Firewall is enabled, launching `core-server` (especially when binding to `0.0.0.0`) can trigger a prompt:

> “Do you want the application ‘core-server’ to accept incoming network connections?”

If nobody clicks **Allow**, other machines on your LAN may not be able to reach the controller UI/API, which can invalidate networked/manual QA. This is not a production blocker (the installer flow expects the user to click **Allow**), but it can interfere with unattended dev runs. Prefer local-only bindings for automation and click **Allow** for LAN testing.

### Installer-first E2E (repeatable “fresh installs” on one Mac)

Production installs are single-instance and system-managed (LaunchDaemons). E2E must be repeatable on a single dev Mac without reimaging or creating new macOS users.

The strategy is:
- Install into an isolated temp root per run.
- Use a dedicated E2E profile that selects random free ports and namespaces launchd labels so parallel/repeated runs don’t collide.
- Use LaunchAgents (`gui/$UID`) for E2E so the harness does not require admin.
- Finish with a clean uninstall/reset so each run leaves no installed launchd jobs behind.

Tip: `make e2e-setup-smoke` will build native dependencies unless you provide a prebuilt directory. For a faster/offline loop, set `FARM_E2E_NATIVE_DEPS=/path/to/native-deps` (e.g. from a prior local release build under `build/release-*/native-deps`).

Note: older installer/E2E runs may have left persistent launchd enable/disable override records (confusing, but not running processes). Treat these as “dirty machine state” for test runs; purge them once with `sudo python3 tools/purge_launchd_overrides.py --uid $(id -u) --apply --backup`.

**Hard gate:** `make e2e-setup-smoke` must validate the real installer/wizard flow end-to-end (wizard-driven, no test-only shortcuts that bypass launchd/DB init).

`make e2e-web-smoke` runs the Sim Lab Playwright adoption smoke against the installed bundle referenced by `reports/e2e-setup-smoke/last_state.json`.

For a faster debug loop (no Playwright), run `make e2e-installed-health-smoke` against the preserved install from `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`.

`make demo-live` runs the dev stack directly using native services (no container runtimes) and will fail fast if Postgres/MQTT are unavailable.

### Tier-A rebuild/refresh (installed controller)

When you need a fast “build → refresh the installed controller” loop (especially on a host where you cannot stop LaunchDaemons), use the running setup daemon to apply a locally-built controller bundle DMG and restart services. See `docs/runbooks/controller-rebuild-refresh-tier-a.md`.

### Rust core-server migration (parity)

During the Rust core-server migration, keep API drift contained by running:
- `make rcs-parity-smoke` (checks the Rust OpenAPI subset against the canonical spec; expands as endpoints are ported).

E2E means running the actual app or full stack (not just unit tests). For high-risk stack changes (core-server, node-agent, telemetry-sidecar, Sim Lab tooling, infra, proto), run `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke` followed by `make e2e-web-smoke` to boot Sim Lab and run the Playwright adoption flow in production mode (`CORE_DEMO_MODE=false`). For dashboard-web-only changes, run `make ci-web-smoke` locally and still validate E2E before marking work Done. If you need a manual run, start the stack with `make demo-live`, then run the adoption smoke directly with:
```bash
node apps/dashboard-web/scripts/sim-lab-smoke.mjs --no-core --no-web --api-base=http://127.0.0.1:8000 --base-url=http://127.0.0.1:3005
```
### CI test tiers (smoke vs full, plus E2E)

- **E2E (required for Done):** `make e2e-setup-smoke` validates installer install/upgrade/rollback/uninstall in an isolated temp root (wizard-driven), then `make e2e-web-smoke` runs the Playwright adoption smoke against that installed stack.
- **Smoke (default for PRs):** Fast checks intended for quick validation. Run locally with `make ci-smoke`.
- **Full (nightly or `ci-full` label):** Full regression coverage. Run locally with `make ci-full` or `make ci`.

## 5. Git hygiene (local runtime files)

Some local files are *runtime state* (timestamps, caches, device configs) and will change during normal dev/test runs. These should not be committed.

- Prefer committing `*.example.json` / `*.template.json` files and gitignoring the real runtime file.
- Example: the Node Agent reads `NODE_CONFIG_PATH` (default: `/opt/node-agent/storage/node_config.json`). In this repo:
  - `apps/node-agent/storage/node_config.json` is gitignored (local runtime state).
  - `apps/node-agent/storage/node_config.example.json` is tracked as a starting template.

### If you must keep a tracked local-only config

If a file must remain tracked but you need a persistent local override (rare), use `skip-worktree` to reduce churn:

- Mark as local-only: `git update-index --skip-worktree <path>`
- Undo: `git update-index --no-skip-worktree <path>`

Note: if the file changes on `main`, you must remove `skip-worktree` to pick up upstream updates.

## 6. Documentation

The project has a comprehensive set of documentation, including this guide, the `README.md` files in each application's directory, and the files in the `docs` directory.

All documentation should be kept up-to-date with the latest changes to the project.

## 7. Authentication and Authorization

The core server issues bearer tokens and enforces capabilities on mutations:

* The dashboard includes a `/login` screen and persists the token (no browser header injection needed for normal use).
* Obtain a token with `POST /api/auth/login` using a user's email; the response includes `token` and user info.
* Send `Authorization: Bearer <token>` on requests. Mutations without a valid token receive `401`.
* Dev convenience: `apps/dashboard-web` also supports `NEXT_PUBLIC_AUTH_TOKEN` as a fallback for local testing.
* Capability gates:
  * `outputs.command` is required for `/api/outputs/{id}/command`.
  * `config.write` is required for node/sensor/output CRUD.
  * `schedules.write` is required for schedule create/update/delete.
* Demo mode users are pre-seeded with the appropriate capabilities for local testing.

## 8. Deployment

Production installs use the single installer DMG (embedded controller bundle) and run services via launchd. Cloud deployment details are TBD.
