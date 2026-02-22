# RUN-20260114-tier-a-dt65-reservoir-depth-0.1.9.124

- **Context:** DT-65 (ADS1263 Phase 5) — Reservoir depth transducer (4–20mA current loop measured as voltage across a 163Ω shunt, converted into depth).
- **Host:** Installed controller (Tier A smoke; no DB/settings reset).
- **Node:** Pi5 Node 1 (`pi5-node1`, `10.255.8.170`, node id `0a55b329-104f-46f0-b50b-dea9a5cca1b3`).
- **Sensors:**
  - Reservoir Depth: `ea5745e00cb0227e046f6b88` (`ft`, current loop; `shunt=163Ω`, `range=5m`, `ch=0`)
  - Debug voltage: `1a5bad49ce9e1bbf429af8c1` (`V`, raw ADC voltage; `ch=0`)

## Commands

### 1) Build controller bundle DMG (clean worktree hard gate enforced)

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.124 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.124.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.124.dmg`.

### 2) Upgrade installed controller to the new bundle (no resets)

Point setup-daemon at the new DMG and upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.124.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Confirm installed version:

```bash
curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'
```

Result: `current_version=0.1.9.124` (previous `0.1.9.123`).

### 3) Create local API token (for dashboard + API checks)

```bash
cargo run --quiet --manifest-path apps/core-server-rs/Cargo.toml --bin create_local_api_token -- \
  --name dt65-tier-a \
  --expires-in-days 7 \
  > /tmp/tier_a_api_token_20260114_dt65.txt
```

### 4) Validation (UI) — capture screenshots (and view key artifacts)

```bash
cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-core \
  --no-web \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/tmp/tier_a_api_token_20260114_dt65.txt \
  --focus-node-id=0a55b329-104f-46f0-b50b-dea9a5cca1b3 \
  --out-dir=manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124

mv manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124 \
  ../manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124
```

Result: screenshots captured at `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/`.

Viewed (evidence):
- `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/sensors_reservoir_depth_detail.png`
- `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/sensors_add_sensor.png`
- `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/trends_reservoir_depth_selected.png`
- `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/trends_reservoir_depth_last_6h.png`

### 5) Validation (API) — confirm ADS1263 health + current-loop config

```bash
TOKEN=$(cat /tmp/tier_a_api_token_20260114_dt65.txt)
NODE_ID=0a55b329-104f-46f0-b50b-dea9a5cca1b3

curl -sS -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:8000/api/nodes/$NODE_ID/sensors/config" | \
  jq '{ads1263, analog_backend, analog_health, sensors_count:(.sensors|length)}'
```

Result:
- `analog_backend="ads1263"`
- `analog_health.ok=true` and `chip_id="0x01"`
- sensors present (count = 2)

Confirm the Reservoir Depth sensor config (current loop) is applied:

```bash
curl -sS -H "Authorization: Bearer $TOKEN" \
  "http://127.0.0.1:8000/api/nodes/$NODE_ID/sensors/config" | \
  jq '.sensors | map(select(.sensor_id=="ea5745e00cb0227e046f6b88"))'
```

Result includes:
- `channel=0`
- `current_loop_shunt_ohms=163`
- `current_loop_range_m=5`
- `unit="ft"`

### 6) Validation (API) — sanity-check voltage → mA → depth conversion

Expected shunt voltage bounds for 4–20mA across 163Ω:
- 4mA → ~0.652V
- 20mA → ~3.26V

Read latest depth and voltage from `/api/sensors`:

```bash
DEPTH_ID=ea5745e00cb0227e046f6b88
VOLT_ID=1a5bad49ce9e1bbf429af8c1

curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/sensors | \
  jq '.[] | select(.sensor_id=="'"$DEPTH_ID"'" or .sensor_id=="'"$VOLT_ID"'") |
     {sensor_id,name,unit,latest_value,latest_ts}'
```

Result (sample):
- `DT64 ADC0 Voltage`: ~`1.125V` (within expected bounds)
- `Reservoir Depth`: ~`2.98 ft`

Sanity math:
- `1.125V / 163Ω ≈ 6.90mA`
- depth = `(6.90mA - 4mA) / 16mA * 5m ≈ 0.91m ≈ 2.98ft` (matches)

## Artifacts

- Controller bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.124.dmg`
- Screenshots: `apps/manual_screenshots_web/20260114_tier_a_dt65_reservoir_depth_0.1.9.124/`
- This run log: `project_management/runs/RUN-20260114-tier-a-dt65-reservoir-depth-0.1.9.124.md`

## Result

Pass (Tier A validated on installed controller; no DB/settings reset).

