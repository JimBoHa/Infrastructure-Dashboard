# Dashboard Web App

Next.js (App Router + Tailwind) dashboard used for configuring nodes, sensors, users, schedules, and trends.

UI changes must follow the UI/UX guardrails in `AGENTS.md` (page patterns + tokens, IA, hierarchy, component variants) to prevent design drift as the dashboard grows.

## Prerequisites

- Node.js 20+
- Core server API running locally (`make core`) or remotely. The dashboard proxies `/api/*` to the core server via Next rewrites; set `FARM_CORE_API_BASE` to the API origin (defaults to `http://127.0.0.1:8000`).
- If auth is enabled on the core server, sign in via `/login` (the dashboard obtains a token from the core server and persists it). Dev fallback: set `NEXT_PUBLIC_AUTH_TOKEN=<token>` in `.env.local` and restart.

## Running locally

```bash
cd apps/dashboard-web
npm install
make web   # or set FARM_CORE_API_BASE in .env.local
```

Next.js will bind to `http://localhost:3000` (or 3001 if 3000 is already in use). Make sure `FARM_CORE_API_BASE` points at the core API URL (`http://127.0.0.1:8000` for local dev).

### Demo mode checklist

1. From repo root, ensure core API is running (`make core`) and the database has demo data (`make seed`).
2. Create `.env.local`:
   ```bash
   cp .env.example .env.local
   ```
3. Start the dev server: `make web`.
4. Visit `http://localhost:3001` (Next.js auto-switches when port 3000 is occupied).

## Building for production

```bash
npm run build
npm run start
```

## Testing

Vitest + React Testing Library cover critical UI flows (adoption wizard, calendar drag/drop, retention, analytics/trends helpers). Run:

```bash
npm run lint
npm test -- --run --watch=false
```

E2E smoke (required before marking work Done):
```bash
make e2e-web-smoke
```

Local CI from repo root runs this as `make ci-web`; full suite: `make ci` (iOS runs use the disposable simulator helper, set `REUSE_SIM=1` to reuse).

## Screenshot smoke tests (Playwright)

Generate full-page screenshots for each main tab for quick manual review:

```bash
cd apps/dashboard-web
npm run playwright:install
npm run screenshots:web
```

Artifacts are written to `manual_screenshots_web/<timestamp>/` at repo root (with a `manifest.json`). By default the script starts a demo core server (`CORE_DEMO_MODE=true`) and a dashboard dev server on `http://127.0.0.1:3005`.

Optional flags: `--base-url=...` `--api-base=...` `--out-dir=...` `--no-core` `--no-web`.

For Tier‑A evidence, capture and **view** at least one screenshot and reference it from a run log under `project_management/runs/`.

## Tabs & Features

- **Nodes** – cards with uptime/CPU/storage, sensor & output summaries, backup status, adoption flow for discovered nodes.
- **Sensors & Outputs** – filtering by node/type, detail drawer (configuration, alarms, trend preview), output control modal.
- **Users** – manage demo users with role capability toggles and create/delete actions.
- **Schedules** – Outlook-style weekly calendar plus visual builder for conditions/actions (Advanced JSON toggle for power users).
- **Trends** – select up to 10 series, choose stacked vs independent axes, adjust axis bounds, and export CSV.
- **Analytics** – power, water, soil, and status panels with Chart.js visualizations plus integration status badges.
- **Backups** – browse daily snapshots per node and trigger restore/download workflows.
- **Provisioning** – form builder that generates `node-agent-firstboot.json` and `node_config.json` for offline/bulk provisioning (node settings + sensor list) with validation. (`node_config.json` is runtime state; don’t commit it—start from `apps/node-agent/storage/node_config.example.json`.)
- **Connection** – edit local/cloud endpoints, run discovery scans, and view controller status.

### Data sourcing

When `CORE_DEMO_MODE=true` the core server may expose demo-only snapshot data for development/testing. In production, the dashboard reads from the core server APIs (`/api/*`) and surfaces explicit error/unavailable states when endpoints cannot be reached (no automatic demo fallback).

### Authentication & tokens

- If `CORE_DEMO_MODE=true`, mock users are embedded; mutations are permitted by default.
- In production, sign in via `/login` (calls `/api/auth/login` on the core server and stores the token locally). Dev fallback: `NEXT_PUBLIC_AUTH_TOKEN` sets a default token (browser-exposed; use only in trusted/internal deployments).
