# Release tooling

This directory contains lightweight release helpers for:

- semantic version validation
- release channel enforcement (alpha/beta/stable)
- changelog generation from git history

## Validate versions

```bash
python tools/release/release.py validate --targets web,ios,firmware
```

Set the channel via `RELEASE_CHANNEL` or let it infer from CI metadata.

## Generate a changelog entry

```bash
python tools/release/release.py changelog --version 0.2.0 --channel beta --output CHANGELOG.md
```

By default it uses the last git tag as the baseline.
