# Pi 5 Deployment — “Preconfigured Media” (Deprecated)

**Deprecated for production:** The only supported Raspberry Pi 5 node deployment method is
**Dashboard → Deployment → Remote Pi 5 Deployment (SSH)**.

This runbook is retained for historical/dev reference only to avoid losing context, but it should
not be used for field deployments.

This runbook describes the **non-technical** “preconfigured storage media” workflow for a Raspberry Pi 5 node:

- Flash an SD/USB device with **Raspberry Pi Imager**
- Run a **first-boot script** that installs the node-agent automatically
- Boot the Pi and **adopt it from the Farm Dashboard** (scan/adopt)

This path is intended to take **<= 5 minutes** and **<= 10 clicks** once you have the kit.

## What you need
- A **Pi 5** and an **SD card or USB boot media**
- A Mac with **Raspberry Pi Imager** installed
- A running Farm Dashboard controller on the same LAN (or the node can at least reach it later)
- The **Pi 5 Node Image Kit** (`pi5-node-image-kit-*.zip`)

## 1) Get the Pi 5 Node Image Kit
Download the latest kit (zip) from the Farm Dashboard release page or Setup Center.

When unzipped, it contains:
- `node-agent-firstrun.sh` (the only file you must select in Imager)
- `VERSION`
- `SHA256SUMS`
- `node-agent-imager.json` (human-readable profile + operator steps)

## 2) Flash using Raspberry Pi Imager
1. Open **Raspberry Pi Imager**
2. Choose **Device**: Raspberry Pi 5
3. Choose **OS**: **Raspberry Pi OS Lite (64-bit)** (Bookworm recommended)
4. Choose **Storage**: your SD card / USB device
5. Click the **gear icon** (OS customization):
   - Set **username + password** (required by Raspberry Pi OS)
   - Configure **Wi‑Fi** (optional if using Ethernet)
   - Enable **SSH** (optional; for troubleshooting only)
   - Enable **Run custom script on first boot** and select `node-agent-firstrun.sh` from the unzipped kit
6. Click **Write**

## 3) Boot and adopt
1. Insert the flashed media into the Pi 5
2. Connect the Pi to your LAN (Ethernet recommended for first boot)
3. Power on the Pi
4. In the Farm Dashboard web UI, go to **Nodes** → **Scan** and adopt the node

## Verify (optional)
- Local node status: `http://<node-ip>:9000/v1/status`
  - `service_version` should match the kit `VERSION`.

## Optional: offline pre-seeding (advanced)
If you need to preload configuration (for a site that boots before a controller is online), you can copy these files to the **boot** volume **before the first boot**:
- `node-agent-firstboot.json` (one-time node name / optional adoption token)
- `node_config.json` (optional full node config)
- `node-agent.env` (optional env overrides; avoid embedding secrets in reusable media)

The first-boot installer consumes these files and deletes them from the boot volume.

## Notes / safety
- The first-run script is designed to be **idempotent**: if it runs again, it re-applies the overlay and revalidates the installed runtime payload.
- No secrets are embedded in the generic kit.
- The kit is offline-capable on the Pi: it does not require WAN access during first boot (all runtime deps are shipped in the overlay).
