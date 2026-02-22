# Raspberry Pi 5 Node Deployment (Production: Deploy over SSH)

**Production policy:** Raspberry Pi 5 nodes are deployed **only** via the dashboard **Deployment → Remote Pi 5 Deployment (SSH)** flow.

Do **not** use Raspberry Pi Imager “Run custom script on first boot”, do **not** copy seed files to the boot partition, and do **not** use `tools/flash_node_image.sh` for production deployments. Those workflows are deprecated to avoid field confusion.

## What you need
- A Raspberry Pi 5 with **Raspberry Pi OS Lite (64-bit)** flashed (clean image).
- SSH enabled on the Pi (and you must know the username/password).
- The controller (Mac mini) can reach the Pi over the network (Ethernet recommended for first deploy).

## Deploy steps (Dashboard → SSH)
1. Boot the Pi 5 on the LAN and find its IP/hostname.
2. In the dashboard, open **Deployment** → **Remote Pi 5 Deployment**.
3. Enter the Pi hostname/IP, SSH port, username, and password.
4. Click **Fetch host key** and verify the fingerprint matches the Pi you intend to deploy to.
   - First-time connections require explicit host-key approval (TOFU).
   - Host key mismatches are blocked to prevent accidental deployment to the wrong device.
5. Click **Connect & Deploy** and keep the window open until the job completes.

## What the deploy job does (important)
- Installs the node-agent overlay to the Pi and configures systemd units.
- Ensures the `farmnode` service user exists and is in required groups (e.g., `gpio`, `spi`).
- **Ensures SPI0 is enabled** for the optional ADS1263 ADC HAT:
  - If `/dev/spidev0.0` is missing, the deploy job enables `dtparam=spi=on` in the appropriate boot config and **reboots the Pi once**, then reconnects automatically.

## Notes / troubleshooting
- Credentials are used only for the deployment job and are not persisted.
- If the node-agent is already installed and healthy, the job may report an idempotent outcome (`already_installed/healthy`).
- Optional secondary services (e.g., `renogy-bt.service`) are installed in the generic node stack and are enabled/disabled automatically based on `/opt/node-agent/storage/node_config.json` via `node-agent-optional-services.path`.
- If you re-flash a Pi and its host key changes, remove the previous trust entry from the controller known_hosts file:
  - Default: `/Users/Shared/FarmDashboard/storage/ssh/known_hosts` (can be overridden by `CORE_SSH_KNOWN_HOSTS_PATH`).

## Deprecated (dev-only / historical)
- “Preconfigured media” / first-boot script workflows are deprecated for production. Historical reference: `docs/runbooks/pi5-preconfigured-media.md`.
- Bulk imaging helpers (`tools/build_image.py`, `tools/flash_node_image.sh`) are not part of the supported production flow.
