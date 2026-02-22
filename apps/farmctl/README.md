# farmctl

`farmctl` is the Rust installer CLI and setup daemon for Farm Dashboard controller bundles.
It mounts local DMG bundles, validates manifests, installs artifacts into the
install root, manages upgrades/rollbacks, and serves the setup wizard UI.

## Production releases (installer-only)

End users should download and run only `FarmDashboardInstaller-<version>.dmg`.
`FarmDashboardController-<version>.dmg` is an internal artifact embedded inside the installer app bundle for auto-detection and upgrades.

Build the public installer DMG locally:

```bash
cargo build --release --manifest-path apps/farmctl/Cargo.toml
apps/farmctl/target/release/farmctl dist --version <version> --native-deps <path>
```

## Common commands

```bash
cargo run -- install --bundle /path/to/FarmDashboardController-1.2.3.dmg
cargo run -- upgrade --bundle /path/to/FarmDashboardController-1.2.4.dmg
cargo run -- rollback
cargo run -- health --json
cargo run -- diagnostics --output /Users/Shared/FarmDashboard/support.zip
cargo run -- serve --host 127.0.0.1 --port 8800 --static-root apps/setup-app/static
```

## Bundle build (local)

```bash
cargo run -- bundle --version 1.2.3 --output build/FarmDashboardController-1.2.3.dmg
```

## Installer DMG build (local)

```bash
cargo run -- installer --version 1.2.3 --bundle build/FarmDashboardController-1.2.3.dmg --output build/FarmDashboardInstaller-1.2.3.dmg
```

## Native dependency bundles

Generate native Postgres (with TimescaleDB), Redis, and Mosquitto builds with
`farmctl native-deps`, then pass the output to `--native-deps` when building bundles:

```bash
cargo run -- native-deps --output build/native-deps
cargo run -- bundle --version 1.2.3 --output build/FarmDashboardController-1.2.3.dmg --native-deps build/native-deps
```
