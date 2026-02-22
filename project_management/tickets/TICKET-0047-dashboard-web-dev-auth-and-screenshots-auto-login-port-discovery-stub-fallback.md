# TICKET-0047: Dashboard-web dev auth + screenshots: auto-login, port discovery, stub fallback

**Status:** Closed (implemented via DW-202..DW-205; validated locally)

## Description
Dashboard-web debugging and screenshot capture repeatedly burn time on the same workflow issues:

- Auth is session-bound; screenshots/Playwright contexts often start unauthenticated.
- Agents fall back to manual token hacks to get unstuck.
- Tooling defaults (ports, “is a dev server already running?”) are inconsistent and cause avoidable failures (e.g., Next dev lock collisions).

This ticket turns the “debug UI + capture screenshots” loop into a self-healing, low-context workflow by adding dev-only login helpers and making the screenshot harness auto-auth + auto-discover a running dev server.

## References
- ADR: `docs/ADRs/0007-self-healing-dashboard-web-dev-auth-and-screenshot-workflows.md`
- Auth token helper: `apps/dashboard-web/src/lib/authToken.ts`
- Screenshot harness: `apps/dashboard-web/scripts/web-screenshots.mjs`

## Scope
* [x] Add a dev-only “Login as Dev” banner/button when unauthenticated (localhost/dev builds only).
* [x] Update screenshot harness to auto-auth via `POST /api/auth/login` using env-provided credentials.
* [x] Make stub-auth fallback explicit + loud (to avoid masking real regressions).
* [x] Add port auto-discovery + reuse existing dev server; default to `:3000`.
* [x] Add minimal tests/smoke checks to prevent dev-only affordances from leaking into production builds.

## Acceptance Criteria
* [x] **Human loop:** In `NODE_ENV=development`, visiting the dashboard unauthenticated shows a small “Login as Dev” affordance; clicking it logs in and navigates to the requested page without manual token copy/paste.
* [x] **Security gate:** The dev-only affordance is not visible/usable in production builds (and is restricted to `localhost`/`127.0.0.1` even in dev).
* [x] **Screenshot loop (real backend):** With a running core-server, `web-screenshots` can capture authenticated pages without manual token steps (auto-auth uses `/api/auth/login`).
* [x] **Screenshot loop (no backend):** If core-server is unreachable, the harness either (a) fails with a clear error, or (b) falls back to stub-auth only when explicitly allowed, and prints the chosen mode.
* [x] **Port behavior:** Tooling prefers `http://localhost:3000`, probes common alternatives (e.g., `:3005`), and reuses an existing dev server instead of starting a second instance.
* [x] `make ci-web-smoke` passes.

## Notes
- Keep this dev workflow low-context: avoid requiring agents to read long runbooks for routine auth/screenshot setup.
- Do not change production auth semantics or loosen capability enforcement; this is a developer experience improvement.
- Implementation tasks: DW-202..DW-205 in `project_management/TASKS.md`.
- Evidence snapshots:
  - `manual_screenshots_web/20260131_dev_auth_workflow_stub/` (stub-auth)
  - `manual_screenshots_web/20260131_dev_auth_workflow_reuse_check/` (auto-detect existing dev server; `--no-web`)
  - `manual_screenshots_web/20260131_dev_auth_workflow_login_mock3/` (auto-login via `/api/auth/login`)
