# RUN-20260114-tier-a-dt62-remove-ads1115-0.1.9.121

- **Context:** DT-62 (ADS1263 Phase 2) — remove “ADS1115” as a user/config concept; `driver_type=analog` only; backend is ADS1263.
- **Host:** Installed controller (Tier A smoke; no DB/settings reset).

## Commands

### 1) Validation (repo) — UI does not reference ADS1115

```bash
rg -n "ADS1115|ads1115" apps/dashboard-web
```

Result: no matches.

### 2) Validation (repo) — `ads1115` only appears in legacy mapper/docs

```bash
rg -n "ads1115" apps
```

Result: `ads1115` appears only in `apps/core-server-rs/src/routes/node_sensors.rs` (legacy mapper + reject message).

### 3) Validation (tests) — core smoke suite

```bash
make ci-core-smoke
```

Result: pass.

### 4) Build controller bundle DMG (clean worktree hard gate enforced)

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.121 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.121.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.121.dmg`.

### 5) Pre-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (pre): `current_version = 0.1.9.120`.

### 6) Point setup-daemon at the new bundle + upgrade

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.121.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Result: `Upgraded to 0.1.9.121`.

Note: upgrade response included non-fatal stderr:
`xattr: [Errno 13] Permission denied: '/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.121.dmg'`.

### 7) Post-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (post): `current_version = 0.1.9.121`.

### 8) Validation (API) — reject `driver_type=ads1115`

Create a local token (stored at `/tmp/tier_a_api_token_20260114_dt62.txt`):

```bash
cargo run --quiet --manifest-path apps/core-server-rs/Cargo.toml --bin create_local_api_token -- \
  --name dt62-tier-a \
  --expires-in-days 7 \
  > /tmp/tier_a_api_token_20260114_dt62.txt
```

Reject test (expect `400`):

```bash
TOKEN=$(cat /tmp/tier_a_api_token_20260114_dt62.txt)
NODE_ID=0a55b329-104f-46f0-b50b-dea9a5cca1b3

curl -sS -o /tmp/dt62_ads1115_reject_response.txt -w "%{http_code}" \
  -X PUT "http://127.0.0.1:8000/api/nodes/$NODE_ID/sensors/config" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"sensors":[{"preset":"voltage","sensor_id":"","name":"reject-ads1115","type":"ads1115","channel":0,"unit":"V"}]}'
```

Result:
- `http_status=400`
- body: `driver_type 'ads1115' has been removed; use 'analog'.`

### 9) Validation (UI) — screenshots captured

```bash
cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-core \
  --no-web \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/tmp/tier_a_api_token_20260114_dt62.txt \
  --out-dir=manual_screenshots_web/20260114_tier_a_dt62_no_ads1115_0.1.9.121

# Move to repo-root for consistency with other run logs:
mv manual_screenshots_web/20260114_tier_a_dt62_no_ads1115_0.1.9.121 \
  ../manual_screenshots_web/20260114_tier_a_dt62_no_ads1115_0.1.9.121
```

Output folder:
- `manual_screenshots_web/20260114_tier_a_dt62_no_ads1115_0.1.9.121/`

## Artifacts

- Controller bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.121.dmg`
- Screenshots: `manual_screenshots_web/20260114_tier_a_dt62_no_ads1115_0.1.9.121/`
- This run log: `project_management/runs/RUN-20260114-tier-a-dt62-remove-ads1115-0.1.9.121.md`

## Result

Pass (Tier A validated on installed controller; no DB/settings reset).
