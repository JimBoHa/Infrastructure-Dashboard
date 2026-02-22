# RUN-20260114-tier-a-dt61-no-sim-prod-0.1.9.120

- **Context:** DT-61 (ADS1263 Phase 1) — bake build flavor into node-agent artifacts + enforce “no simulation in production” + fail-closed analog publish.
- **Host:** Installed controller (Tier A smoke; no DB/settings reset).

## Commands

### 1) Validate node-agent tests in prod build flavor

```bash
NODE_TEST_BUILD_FLAVOR=prod make ci-node
```

Result: `63 passed`.

### 2) Build controller bundle DMG (clean worktree hard gate enforced)

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.120 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.120.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.120.dmg`.

### 3) Pre-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (pre): `current_version = 0.1.9.119`.

### 4) Point setup-daemon at the new bundle + upgrade

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.120.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Result: `Upgraded to 0.1.9.120`.

Note: upgrade response included non-fatal stderr:
`xattr: [Errno 13] Permission denied: '/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.120.dmg'`.

### 5) Post-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (post): `current_version = 0.1.9.120`.

### 6) Verify build flavor is baked into the shipped node-agent artifact (controller release)

```bash
tar -xOf /usr/local/farm-dashboard/releases/0.1.9.120/artifacts/node-agent/node-agent-overlay.tar.gz \
  opt/node-agent/app/build_info.py
```

Result:

```py
BUILD_FLAVOR: Literal["prod", "dev", "test"] = "prod"
```

## Artifacts

- Controller bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.120.dmg`
- This run log: `project_management/runs/RUN-20260114-tier-a-dt61-no-sim-prod-0.1.9.120.md`

## Result

Pass (Tier A validated on installed controller; no DB/settings reset).

