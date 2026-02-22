# Raspberry Pi 5 Network-Boot Provisioning (Prototype)

This runbook documents a **macOS-first** prototype for provisioning Raspberry Pi 5 nodes using the Raspberry Pi bootloader’s **HTTP boot / Network Install** feature.

This is an **advanced** deployment option intended for bulk rollouts and for sites where SD card imaging is undesirable.

## Scope and current state
- This runbook completes the **prototype/documentation** portion of DT-44.
- Real hardware validation is tracked separately (see `project_management/TASKS.md`).
- This workflow provisions **Raspberry Pi OS**, then uses the Farm Dashboard controller to deploy the node-agent (Dashboard → Deployment → SSH).

## Safety boundaries (do not skip)
1. **Do not run DHCP/TFTP servers on a production LAN.**
   - This runbook avoids DHCP/TFTP entirely by using **Pi EEPROM HTTP boot** (BOOT_ORDER includes `0x7`).
2. **Changing `HTTP_HOST` can disable HTTPS in the Pi bootloader.**
   - Treat any plain-HTTP netboot as “trusted LAN only” (ideally a dedicated provisioning VLAN).
   - Production hardening should use HTTPS + a custom CA hash (Pi 5 supports this) and/or keep the default Raspberry Pi host.
3. **Use wired Ethernet for provisioning.**
   - HTTP boot / Network Install requires a wired Ethernet link.

## Background (what the Pi bootloader does)
- The Pi bootloader boot mode `0x7` (“HTTP”) downloads `boot.img` + `boot.sig` and boots into an embedded Raspberry Pi Imager.
- The embedded Imager fetches an OS list JSON (`IMAGER_REPO_URL`) and then installs an OS image to the local storage target (SD/USB/NVMe).

## Prototype path A: Use Raspberry Pi’s default Network Install (internet required)
Use this when you just want to confirm “Pi 5 can boot with blank media and install an OS” without hosting anything on the controller.

### 1) Update and edit the Pi EEPROM bootloader config
On a Pi 5 running Raspberry Pi OS (temporary boot is fine):
1. Update EEPROM tooling:
   ```bash
   sudo apt-get update
   sudo apt-get install -y rpi-eeprom
   ```
2. Edit EEPROM config:
   ```bash
   sudo -E rpi-eeprom-config --edit
   ```
3. Ensure your boot order includes HTTP as a fallback:
   ```ini
   [all]
   BOOT_ORDER=0xf71
   ```
   - `0xf71` means: try SD (`1`), then HTTP (`7`), then restart/loop (`f`).

### 2) Boot the Pi with blank target media
1. Connect Ethernet.
2. Insert the target storage (blank SD/USB/NVMe).
3. Boot with no SD OS present (or with an empty SD).
4. The embedded Imager should appear and allow selecting an OS to install.

### 3) Install Raspberry Pi OS Lite and enable SSH during install
In the embedded Imager, select:
- Raspberry Pi OS Lite (64-bit)
- Enable SSH + set credentials (preferred: set hostname too)

## Prototype path B: Local HTTP mirror for net_install artifacts (controller-hosted)
Use this when you want the Pi bootloader to fetch `boot.img`/`boot.sig` from the controller (useful for repeatability and for later offline hardening).

### 1) Prepare netboot artifacts on the controller (macOS)
This downloads the official `boot.img`/`boot.sig` and the current Imager OS list JSON into a local directory.

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  netboot prepare \
  --output build/pi5-netboot \
  --force
```

Expected output layout:
- `build/pi5-netboot/net_install/boot.img`
- `build/pi5-netboot/net_install/boot.sig`
- `build/pi5-netboot/os_list_imagingutility_v4.json`

### 2) Serve netboot artifacts over HTTP
Choose a controller IP on the LAN and bind the server to it:

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  netboot serve \
  --root build/pi5-netboot \
  --host 0.0.0.0 \
  --port 8080
```

Verify locally:
- `curl http://127.0.0.1:8080/healthz`

### 3) Point the Pi EEPROM at the controller host
On the Pi 5, edit EEPROM config:

```bash
sudo -E rpi-eeprom-config --edit
```

Add (or update) settings similar to:

```ini
[all]
BOOT_ORDER=0xf71
HTTP_HOST=<controller_ip_or_dns>
HTTP_PORT=8080
HTTP_PATH=net_install
IMAGER_REPO_URL=http://<controller_ip_or_dns>:8080/os_list_imagingutility_v4.json
NET_INSTALL_ENABLED=1
```

Notes:
- This prototype uses **plain HTTP**. Use a dedicated provisioning VLAN.
- A hardened production variant should use HTTPS + a custom CA hash and/or keep the default Raspberry Pi host.

## After OS install: Deploy node-agent and adopt
Once Raspberry Pi OS boots:
1. Ensure SSH is enabled and the controller can reach the Pi on the LAN.
2. In the dashboard, go to **Deployment** → **Remote Pi 5 Deployment** and deploy the node-agent over SSH.
3. Return to **Nodes** → **Scan** → **Adopt**.

## Troubleshooting
- Enable boot UART logging (helps confirm which URL the Pi is fetching):
  - Set `BOOT_UART=1` in EEPROM config and attach a UART adapter to GPIO14/15.
- If the Pi never reaches the embedded Imager:
  - Confirm Ethernet link lights are on.
  - Confirm `BOOT_ORDER` includes `0x7` (HTTP).
  - If using a controller-hosted server, confirm the controller firewall allows inbound TCP on the chosen port.
- If embedded Imager cannot fetch OS images:
  - Path A requires internet.
  - Path B only mirrors `boot.img`/`boot.sig` and the OS list JSON; OS image downloads may still be remote unless you also mirror image URLs.

## Production hardening (follow-up work)
- Replace plain HTTP with HTTPS + `HTTP_CACERT_HASH` (Pi 5 supports custom CA certs in EEPROM).
- Host a curated Imager repo JSON that includes a “Farm Dashboard Node” image entry (ties into DT-43).
- Add real-hardware validation and concurrency testing (ties into DT-48).

