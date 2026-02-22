# Runbook: Mesh Dropouts

## Symptoms
- Mesh nodes intermittently disappear from the dashboard.
- LQI/RSSI values degrade or stop updating.
- Alerts for node offline fire frequently.

## Fast Checks
1. Confirm mesh coordinator is online and `mesh.health` is `online`.
2. Check node-agent mesh diagnostics (`/v1/mesh/diagnostics` if available).
3. Review logs for `mesh` telemetry publish failures.
4. Validate radio configuration (PAN ID, channel, network key).

## Likely Causes
- Coordinator reboot or serial adapter disconnect.
- Channel interference (Wi-Fi overlap).
- Battery-powered nodes sleeping too aggressively.

## Fix Steps
1. Restart mesh adapter or node-agent service.
2. Re-run `tools/mesh_pair.py` to confirm pairing.
3. Adjust channel to avoid Wi-Fi overlap.
4. Reduce sleep intervals or increase mesh poll interval.

## Escalation
- Capture mesh diagnostics snapshots and attach the latest mesh telemetry payloads for analysis.
