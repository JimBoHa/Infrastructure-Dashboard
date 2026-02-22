# Farm Dashboard Setup App

This app provides the static UI for the local, GUI-first setup wizard. The
backend is served by `farmctl serve` (Rust); there is no Python backend.

The wizard now integrates with `farmctl` for bundle installs, upgrades, rollbacks,
health checks, and diagnostics exports. Provide a local DMG bundle path in the
Configuration step before running install/upgrade actions.

## Local run (developer workflow)

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- serve --host 127.0.0.1 --port 8800 --static-root apps/setup-app/static
```

Then open `http://127.0.0.1:8800`.
