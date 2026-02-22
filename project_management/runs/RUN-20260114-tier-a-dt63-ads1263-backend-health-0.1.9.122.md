# RUN-20260114-tier-a-dt63-ads1263-backend-health-0.1.9.122

- **Context:** DT-63 (ADS1263 Phase 3) — Pi5 ADS1263 hardware backend + deterministic health checks in `node-agent` (gpiozero + spidev), with fail-closed behavior when unhealthy.
- **Host:** Installed controller (Tier A smoke; no DB/settings reset).
- **Node:** Pi5 Node 1 (`pi5-node1`, `10.255.8.170`).

## Commands

### 1) Validation (tests) — node-agent unit tests (prod flavor)

```bash
NODE_TEST_BUILD_FLAVOR=prod make ci-node
```

Result: pass (`66 passed`).

### 2) Deploy (Node1) — update ADS1263 backend file + restart service

```bash
scp apps/node-agent/app/hardware/ads1263_hat.py node1@10.255.8.170:/tmp/ads1263_hat.py

ssh node1@10.255.8.170 \
  'sudo install -m 0644 /tmp/ads1263_hat.py /opt/node-agent/app/hardware/ads1263_hat.py && \
   sudo systemctl restart node-agent.service && \
   sudo systemctl is-active node-agent.service'
```

Result: `active`.

### 3) Validation (Node1) — SPI device present

```bash
ssh node1@10.255.8.170 'ls -l /dev/spidev0.0'
```

Result: `/dev/spidev0.0` exists.

### 4) Validation (controller API) — backend + health surfaced in node status

Create a local token (stored at `/tmp/tier_a_api_token_20260114_dt63.txt`):

```bash
cargo run --quiet --manifest-path apps/core-server-rs/Cargo.toml --bin create_local_api_token -- \
  --name dt63-tier-a \
  --expires-in-days 7 \
  > /tmp/tier_a_api_token_20260114_dt63.txt
```

Query Node1 status (confirm `analog_backend=ads1263` and `analog_health.ok=true`):

```bash
TOKEN=$(cat /tmp/tier_a_api_token_20260114_dt63.txt)
curl -sS -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8000/api/nodes | \
  jq '.[] | select(.config.agent_node_id=="pi5-node1") | {id, name, analog_backend: .config.analog_backend, analog_health: .config.analog_health}'
```

Result: `analog_backend="ads1263"`, `analog_health.ok=true`, `chip_id="0x01"`.

### 5) Build controller bundle DMG (clean worktree hard gate enforced)

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.122 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.122.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.122.dmg`.

### 6) Pre-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (pre): `current_version = 0.1.9.121`.

### 7) Point setup-daemon at the new bundle + upgrade

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.122.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Result: `Upgraded to 0.1.9.122`.

Note: upgrade response included non-fatal stderr:
`xattr: [Errno 13] Permission denied: '/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.122.dmg'`.

### 8) Post-upgrade health + installed version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Result (post): `current_version = 0.1.9.122`.

### 9) Validation (UI) — screenshots captured

```bash
cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-core \
  --no-web \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/tmp/tier_a_api_token_20260114_dt63.txt \
  --out-dir=manual_screenshots_web/20260114_tier_a_dt63_ads1263_health_0.1.9.122

mv manual_screenshots_web/20260114_tier_a_dt63_ads1263_health_0.1.9.122 \
  ../manual_screenshots_web/20260114_tier_a_dt63_ads1263_health_0.1.9.122
```

Output folder:
- `apps/manual_screenshots_web/20260114_tier_a_dt63_ads1263_health_0.1.9.122/`

## Artifacts

- Controller bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.122.dmg`
- Screenshots: `apps/manual_screenshots_web/20260114_tier_a_dt63_ads1263_health_0.1.9.122/`
- This run log: `project_management/runs/RUN-20260114-tier-a-dt63-ads1263-backend-health-0.1.9.122.md`

## Result

Pass (Tier A validated on installed controller; no DB/settings reset).

