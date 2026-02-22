# RUN-20260131 — Tier A — NA-65 Renogy kWh sensors (0.1.9.231)

- **Date:** 2026-01-31
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.231
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds_TierA/FarmDashboardController-0.1.9.231.dmg`

## Scope

- NA-65: Add Renogy PV energy sensors:
  - `PV Energy (today)` (`kWh`)
  - `PV Energy (total)` (`kWh`)
- Confirm Node 1 Renogy charge-controller telemetry is ingested by the controller (nighttime expected: “today” may be `0.0`).
- DW-199 (follow-up): Fix Sensors & Outputs sensor drawer crash caused by instantiating Highcharts stock-tools bindings in the Trend preview chart.

## CI / smoke (repo)

Node agent:

```bash
make ci-node
```

Core smoke:

```bash
make ci-core-smoke
```

Dashboard web:

```bash
make ci-web-smoke
cd apps/dashboard-web && npm run build
```

Result: `PASS`.

## Refresh installed controller (Upgrade)

Preflight health checks:

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
```

Installed version before: `0.1.9.230`.

Rebuild + refresh:

```bash
python3 tools/rebuild_refresh_installed_controller.py \
  --output-dir /Users/Shared/FarmDashboardBuilds_TierA \
  --allow-dirty \
  --version 0.1.9.231
```

Bundle build output:
- DMG: `/Users/Shared/FarmDashboardBuilds_TierA/FarmDashboardController-0.1.9.231.dmg`
- Log: `/Users/Shared/FarmDashboardBuilds_TierA/logs/bundle-0.1.9.231.log`

Installed version after: `0.1.9.231` (previous: `0.1.9.230`).

Installed stack smoke:

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Node 1 ingest verification (controller API)

Node:
- **Node name:** Pi5 Node 1
- **Node ID:** `0a55b329-104f-46f0-b50b-dea9a5cca1b3`

Node-agent (hardware) confirms the updated Renogy collector is present on Node 1:

```bash
ssh node1@10.255.8.170 'grep -n "pv_energy_today_kwh" /opt/node-agent/app/hardware/renogy_bt2.py'
ssh node1@10.255.8.170 'grep -n "pv_energy_total_kwh" /opt/node-agent/app/hardware/renogy_bt2.py'
```

Sensors (preset-applied):
- `PV Energy (today)` sensor id: `99d0f28d17605f067931581c` (latest: `0.0` kWh; nighttime expected)
- `PV Energy (total)` sensor id: `3fc3024621abbaba11c37c52` (latest: non-zero kWh total)

Example check (auth required):

```bash
curl -fsS -H "Authorization: Bearer $(cat /tmp/tier_a_api_token_20260131_na62_renogy_kwh.txt)" \
  "http://127.0.0.1:8000/api/sensors?node=0a55b329-104f-46f0-b50b-dea9a5cca1b3" \
  | jq 'map(select(.name|test(\"PV Energy\")))'
```

## Regression check (Trends)

Confirmed the Trends page still loads (no client-side exception) after the TrendChart navigation/bindings fix:
- `http://127.0.0.1:8000/analytics/trends`

## Tier‑A screenshots (captured + viewed)

Evidence directory:
- `manual_screenshots_web/tier_a_0.1.9.231_renogy_kwh_20260131_072924`

Evidence (opened and visually reviewed):
- `manual_screenshots_web/tier_a_0.1.9.231_renogy_kwh_20260131_072924/sensors_node1.png`
  - Verified: Node 1 sensor list shows PV Energy sensors.
- `manual_screenshots_web/tier_a_0.1.9.231_renogy_kwh_20260131_072924/sensor_pv_energy_today_drawer.png`
  - Verified: Sensor drawer opens and Trend preview renders; PV Energy (today) shows latest `0.0` kWh.
- `manual_screenshots_web/tier_a_0.1.9.231_renogy_kwh_20260131_072924/sensor_pv_energy_total_drawer.png`
  - Verified: Sensor drawer opens and Trend preview renders; PV Energy (total) shows non-zero kWh.
