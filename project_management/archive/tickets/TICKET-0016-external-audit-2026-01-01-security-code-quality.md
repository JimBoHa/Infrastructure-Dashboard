# TICKET-0016: External Audit (2026-01-01) — Security & Code Quality

This ticket archives the external audit context and captures our **validated** findings vs **false positives**, plus the tracked remediation work items.

## Source Reports (verbatim)

- `docs/audits/2026-01-01-farm-dashboard-security-code-quality-audit-report.md`
- `docs/audits/2026-01-01-rust-migration-audit-snippet.md`

## Validated Findings (confirmed in-repo)

### Rust controller (apps/core-server-rs)

- Deploy-from-server job store uses `std::sync::Mutex` with `.lock().expect("... poisoned")` (panic → poisoned lock → future panics).
- Deploy-from-server request structs contain plaintext secrets (passwords) and derive `Debug` (risk of accidental logging); no SSH key-based auth option is exposed.
- Some routes use `expect` / `unwrap` in non-test code (panic on unexpected input).
- Some routes coerce invalid JSON config types by overwriting values (silent data loss risk).
- The Rust core-server does **not** fully implement the OpenAPI contract currently shipped in `apps/core-server/openapi/farm-dashboard.json` (missing paths).
- Rust core-server currently serves a vendored Python-generated OpenAPI JSON via `include_str!(...)`, so Rust is not yet the canonical contract source.

### Setup substrate (apps/farmctl)

- Bundled Postgres is initialized with a hardcoded password (`postgres`) and default config writes a `postgres:postgres` URL (non-unique credentials).
- Bundled Mosquitto config uses a loopback-only listener (`127.0.0.1`), which conflicts with documented node broker URLs (nodes need LAN-reachable broker).

### WAN portal (apps/wan-portal)

- Startup uses `.expect("reqwest client should build")` (panic instead of controlled failure on TLS/system config errors).

### Tooling

- `tools/e2e_ios_smoke.py` hardcodes a test password (`SmokeTest!123`) (should be env-configurable; iOS/watch is currently deferred).

## False Positives / Non-Issues (confirmed)

- `apps/farmctl/src/launchd.rs` “env lock poisoned” is inside `#[cfg(test)]` only.
- `apps/farmctl/src/native_deps.rs` `.expect(...)` calls are inside tests only.
- `apps/core-server-rs/src/routes/analytics.rs` `.expect("expected sensor")` is inside tests only.
- Some `127.0.0.1` usages are intentional for local-only services (e.g., Postgres/Redis binding) and are not automatically a defect.
- Old failing E2E artifact references are not evidence of current failure; the gating target is `make e2e-installer-stack-smoke`.

## Tracked Work Items (SSoT)

See `project_management/TASKS.md` for current statuses:

- Core Server: `CS-50`, `CS-51`, `CS-52`
- Rust Core Server Migration: `RCS-10`, `RCS-11`, `RCS-12`
- Setup App & Native Services: `SETUP-19`, `SETUP-20`
- Developer Tooling: `DT-54`, `DT-55`

