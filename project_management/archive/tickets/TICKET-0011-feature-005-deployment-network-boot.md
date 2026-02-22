# FEATURE-005: Deployment Option — Network Boot Provisioning (Pi 5)

## Summary
Add a network-boot-based provisioning path for Pi 5 nodes so an operator can deploy a node with **blank media** by powering on a Pi 5 on the LAN and having it boot/install from a local server. This is an advanced deployment option intended for bulk rollouts and for sites where SD imaging is undesirable.

## Business goal
Enable “rack-and-stack” style provisioning for multiple nodes with minimal manual handling of storage media.

## Raw inputs (from feature checklist)
- Network boot:
  - deploy Pi5 with blank media installed
  - power on Pi5
  - server on local network with network boot server detected by Pi5
  - used to boot and configure with node software

## Recommended build strategy (reuse-first)
Use the Raspberry Pi platform’s supported mechanisms rather than inventing a custom PXE stack:
- **Network install / HTTP boot** via Raspberry Pi bootloader features.
- Host the required boot artifacts and/or an Imager OS list JSON on the local server.

## Scope
### In scope
- A “netboot server” component (may be a documented configuration on the core server host) that provides:
  - boot artifacts for HTTP boot/network install
  - optionally a custom Imager OS list that includes “Farm Dashboard Node”
- A runbook to configure Pi 5 bootloader EEPROM to enable the desired boot order.
- A minimal “golden path” that results in a node-agent installed and adoptable node.

### Out of scope
- Supporting older Raspberry Pi models.
- Replacing Raspberry Pi OS; we install/configure on top of supported OS images.

## Functional requirements
### FR1. Netboot server deliverable
Provide a documented server setup that can:
- serve `boot.img` / `boot.sig` (or equivalent) for HTTP boot, and/or
- serve a custom `os_list` JSON for network install that includes a Farm Dashboard node image option.

### FR2. Security posture
- Follow Raspberry Pi’s signing/verification expectations for network boot artifacts.
- Document how to run in a “trusted LAN only” mode vs a more hardened mode (TLS, signing keys).

### FR3. Node identity and configuration
- The provisioned node must come up in a discoverable/adoptable state using existing scan/adopt.
- If multiple nodes are provisioned, identity collisions must be avoided (MAC-based identity is acceptable).

### FR4. Operator UX (documented)
A runbook must exist that an operator can follow, including:
- how to enable HTTP boot/network install in Pi 5 EEPROM (one-time per device)
- how to verify the Pi is using network boot (UART logs optional)
- how to choose/install the Farm Dashboard node image
- how to adopt/configure after install

## Non-functional requirements
- The netboot server must be able to handle at least **5 concurrent** node boots on a typical LAN without timing out.
- Failure must be diagnosable via logs on the netboot server.

## Repo boundaries (where work belongs)
- `docs/runbooks/`
  - add a “Pi 5 network boot provisioning” runbook.
- `infra/` (optional)
  - if we package a netboot server as a container/service, define it here.
- `apps/core-server/` (optional)
  - if we embed netboot server functions, keep them modular; do not bloat the core API runtime.

## Acceptance criteria
1) A runbook exists and can be followed end-to-end on a real Pi 5.
2) With blank media installed, a Pi 5 can boot using the documented network boot method on the same LAN.
3) The resulting system boots into Raspberry Pi OS with node-agent installed/enabled and is adoptable via the dashboard.
4) Provisioning 5 Pis sequentially works reliably; provisioning 2 in parallel works reliably.
5) Security/signing requirements are explicitly documented (keys, where stored, rotation story).

## Test plan
- Manual validation on Pi 5 hardware (required).
- Optional: small lab automation that provisions a Pi 5 and asserts it appears in scan/adopt.

## Dependencies
- FEATURE-003 may provide a “Farm Dashboard node image” that can be offered in network install OS list.
- Requires a LAN environment where DHCP/Ethernet is available.

## Risks / open questions
- Decide whether MVP is:
  - “network install into Raspberry Pi Imager + operator selects image” (lower automation), or
  - “fully automated HTTP boot to our installer” (higher automation, higher complexity).
- Managing signing keys and EEPROM configuration safely at scale.
