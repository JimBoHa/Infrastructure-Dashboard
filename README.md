# Infrastructure Dashboard Monorepo

This repository centralizes every component required to deliver the Infrastructure Dashboard platform: backend services, distributed node agents, user interfaces, infrastructure automation, and operational documentation. The structure is intentionally modular so each surface can evolve independently while still sharing tooling, workflows, and governance.

## Repository Layout

| Path | Purpose |
| --- | --- |
| `apps/core-server-rs` | **Rust controller backend** (production runtime). Serves `/api/*` and the static dashboard build in production. |
| `apps/telemetry-sidecar` | **Rust telemetry ingest sidecar** (production runtime). Subscribes to MQTT and batches telemetry into TimescaleDB/Postgres. |
| `apps/farmctl` | **Rust CLI tooling** for packaging, installer workflows, DB migrations, and operator/admin actions. |
| `apps/node-agent` | Software stack for Raspberry Pi field nodes. Handles sensor I/O, buffering, mesh/BLE provisioning, and secure synchronization with the controller. |
| `apps/dashboard-web` | Next.js-based admin dashboard for configuring nodes, visualizing trends/analytics, backups, and adoption. |
| `apps/setup-app` | macOS installer + Setup Center app (wizard UX, service bootstrap, upgrade/rollback). |
| `infra/migrations` | Database and infrastructure change sets (SQL, Terraform modules, Ansible playbooks, etc.) coordinated with CI. |
| `docs/ADRs` | Architecture Decision Records that capture the rationale for system-level choices. |
| `project_management` | Centralized project management documents, including a project board, epics, and tasks. |
| `project_management/tickets` | Long-form ticket briefs (created via `make ticket`) referenced from `project_management/TASKS.md`. |
| `tools` | Shared developer tooling such as scripts, local CLIs, lint/format configs, and Make targets. |
| `.github/workflows` | CI/CD definitions for linting, testing, packaging, and deployments across all apps. |

## Architecture Overview

```mermaid
flowchart LR
  classDef client fill:#74c0fc,stroke:#339af0,color:#000;
  classDef service fill:#4c6ef5,stroke:#364fc7,color:#fff;
  classDef data fill:#51cf66,stroke:#2f9e44,color:#fff;
  classDef infra fill:#ffd43b,stroke:#fab005,color:#000;
  classDef device fill:#ff922b,stroke:#e8590c,color:#fff;
  classDef tooling fill:#845ef7,stroke:#5f3dc4,color:#fff;

  operator["Operator"]:::client

  subgraph Clients["Clients"]
    browser["Web browser"]:::client
  end

  subgraph Interfaces["UI Services"]
    dashboardweb["dashboard-web<br/>(Next.js)"]:::service
  end

  subgraph ControlPlane["Control Plane"]
    coreserver["core-server<br/>(Rust)"]:::service
    sidecar["telemetry-sidecar<br/>(Rust)"]:::service
  end

  subgraph Edge["Edge Nodes"]
    nodeagent["node-agent<br/>(Pi 5 / Pi Zero 2W)"]:::device
    sensors["Sensors / outputs"]:::device
  end

  subgraph InfraServices["Infra"]
    mqtt["Mosquitto<br/>(MQTT broker)"]:::infra
    db["TimescaleDB / Postgres"]:::data
    redis["Redis"]:::infra
    grafana["Grafana / Tempo<br/>(observability)"]:::infra
  end

  subgraph Tooling["Tooling / CI"]
    simlab["Sim Lab<br/>(hardware mocks + E2E)"]:::tooling
    deploytools["Pi deployment tooling<br/>(dashboard SSH deploy; legacy imaging scripts)"]:::tooling
  end

  operator --> browser
  browser <--> dashboardweb
  dashboardweb <--> coreserver

  coreserver <--> db
  coreserver <--> redis

  nodeagent -->|mDNS/zeroconf advertise| coreserver
  coreserver -->|REST: adopt, backups, restore, provisioning| nodeagent

  nodeagent -->|MQTT: telemetry + status| mqtt
  coreserver -->|MQTT: output commands + schedule actions| mqtt

  mqtt -->|subscribe| sidecar
  sidecar -->|SQL batches| db
  sidecar <-.->|gRPC over unix socket (health/optional ingest)| coreserver

  nodeagent --- sensors

  coreserver -.->|OTel traces/logs| grafana
  sidecar -.->|OTel traces/logs| grafana
  grafana --> db

  simlab --> coreserver
  simlab --> mqtt
  simlab --> dashboardweb

  deploytools -.->|deploy over SSH installs node-agent| nodeagent
```

## Key Runbooks

- `docs/runbooks/pi5-deployment-tool.md`: Raspberry Pi 5 production deployment via dashboard “Deploy over SSH”.
- `docs/runbooks/renogy-pi5-deployment.md`: Renogy BT-2 charge-controller configuration notes (no imaging).
- `docs/runbooks/pi5-simulator.md`: Local Raspberry Pi 5 simulator for node-agent testing.

## Development Workflow

A root `Makefile` exposes the most common dev flows:

```bash
make bootstrap # install app dependencies (poetry/npm)
make core   # run Rust controller backend
make web    # run Next.js dashboard with hot reload
make migrate # apply SQL migrations to the local Postgres/TimescaleDB
make seed   # populate demo nodes/sensors/metrics
make ticket t="..." # create a detailed ticket stub in project_management/tickets/
make adr t="..."    # create an ADR stub in docs/ADRs/
```

Individual apps have their own READMEs with toolchain-specific steps, but the table stakes are:

