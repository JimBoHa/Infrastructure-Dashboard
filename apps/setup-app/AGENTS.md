> Setup App Notes
>
> - Setup app UI is served by `farmctl serve` (Rust). There is no Python backend.
> - Setup app is a thin UI/API wrapper around `farmctl`; do not re-implement installer logic here.
> - Use the `farmctl` CLI for install/upgrade/rollback/diagnostics; keep actions idempotent.
> - Treat secrets as write-only in the UI (redact on read, allow explicit reveal only if requested).
> - Keep the wizard usable for non-experts; prefer clear statuses and single-click actions.
> - If new tooling is required, implement it in Rust and expose via `farmctl`.
