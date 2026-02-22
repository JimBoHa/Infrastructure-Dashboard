# Release Channels & Versioning

The platform ships along three release channels:

- **alpha**: early validation builds for internal testing
- **beta**: staging builds for broader QA
- **stable**: production releases

Each build uses semantic versioning (`MAJOR.MINOR.PATCH`). Channel selection is controlled by the
`RELEASE_CHANNEL` environment variable or inferred from CI context.

## Validation rules

- Versions must be valid semver in:
  - `apps/dashboard-web/package.json`
- Stable channel disallows prerelease tags.
- Alpha/beta allow prerelease tags; if present they must match the channel (`-alpha.*` or `-beta.*`).

## CI enforcement

CI runs `python tools/release/release.py validate` for web changes and will fail if:

- a version file was not updated alongside code changes, or
- the version is not valid semver for the selected channel.

## Changelog generation

```bash
python tools/release/release.py changelog --version 0.2.0 --channel beta --output CHANGELOG.md
```

## Production release artifacts (DMG)

Public production releases must ship exactly one end-user DMG:

- ✅ `FarmDashboardInstaller-<version>.dmg` (the only public download)
- 🚫 `FarmDashboardController-<version>.dmg` is an internal build artifact and is embedded inside the installer DMG for auto-detection and upgrades.

### Build a release installer DMG locally (recommended)

1) Build `farmctl`:

```bash
cargo build --release --manifest-path apps/farmctl/Cargo.toml
```

2) Ensure native deps are available (see `farmctl native-deps --help`).

3) Build the public installer artifact:

```bash
apps/farmctl/target/release/farmctl dist --version <version> --native-deps <path>
```

This emits:
- `build/release-<version>/FarmDashboardInstaller-<version>.dmg`
- `build/release-<version>/SHA256SUMS.txt` (installer only)

### Publish to GitHub

DMG artifacts are built **locally on macOS** (never via GitHub Actions). CI only validates versions and runs tests; it does not produce DMG release artifacts.

When creating a GitHub release, upload only the installer DMG (and optional checksums/logs). Do not attach a standalone `FarmDashboardController-*.dmg` asset.

Example (from the repo root):

```bash
VERSION=0.1.5
OUT=build/release-$VERSION

# Build the installer DMG locally.
cargo build --release --manifest-path apps/farmctl/Cargo.toml
apps/farmctl/target/release/farmctl dist --version "$VERSION" --native-deps build/release-0.1.0/native-deps

# Publish to GitHub (installer-only asset).
gh release create "$VERSION" "$OUT/FarmDashboardInstaller-$VERSION.dmg" "$OUT/SHA256SUMS.txt" \
  --title "Farm Dashboard $VERSION" \
  --notes "Installer-only release (controller bundle DMG is embedded inside the installer)."
```