- `apps/core-server-rs`: `cargo run` (or `make core`) to run the Rust controller backend on `127.0.0.1:8000`.
- `apps/telemetry-sidecar`: `cargo run` (or `make rust-sidecar`) to run the Rust telemetry ingest sidecar.
- `apps/farmctl`: `cargo run --manifest-path apps/farmctl/Cargo.toml -- ...` for installer/ops workflows (including DB migrate/seed).
- `apps/dashboard-web`: `npm install` then `make web` (use `FARM_CORE_API_BASE` to point at the core API).
- `apps/node-agent`: `poetry install` then `poetry run uvicorn app.main:app --reload` for local testing. Production nodes are installed via dashboard “Deploy over SSH”; configure sensors post-adoption from the dashboard.
- Mobile clients are deferred on `main`; see branch `freeze/ios-watch-2026q1` for the preserved iOS/watch code.
- `infra`: database migrations live in `infra/migrations`; native service configs live in `infra/`.

### Demo mode setup

#### Fresh install (clean database)

```bash
cd /path/to/farm_dashboard
make migrate

make seed                                   # loads demo nodes/sensors/metrics
make core                                   # starts Rust controller on 127.0.0.1:8000 (leave running)
FARM_CORE_API_BASE=http://127.0.0.1:8000 make web   # dashboard on http://localhost:3001
```

WARNING: `make seed` is destructive to the target database. Only run it against a local/dev DB you are willing to wipe.

Before running the commands above, ensure native Postgres/Mosquitto/Redis are running via the installer/launchd.

Then visit `http://localhost:3001` – all tabs show demo data, alarms, and trends.

#### Existing install (infra already running)

If native services are already running and schema already created:

```bash
# optional: reseed demo data
make seed

# restart core server (Ctrl+C old window first)
make core

# restart dashboard with API base set if needed
FARM_CORE_API_BASE=http://127.0.0.1:8000 make web
```

If the dashboard previously threw “Failed to fetch”, ensure `apps/dashboard-web/.env.local` contains `FARM_CORE_API_BASE=http://127.0.0.1:8000` (legacy: `NEXT_PUBLIC_API_BASE`) and restart both servers.

### Sim Lab (local hardware mocks)

Sim Lab uses local fixture servers plus a native Mosquitto broker to emulate MQTT node telemetry and BLE/mesh/forecast/rates feeds without physical hardware. The E2E harness (`make e2e-web-smoke`) starts these automatically. For manual runs, point the core server at the fixture providers:

```bash
CORE_FORECAST_API_BASE_URL=http://127.0.0.1:9103 \\
CORE_FORECAST_API_PATH=/forecast.json \\
CORE_ANALYTICS_RATES__API_BASE_URL=http://127.0.0.1:9104 \\
CORE_ANALYTICS_RATES__API_PATH=/rates.json \\
make core
```

Run the Playwright adoption smoke against the sim lab endpoints (or use `make e2e-web-smoke` to boot the stack and run this automatically):

```bash
FARM_SIM_LAB_API_BASE=http://127.0.0.1:8000 \\
FARM_SIM_LAB_BASE_URL=http://127.0.0.1:3005 \\
node apps/dashboard-web/scripts/sim-lab-smoke.mjs
```

If Playwright browsers are not installed yet, run `npm run playwright:install` in `apps/dashboard-web` once.

### Operating outside demo mode

The dashboard uses `/api/dashboard/state` as its single “snapshot” endpoint in production. Missing/invalid snapshot responses are treated as backend contract errors (no client-side reconstruction fallback).

Real outputs can also be commanded via `/api/outputs/{id}/command`. When running outside demo mode the endpoint now publishes an MQTT message (`iot/{nodeId}/{outputId}/command` by default), persists the last state/command metadata, and maintains a short history array in the output’s config blob.

### Test suite

Policy: any code change must be tested, and no task is considered Done until the relevant E2E flow passes (run the real app/stack, not just unit tests).

All tests and linting are run locally (no external runners). The pre-commit hook uses a staged-path selector (`tools/git-hooks/select-tests.py`) that runs:

- Doc/log/image-only changes: skip tests.
- High-risk stack changes (core-server, node-agent, telemetry-sidecar, infra, Sim Lab tooling, proto): `make e2e-web-smoke` (boots the full Sim Lab stack and runs the Playwright adoption smoke).
- Dashboard-web-only changes: `make ci-web-smoke` (lint + Vitest smoke).
- Unknown paths default to the stack E2E smoke.

Native Postgres/Mosquitto/Redis must be running for `make e2e-web-smoke` (installer/launchd). For deeper regression coverage, run `make ci` (full suite) or the component-specific commands below.

Do not rely on GitHub Actions to catch regressions; validate locally before pushing. Additional surface-specific commands remain available:

- `npm run build` – dashboard type/lint/build checks.

Install the shared pre-commit hook with `tools/git-hooks/install.sh` (use `--force` to overwrite). The hook runs the staged-path selector and exits early only when every staged file is a doc/log/image extension (`.md`, `.txt`, `.log`, `.jpg`, `.png`, etc).

### QA Notes
- QA workflow: [docs/qa/QA_NOTES.md](docs/qa/QA_NOTES.md)

## Getting Started

1. Fork/clone the repository and install the language toolchains you plan to touch (Python 3.11+, Node 20+).
2. Install dependencies: `make bootstrap`.
3. Copy `.env.example` files into place (only if you need overrides):
   - Core server: `cp apps/core-server-rs/.env.example apps/core-server-rs/.env`
   - Node agent: `cp apps/node-agent/.env.example apps/node-agent/.env`
   - Dashboard web: `cp apps/dashboard-web/.env.example apps/dashboard-web/.env.local`
4. Use the commands above to boot individual components.
5. Document meaningful design choices in `docs/ADRs` so the cross-functional team can follow the evolution of the platform.

This README will evolve alongside the scaffold as the concrete implementations land, but it should give new contributors a reliable entry point today.
