# Farm Dashboard Documentation Hub

Use this page as the single entry point for engineering docs. It links to component guides, testing policy, and production-readiness checklists so newcomers know where to go next.

## Quick Start
- Install prerequisites: Python 3.11+ with Poetry, Node 20+, native Postgres/Mosquitto/Redis via the installer/launchd.
- Fresh machine checklist: `docs/development/new_machine.md`.
- Install dependencies: `make bootstrap`.
- Apply migrations with `make migrate`, seed demo data with `make seed` (native DB must be running).
- Dev servers: `make core` (Rust controller), `make web` (Next.js), node agent via `cd apps/node-agent && poetry run uvicorn app.main:app --reload`.
- Local E2E: run `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`, optionally sanity-check with `make e2e-installed-health-smoke`, then `make e2e-web-smoke` (boots Sim Lab in production mode and runs the Playwright adoption smoke against the installed bundle recorded in `reports/e2e-setup-smoke/last_state.json`). For unit/integration suites use `make ci-smoke` (fast) or `make ci` (full suite). The pre-commit hook chooses the right target based on staged paths.
- Auth/capabilities: see `apps/core-server-rs/src/routes/auth.rs` and `docs/DEVELOPMENT_GUIDE.md` for env vars and tokens.

## What to Read Next
- **Architecture & APIs**: `docs/analytics.md`, `docs/analytics_feeds.md`, `docs/DEVELOPMENT_GUIDE.md`.
- **Production (installer DMG)**: `docs/runbooks/core-server-production-setup.md`.
- **Tier-A rebuild/refresh (installed controller)**: `docs/runbooks/controller-rebuild-refresh-tier-a.md`.
- **Node Agent**: `docs/node-agent.md` (provisioning, mesh, validation), `apps/node-agent/README.md` (config, services), `docs/runbooks/pi5-deployment-tool.md` (production deploy-over-SSH), `docs/runbooks/renogy-pi5-deployment.md` (Renogy configuration notes), `docs/runbooks/pi5-simulator.md`.
- **Dashboard Web**: `apps/dashboard-web/README.md` (local dev, mock/demo data, testing).
- **Mobile (deferred)**: iOS/watch work is paused on `main`; see branch `freeze/ios-watch-2026q1`.
- **Development Workflow**: `docs/DEVELOPMENT_GUIDE.md`, `docs/development` (linting, style, CI contract).
- **Production**: `docs/PRODUCTION_GUIDE.md` (end-to-end deployment, TLS, feeds, monitoring).
- **Observability**: `docs/observability.md` (logging, tracing, Tempo stack).
- **Releases**: `docs/releases.md` (channels, versioning, changelog tooling).
- **Operations**: `docs/qa` (release QA checklists), `docs/runbooks` (common failure runbooks), `docs/runbooks/emporia-cloud-api.md` (Emporia API setup).
- **Sim Lab**: `docs/runbooks/sim-lab.md` (start-from-closed walkthrough).

## Production Checklist (high level)
- Core server: run migrations, set `CORE_DEMO_MODE=false`, configure MQTT/DB creds, enable auth tokens/capabilities, and wire analytics feeds (currently Emporia cloud) per `docs/analytics_feeds.md`.
- Node agents: flash a clean Raspberry Pi OS Lite (64-bit) image with SSH enabled, then deploy the node-agent via Dashboard → Deployment → Remote Pi 5 Deployment (SSH) (`docs/runbooks/pi5-deployment-tool.md`). Configure sensors post-adoption from the dashboard (no SSH edits).
- Dashboard: controller installs run via the installer/launchd; for custom deployments set `FARM_CORE_API_BASE` and serve the build (`npm run build && npm run start`) behind TLS.
- Monitoring: enable Timescale retention, Grafana dashboards, Mosquitto auth, and alarm webhooks; document restore/backups policy in the controller config (core-server-rs / installer-generated config.json).

## Common Commands
- `make migrate` / `make seed` — DB migrations and demo data (native DB required).
- `make core` / `make web` — run servers during development.
- `make ticket t="..."` — create a detailed ticket stub in `project_management/tickets/`.
- `make adr t="..."` — create an ADR stub in `docs/ADRs/`.
- `make e2e-setup-smoke` — build installer DMG and validate install/upgrade/rollback in a clean temp root.
- `make e2e-installed-health-smoke` — fast non-UI health check of the installed stack (no Playwright).
- `make e2e-web-smoke` — run Playwright adoption smoke against the installed bundle from the latest setup smoke run.
- `make rcs-parity-smoke` — Rust core-server migration: compare Rust OpenAPI subset vs the canonical spec.
- `cd apps/node-agent && PYTHONPATH=. poetry run pytest`, `cd apps/core-server-rs && cargo test`, `npm run lint && npm run test` (dashboard).

## Visual Smoke Tests

- Web UI screenshots (Playwright): `cd apps/dashboard-web && npm run screenshots:web` (writes to `manual_screenshots_web/<timestamp>/`). For Tier‑A evidence, capture and **view** at least one screenshot and reference it from a run log under `project_management/runs/`.
- Latest committed screenshots:
  - Web: `manual_screenshots_web/20251215_080355/`

## Support & Ownership
- Codeowners live in `CODEOWNERS`.
- When in doubt about platform expectations, follow the local-only CI policy in `docs/DEVELOPMENT_GUIDE.md`.
