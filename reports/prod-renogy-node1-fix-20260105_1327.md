# Prod Run Log — Pi5 Renogy Node1 telemetry + analytics (2026-01-05)

## Environment
- Controller: macOS (Mac mini) production stack, service user `_farmdashboard`
- Node: Raspberry Pi 5 `10.255.8.170` (node-agent id `pi5-node1`), Renogy Rover + BT-2
- MQTT broker: `10.255.8.66:1883`

## Goal
- Fix dashboard showing no live data / analytics blank; ensure telemetry persists in the DB and analytics endpoints return real values.

## Actions
- Built controller + installer bundles:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.8.5.dmg`
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardInstaller-0.1.8.5.dmg`
- Upgraded controller via setup-daemon (`POST http://127.0.0.1:8800/api/upgrade`) to `0.1.8.5`.
- Verified controller endpoints now expose live telemetry-backed state:
  - `/api/sensors` includes `latest_value` / `latest_ts` (read-time joins).
  - `/api/metrics/query` returns bounded series.
  - `/api/analytics/status` reports non-zero SOC and fresh `last_updated`.
- Issue: node uptime/CPU/storage stayed `0` because node-agent publishes status on a non-UUID topic (`iot/pi5-node1/status`) and the existing node row had empty `nodes.config.agent_node_id`.
- Diagnosed status payload via `mosquitto_sub` (required `DYLD_LIBRARY_PATH=...`; see Issues).
- Implemented telemetry-sidecar fallback mapping:
  - When status topic node id is non-UUID and no `agent_node_id` match exists, resolve node by MAC hint from the status payload (`mesh.coordinator_ieee`), then persist `nodes.config.agent_node_id`.
- Built + upgraded to `0.1.8.6` to ship the telemetry-sidecar fix.

## Verification (evidence)
- After `0.1.8.6` upgrade:
  - `/api/dashboard/state` node shows non-zero `uptime_seconds`, `cpu_percent`, `storage_used_bytes` and `config.agent_node_id='pi5-node1'`.
  - `/api/sensors` Renogy sensors show non-zero `latest_value` for battery voltage/SOC with timestamps advancing every ~30s.
  - `/api/metrics/query` for battery voltage returns 60+ points over the last hour, with a recent last point timestamp.
  - `/api/analytics/status` shows `battery_soc` with `last_updated` < 1 minute old.

## Issues encountered
- `farmctl upgrade` stderr: `xattr: [Errno 13] Permission denied` when attempting to mutate xattrs on controller DMGs owned by the login user; upgrade succeeds but logs are noisy.
- Setup-daemon `/api/config` response includes full `database_url` (credential exposure risk); avoid printing, consider redacting server-side.
- `mosquitto_sub` / `mosquitto_pub` require `DYLD_LIBRARY_PATH=/usr/local/farm-dashboard/native/mosquitto/lib` due to missing LC_RPATHs in the packaged binaries.
- Tests not run: test hygiene gate requires stopping Farm launchd jobs/processes; production stack was kept running during the live fix.
