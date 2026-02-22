# RUN-20260114-tier-a-dt64-add-hardware-sensor-0.1.9.123

- **Context:** DT-64 (ADS1263 Phase 4) — End-to-end “Add hardware sensor” from Dashboard (Pi-only), with apply semantics and immediate ingest via core’s sensor registry.
- **Host:** Installed controller (Tier A smoke; no DB/settings reset).
- **Node:** Pi5 Node 1 (`pi5-node1`, `10.255.8.170`, node id `0a55b329-104f-46f0-b50b-dea9a5cca1b3`).

## Commands

### 1) Build controller bundle DMG (clean worktree hard gate enforced)

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.123 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.123.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.123.dmg`.

### 2) Pre-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (pre): `current_version = 0.1.9.122`.

### 3) Point setup-daemon at the new bundle + upgrade

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.123.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Result: `Upgraded to 0.1.9.123`.

Note: upgrade response included non-fatal stderr:
`xattr: [Errno 13] Permission denied: '/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.123.dmg'`.

### 4) Post-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (post): `current_version = 0.1.9.123`.

### 5) Create local API token (for dashboard + API checks)

```bash
cargo run --quiet --manifest-path apps/core-server-rs/Cargo.toml --bin create_local_api_token -- \
  --name dt64-tier-a \
  --expires-in-days 7 \
  > /tmp/tier_a_api_token_20260114_dt64.txt
```

Result: token written to `/tmp/tier_a_api_token_20260114_dt64.txt`.

### 6) Validation (UI) — capture screenshots (and confirm “Add sensor” flow)

```bash
cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-core \
  --no-web \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/tmp/tier_a_api_token_20260114_dt64.txt \
  --focus-node-id=0a55b329-104f-46f0-b50b-dea9a5cca1b3 \
  --apply-node-sensor \
  --out-dir=manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123

mv manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123 \
  ../manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123
```

Result: screenshots captured at `apps/manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123/`.

Viewed: `apps/manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123/sensors_node_after_apply.png`.

### 7) Validation (API) — apply a node sensor config and verify telemetry ingest

Confirm the stored/applied sensor config includes `DT64 ADC0 Voltage`:

```bash
TOKEN=$(cat /tmp/tier_a_api_token_20260114_dt64.txt)
NODE_ID=0a55b329-104f-46f0-b50b-dea9a5cca1b3

curl -sS -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:8000/api/nodes/$NODE_ID/sensors/config" | jq
```

Result: config includes `DT64 ADC0 Voltage` (sensor id `1a5bad49ce9e1bbf429af8c1`) and `analog_backend=ads1263` with `analog_health.ok=true`.

Confirm apply status fields persisted on the node record:

```bash
curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/nodes | \
  jq '.[] | select(.id=="0a55b329-104f-46f0-b50b-dea9a5cca1b3") |
    {desired_sensors_updated_at:.config.desired_sensors_updated_at,
     node_sensors_last_apply_status:.config.node_sensors_last_apply_status,
     node_sensors_last_apply_at:.config.node_sensors_last_apply_at,
     node_sensors_last_apply_warning:.config.node_sensors_last_apply_warning}'
```

Result: `node_sensors_last_apply_status="applied"` and timestamps present.

Confirm `/api/sensors` shows live updates (poll twice and ensure `latest_ts` advances):

```bash
SENSOR_ID=1a5bad49ce9e1bbf429af8c1

curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/sensors | \
  jq '.[] | select(.sensor_id=="'"$SENSOR_ID"'") | {sensor_id,name,latest_value,latest_ts}'

sleep 2

curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/sensors | \
  jq '.[] | select(.sensor_id=="'"$SENSOR_ID"'") | {sensor_id,latest_value,latest_ts}'
```

Result: `latest_ts` advanced and `latest_value` present.

### 8) Optional: post-apply screenshots (shows the added sensor in the drawer)

```bash
cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-core \
  --no-web \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/tmp/tier_a_api_token_20260114_dt64.txt \
  --focus-node-id=0a55b329-104f-46f0-b50b-dea9a5cca1b3 \
  --out-dir=manual_screenshots_web/20260114_tier_a_dt64_post_apply_0.1.9.123

mv manual_screenshots_web/20260114_tier_a_dt64_post_apply_0.1.9.123 \
  ../manual_screenshots_web/20260114_tier_a_dt64_post_apply_0.1.9.123
```

Result: screenshots captured at `apps/manual_screenshots_web/20260114_tier_a_dt64_post_apply_0.1.9.123/`.

## Artifacts

- Controller bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.123.dmg`
- Screenshots (apply flow): `apps/manual_screenshots_web/20260114_tier_a_dt64_add_sensor_0.1.9.123/`
- Screenshots (post-apply): `apps/manual_screenshots_web/20260114_tier_a_dt64_post_apply_0.1.9.123/`
- This run log: `project_management/runs/RUN-20260114-tier-a-dt64-add-hardware-sensor-0.1.9.123.md`

## Result

Pass (Tier A validated on installed controller; no DB/settings reset).

