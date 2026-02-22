# Production Environment Guide

This guide walks through deploying Farm Dashboard components (core server, node agents, dashboard web, analytics feeds, iOS) into a production environment with TLS, secrets, and monitoring. It assumes familiarity with launchd, service management, and basic networking.

If you are setting up a brand new controller host, start with the installer-based runbook:
- `docs/runbooks/core-server-production-setup.md`

## Prerequisites
- Domain + TLS termination (nginx/Caddy/Traefik or a cloud LB).
- PostgreSQL/TimescaleDB, Redis, and Mosquitto (broker) reachable by core server and node agents (installer/launchd or managed services).
- Secrets management for database/MQTT credentials, JWT/auth tokens, and feed API keys.
- macOS build host (or CI runner) with Xcode for the iOS app.

## Infrastructure Bring-up
1) Use the installer/launchd stack unless you have managed services.
2) Record connection info (DB URL, MQTT host/port, Redis port).
3) Run migrations on the target DB: `make migrate`.
4) Seed only if you want demo data for smoke tests: `make seed` (otherwise skip).

## Core Server Deployment
1) Configure environment (see `apps/core-server-rs/.env.example` and `docs/DEVELOPMENT_GUIDE.md` for keys):
   - DB URL, Redis URL, MQTT broker URL, auth signing key/secret, demo mode **off** (`CORE_DEMO_MODE=false`).
   - Utility feed API bases/tokens if using analytics adapters.
   - If using schedule guards that depend on forecasts/indicators:
     - Enable `CORE_ENABLE_FORECAST_INGESTION=true` and set `CORE_FORECAST_LATITUDE` / `CORE_FORECAST_LONGITUDE`.
     - Enable `CORE_ENABLE_INDICATOR_GENERATION=true` (optional: tune `CORE_INDICATOR_POLL_INTERVAL_SECONDS`).
2) Start the API:
   - Launchd (installer): run the bundled `core-server` via the generated LaunchDaemon.
   - Manual (advanced only): run the bundled binary at `/usr/local/farm-dashboard/bin/core-server`.
3) Validate: `curl /healthz`, `curl /api/auth/login` (with a real user), and `curl /api/analytics/status`.

## Dashboard Web
Controller installs (recommended): the installer provisions and starts the dashboard via launchd; no manual build/run steps required.

Custom deployments (advanced): run the dashboard server with a server-side API proxy or a reverse proxy so browser requests hit the same origin.
1) Set `FARM_CORE_API_BASE` to the public core server URL (HTTPS) for Next.js rewrites (or route `/api/*` to core-server at the ingress).
2) Build: `npm ci && npm run build`.
3) Serve: `npm run start` behind your ingress.
4) Smoke-test: nodes page, analytics cards, backups modal, and adoption wizard against the live API.

## Node Agents
**Production policy:** Raspberry Pi 5 nodes are deployed via the dashboard **Deployment → Remote Pi 5 Deployment (SSH)** flow only.

1) Flash a clean Raspberry Pi OS Lite (64-bit) image, enable SSH, boot the Pi on the LAN.
2) From the dashboard, run the Remote Pi 5 deployment job (see `docs/runbooks/pi5-deployment-tool.md`).
3) Adopt the node from the dashboard (scan/adopt) and apply sensor config from the dashboard (no SSH edits).
4) Verify: `curl http://<node>:9000/v1/status` and confirm the node appears in the dashboard Nodes tab.

## Analytics Feeds
1) Configure analytics feeds via Setup Center (see `docs/analytics_feeds.md`). Today this is primarily Emporia cloud ingest plus forecast-backed sensors.
2) Ensure outbound HTTPS connectivity from the core server (Emporia + forecast providers).
3) Validate via `GET /api/analytics/feeds/status` and `GET /api/analytics/power`.

## Backups and Retention
- Core server: configure backup destination (object storage or disk), retention policy, and scheduled job; test `/api/backups` and `/api/restore`.
- Node agent: ensure `/v1/config/restore` works and SD card imaging scripts are available for break-glass recovery.

## Observability and Security
- Enable TLS at the edge; restrict MQTT to authenticated clients; rotate auth tokens regularly.
- Ship logs to your aggregator; expose Grafana dashboards for TimescaleDB + MQTT + node agent metrics.
- Configure alarms/webhooks for offline nodes, failed analytics feeds, and backup failures.

## Release / Validation Checklist
- `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke` then `make e2e-web-smoke` (local) before promoting a build; this boots Sim Lab and runs the Playwright adoption smoke against the installed bundle. Use `make ci` for full unit/integration coverage if time allows.
- Core: health check, auth login, analytics status, backups list/restore dry-run.
- Dashboard: adoption wizard, backups modal, analytics cards render with live data.
- Node agent: BLE provisioning, mesh join window opens, `/v1/status` reports expected intervals and MACs.
- Analytics: feeds report `ok` in `/api/analytics/status`, and data lands in TimescaleDB.
