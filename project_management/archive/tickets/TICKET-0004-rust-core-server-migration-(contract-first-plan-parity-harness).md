# TICKET-0004: Rust Core Server Migration (contract-first plan + parity harness)

**Status:** Open

## Description
Migrate the controller production runtime from the current Python core-server + Node/Next dashboard server to a **single Rust core-server binary** that serves:
- `/api/*` (Rust API)
- `/` (static dashboard build assets)

This enables a Rust-first production runtime without rewriting the JS/TS dashboard UI into Rust/WASM.

This ticket is the long-form requirements dump for the `RCS-*` tasks in `project_management/TASKS.md`.

Related ADR:
- `docs/ADRs/0004-rust-core-server-migration-(api-+-static-dashboard-served-by-rust).md`

## Scope
- Contract-first workflow where Rust exports the canonical OpenAPI spec and the dashboard consumes a generated TS client.
- DB schema parity throughout the migration (shared migrations; no “Rust-only schema fork”).
- A parity harness capable of running Python + Rust backends side-by-side against the same DB seed and comparing responses for a milestone set of endpoints.
- Production runtime serves dashboard static assets from Rust (no Node server process in launchd).
- Preserve rollback: installer can switch back to Python core-server if parity regressions are discovered.

Out of scope (for this ticket; tracked separately):
- Rewriting the dashboard UI into Rust/WASM.
- Replacing the setup wizard UI (installer surface) with the full dashboard UI.
- Node-agent rewrite (Pi targets, Linux/systemd) unless explicitly planned as a separate migration.

## Acceptance Criteria
- `RCS-1` produces an ADR + parity harness scaffolding and is wired into `make`/docs so future agents can follow the plan.
- `RCS-2` lands a Rust core-server skeleton that serves `/healthz`, a minimal `/api` surface, static asset hosting for `/`, and exports OpenAPI deterministically.
- `RCS-3` makes the dashboard build output static and updates production to drop the dashboard Node server runtime.
- `RCS-4` ports endpoints incrementally with parity checks; `make e2e-web-smoke` can run against the Rust core-server.

## Notes
- Prefer an “API-first” migration order that unlocks the dashboard quickly: auth/session, dashboard state, nodes, sensors/outputs, schedules, backups, adoption.
- Keep the parity harness strict and cheap: compare status code + JSON body (stable key ordering), allow optional ignore lists for volatile fields (timestamps, IDs) only when justified.
- Keep the installer substrate stable first: do not weaken the Setup North-Star hard gate while migrating.
