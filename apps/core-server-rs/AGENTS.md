# Rust Core Server (Migration) — Agent Notes

- This is the Rust replacement for the Python `apps/core-server` production runtime.
- Goal: a single Rust binary serves `/api/*` + `/` (static dashboard assets) for production.
- Contract-first: Rust will own/export the canonical OpenAPI spec over time.
- Keep code modular; avoid monolithic files.
- Validate changes with `cargo build --manifest-path apps/core-server-rs/Cargo.toml` (and parity harness when applicable).

