# Runbook: Node Adoption Failures

## Symptoms
- Node shows up in discovery but adoption fails with 4xx/5xx.
- Dashboard reports "adoption token invalid" or "node not reachable".
- MQTT command publishes but node never transitions to `adopted`.

## Fast Checks
1. Verify node is reachable on the network (`/v1/status` on node agent).
2. Confirm the adoption token matches (`/v1/config` on node agent or local dashboard).
3. Check core-server logs for `request_id` tied to the adoption attempt.
4. Confirm MQTT broker is reachable from both node and core server.

## Likely Causes
- Adoption token mismatch or expired token.
- Node cannot reach core-server URL/port (firewall, Wi-Fi mismatch).
- MQTT broker misconfigured or credentials invalid.

## Fix Steps
1. Regenerate an adoption token from the dashboard and re-run adoption.
2. Re-provision Wi-Fi credentials via BLE or the local node UI.
3. Validate MQTT settings in node config (`mqtt_host`, `mqtt_username`, `mqtt_password`).
4. If using Sim Lab, ensure deterministic simulator nodes are running and discovery scan is enabled (core seeds the advertised adoption token on first adopt).

## Escalation
- If tokens are valid and MQTT is healthy, capture core-server logs + node agent logs with `request_id` and file an issue.
