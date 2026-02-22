# 0004. Rust core-server migration (API + static dashboard served by Rust)

* **Status:** Accepted
* **Date:** 2025-12-30

## Context
The current controller runtime is split across multiple languages/processes:
- Python FastAPI core-server (business logic + API)
- A Node/Next.js server for the dashboard (server runtime + API proxying)
- A separate setup wizard/daemon surface (moving to Rust)

This increases production complexity:
- More launchd services to manage and coordinate
- More moving parts for auth/session/CORS
- Higher operational/debug burden during the transition away from container runtimes toward an installer-first, launchd-native macOS stack

We have a strong Rust-first commitment for production runtime. However, the dashboard UI must remain a JS/TS frontend (charts, Outlook-like scheduling UI, complex provisioning flows). A full “dashboard in Rust/WASM” rewrite adds significant risk with little upside.

## Decision
We will migrate the controller production runtime to a single Rust core-server binary that:
- Serves the canonical REST API under `/api/*`
- Serves the dashboard as static assets under `/` (SPA fallback routing)

Key constraints:
- **Contract-first:** Rust owns/exports the canonical OpenAPI spec. TS client generation is driven from that spec to keep the dashboard aligned.
- **DB schema parity:** Rust and Python run against the same DB schema/migrations during the transition.
- **Side-by-side parity harness:** A dedicated harness compares responses for a milestone set of endpoints while we port functionality incrementally.
- **Rollback preserved:** The installer can switch back to the Python core-server if parity regressions are discovered during the transition.

Scope boundary:
- Rust SSR/HTMX is acceptable for the **setup wizard** surface (form-heavy, small UI footprint).
- The main dashboard stays as JS/TS static assets.

## Consequences
Benefits:
- Production runtime becomes Rust-first without a UI rewrite.
- Removes the Node server process from production (one fewer launchd service).
- Simplifies CORS/auth/sessions by serving `/api/*` and `/` from the same origin.

Risks / costs:
- Requires a careful, incremental migration plan with strong parity checks to avoid regressions.
- Some Next.js server-only features must be replaced or redesigned to work as static assets.
- Build/release tooling must reliably produce a dashboard static bundle and embed/serve it from Rust.
