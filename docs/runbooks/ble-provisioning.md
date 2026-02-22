# Runbook: BLE Provisioning Failures

## Symptoms
- iOS app cannot discover the node over BLE.
- Provisioning session starts but Wi-Fi never applies.
- Node agent reports BLE session created but no config changes persist.

## Fast Checks
1. Confirm BLE is enabled on the node and the device supports BLE.
2. Check node-agent logs for `ble_payload` events and `request_id`.
3. Verify the provisioning secret is set (`NODE_PROVISIONING_SECRET`).
4. Ensure the iOS app points to the correct environment (local vs cloud).

## Likely Causes
- Bluetooth stack down (BlueZ service on Linux).
- BLE permissions missing for the iOS app.
- Provisioning queue encryption key mismatch.

## Fix Steps
1. Restart the node agent service (`systemctl restart node-agent`).
2. Validate BLE health (`hciconfig`, `bluetoothctl`).
3. Clear stale provisioning sessions in the node UI or via `/v1/provisioning/session`.
4. Re-run provisioning with a new adoption token.

## Escalation
- Collect node-agent logs with `request_id`, and include the BLE payload status from `/v1/provisioning/session`.
