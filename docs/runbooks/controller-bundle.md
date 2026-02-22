# Controller Bundle Format (DMG)

This runbook defines the local controller bundle format used by `farmctl` and the setup app.
Bundles are delivered as **local-path DMGs** (no remote downloads).

Note: for production, end users download only `FarmDashboardInstaller-<version>.dmg`. The controller bundle DMG is an internal artifact embedded inside the installer DMG for auto-detection and upgrades.

## Volume layout

When mounted, the DMG exposes a single directory:

```
FarmDashboardController/
  manifest.json
  artifacts/
    core-server/
      bin/core-server
    telemetry-sidecar/
      bin/telemetry-sidecar
    dashboard-web/
      static/
  configs/
    setup-config.json
    core-server.env.example
  native/
    postgres/
    redis/
    mosquitto/
```

## Manifest schema (`manifest.json`)

```json
{
  "format_version": 2,
  "bundle_version": "1.2.3",
  "created_at": "2025-01-01T00:00:00Z",
  "components": [
    {
      "name": "core-server",
      "version": "1.2.3",
      "path": "artifacts/core-server",
      "entrypoint": "artifacts/core-server/bin/core-server",
      "sha256": "...",
      "size_bytes": 123456
    },
    {
      "name": "telemetry-sidecar",
      "version": "1.2.3",
      "path": "artifacts/telemetry-sidecar",
      "entrypoint": "artifacts/telemetry-sidecar/bin/telemetry-sidecar",
      "sha256": "...",
      "size_bytes": 123456
    }
  ],
  "files": [
    {
      "path": "artifacts/core-server/bin/core-server",
      "sha256": "...",
      "size_bytes": 123456
    },
    {
      "path": "artifacts/dashboard-web/static/index.html",
      "sha256": "...",
      "size_bytes": 123456
    }
  ]
}
```

## Install/upgrade behavior

- `farmctl install --bundle <path>` mounts the DMG, validates checksums, and copies
  artifacts into `install_root/releases/<bundle_version>`.
- Stable entrypoints are symlinked in `install_root/bin` so launchd plists remain
  unchanged across upgrades.
- Dashboard web is shipped as static assets under `artifacts/dashboard-web/static` and
  served by the Rust core-server (no separate dashboard runtime/service in production).
- Native dependencies (Postgres, Redis, Mosquitto) are copied into
  `install_root/native` and launched via launchd alongside core services.
- `farmctl rollback` switches the symlinks back to the previous release recorded
  in `install_root/state.json`.
- `farmctl` remains compatible with older bundles using `format_version: 1`, but new bundles
  are emitted as `format_version: 2`.

## Building bundles

`farmctl bundle --version <version> --output <path>.dmg` builds artifacts and
writes a DMG with the layout above.

To include native dependencies in the bundle, build them with `farmctl native-deps`
and pass the output directory to `--native-deps`. The output should include
`postgres/` (with TimescaleDB extensions), `redis/`, and `mosquitto/` subfolders.

Example:

```
cargo run --manifest-path apps/farmctl/Cargo.toml -- native-deps --output build/native-deps
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 1.2.3 --output build/FarmDashboardController-1.2.3.dmg --native-deps build/native-deps
```

To refresh an already-installed controller from a locally-built bundle (Tier‑A dev loop), see `docs/runbooks/controller-rebuild-refresh-tier-a.md`.
