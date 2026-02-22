# New Machine Setup

This doc is the “fresh clone on a new computer” checklist for Farm Dashboard development.

## Prerequisites

- Git
- Python 3.11+ and Poetry
- Node.js 20+ (npm)
- Native Postgres/Mosquitto/Redis via the installer/launchd

Optional (only needed for specific workflows):
- Playwright (web screenshot smoke tests): `cd apps/dashboard-web && npm run playwright:install`

## Clone + install

```bash
git clone <REPO_URL> farm_dashboard
cd farm_dashboard
make bootstrap
```

## Environment files (optional)

Copy templates only if you need non-default configuration:

```bash
cp apps/core-server-rs/.env.example apps/core-server-rs/.env
cp apps/node-agent/.env.example apps/node-agent/.env
cp apps/dashboard-web/.env.example apps/dashboard-web/.env.local
```

## Bring up demo mode (recommended first run)

```bash
make migrate
make seed

make core
FARM_CORE_API_BASE=http://127.0.0.1:8000 make web
```

WARNING: `make seed` is destructive to the target database. Only run it against a local/dev DB you are willing to wipe.

Open `http://localhost:3001` (or `http://localhost:3000` if free).

## Run tests

```bash
FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke
make e2e-web-smoke
make ci-smoke
```

`make e2e-web-smoke` runs against the installed bundle from the latest setup smoke run (`reports/e2e-setup-smoke/last_state.json`).

## Smoke-test screenshots

- Web (Playwright): `cd apps/dashboard-web && npm run screenshots:web`
Note: `manual_screenshots_*` paths are gitignored by default; to commit new screenshot runs use `git add -f manual_screenshots_web/<timestamp>/`.
