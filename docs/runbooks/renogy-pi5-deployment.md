# Renogy Pi 5 Deployment Runbook (BT-2)

This runbook is for real hardware deployments on macOS only. Simulator validation lives in
`docs/runbooks/renogy-pi5-simulator.md`.

**Production deployment policy:** Raspberry Pi 5 nodes are deployed **only** via the dashboard
**Deployment → Remote Pi 5 Deployment (SSH)** flow. Do not use Pi Imager first-boot scripts or boot-partition file copying.

## Before you start
- Raspberry Pi 5 (Raspberry Pi OS Lite 64-bit recommended).
- Renogy `RNG-CTRL-RVR20-US` controller with a powered `BT-2` module.
- Core server reachable on the LAN (MQTT broker reachable by the Pi).
- Ethernet connected to the Pi 5 (recommended for first deploy).

## 1) Find the BT-2 BLE address (no Pi CLI required)
Try these first:
- **iPhone**: Use a BLE scanner app (e.g., LightBlue or nRF Connect). Look for a device named
  `Renogy`, `BT-2`, or similar and note its MAC address (for example `AA:BB:CC:DD:EE:FF`).
- **macOS**: Use a BLE scanner app (LightBlue for macOS) or Apple’s “Bluetooth Explorer”
  (Additional Tools for Xcode) to view the device address.

If the address is not shown, see troubleshooting below for a one-time `bluetoothctl` scan.

## 2) Deploy the Pi 5 (Dashboard → SSH)
1. Flash a clean Raspberry Pi OS Lite (64-bit) image with SSH enabled, boot the Pi on the LAN, and find its IP/hostname.
2. In the dashboard, open **Deployment** → **Remote Pi 5 Deployment** and deploy the node-agent.
   - Runbook: `docs/runbooks/pi5-deployment-tool.md`
3. Adopt the node from the dashboard (scan/adopt).

## 3) Configure Renogy BT-2 (Dashboard)
1. Open **Nodes** → select your node → open the **Renogy** section.
2. Enter the **BT-2 address** and click **Save connection**.
   - Mode: **BLE (recommended)** for the standard path.
   - Poll interval: default is fine for most sites.
3. Wait for telemetry ingestion. The node will publish Renogy sensors (PV/load/battery) once connected.

## 4) Verify telemetry
- Dashboard → **Power** and **Sensors/Trends** should show live `pv_power_w`, `battery_soc_percent`, `load_power_w`, etc.
- Local node check (optional): `curl http://<node-ip>:9000/v1/status` and `curl http://<node-ip>:9000/v1/config`.

---

## Troubleshooting

### BT-2 address not visible on iPhone/macOS
If the BLE address is not exposed by your scanner app, use a one-time scan on the Pi 5:

```bash
sudo bluetoothctl
power on
scan on
# wait for the Renogy/BT-2 device and note the MAC address
scan off
quit
```

### Common issues
- **BT-2 not found**: confirm the BT-2 is powered and within a few feet; re-scan.
- **Telemetry missing**: confirm the node is online and publishing status; then confirm the controller is ingesting MQTT telemetry.
