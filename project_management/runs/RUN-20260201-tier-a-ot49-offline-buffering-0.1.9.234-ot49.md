# RUN-20260201 — Tier A + Hardware — OT / TICKET-0049 Offline telemetry buffering (0.1.9.234-ot49)

- **Date:** 2026-02-01
- **Tier:** A (installed controller refresh; **no DB/settings reset**) + hardware validation (Pi 5 nodes)
- **Controller version:** `0.1.9.234-ot49`
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.234-ot49.dmg`

## Scope

- Complete OT offline buffering implementation (Option C): append-only segment spool on each node + Rust replay publisher with controller ACK and receipt-time liveness.
- Validate on the installed controller (Tier A) and on real Pi 5 nodes (microSD + NVMe) using disconnect + reboot-mid-outage + replay-drain scenarios.
- Tier‑B clean-host E2E is deferred to the validation cluster ticket `OT-13`.

## CI / smoke (repo)

```bash
make ci-node-smoke
cargo test --manifest-path apps/node-forwarder/Cargo.toml
cargo test --manifest-path apps/telemetry-sidecar/Cargo.toml
make ci-farmctl
make ci-web-smoke
```

Result: `PASS`.

## Refresh installed controller (Tier A)

Preflight health checks:

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:8800/api/status
```

Bundle build output:
- DMG: `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.234-ot49.dmg`
- Log: `/Users/Shared/FarmDashboardBuildsDirty/logs/bundle-0.1.9.234-ot49.log`

Installed stack smoke:

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier‑A screenshots (captured + viewed)

Evidence directory:
- `manual_screenshots_web/20260201_030013`

Evidence (opened and visually reviewed):
- `manual_screenshots_web/20260201_030013/nodes_0a55b329-104f-46f0-b50b-dea9a5cca1b3.png`
  - Verified: Node detail shows “Telemetry buffering” card with spool/ACK status visible.

## Hardware validation (Pi 5 nodes)

Node boot media + free space (captured from the nodes):

### Node 1
- SSH: `node1@10.255.8.170`
- Boot media: `mmcblk0` (microSD), ~119G
- `/` free: ~105G
- Services: `node-forwarder=active`, `node-agent=active`

Example checks:

```bash
ssh node1@10.255.8.170 'lsblk -o NAME,TYPE,SIZE,MODEL,MOUNTPOINT -e7'
ssh node1@10.255.8.170 'df -h /'
ssh node1@10.255.8.170 'systemctl is-active node-forwarder node-agent'
ssh node1@10.255.8.170 'curl -fsS http://127.0.0.1:9101/v1/status | jq .'
```

### Node 2
- SSH: `node2@10.255.8.20`
- Boot media: `nvme0n1` (CT500P310SSD8), ~466G
- `/` free: ~433G
- Services: `node-forwarder=active`, `node-agent=active`

Example checks:

```bash
ssh node2@10.255.8.20 'lsblk -o NAME,TYPE,SIZE,MODEL,MOUNTPOINT -e7'
ssh node2@10.255.8.20 'df -h /'
ssh node2@10.255.8.20 'systemctl is-active node-forwarder node-agent'
ssh node2@10.255.8.20 'curl -fsS http://127.0.0.1:9101/v1/status | jq .'
```

### Deploy updated node stack

Deployed via controller “Deploy to Pi 5” workflow (deploy-from-server).

Job IDs:
- Node 1 job: `c80b3ca166e73418` (`PASS`)
- Node 2 job: `06a1feb1781e634e` (`PASS`)

### Disconnect / reconnect harness (hardware)

Harness:
- `tools/ot_offline_buffer_harness.sh`

Scenarios:
- Force MQTT offline (simulate hard disconnect), continue sampling, buffer locally.
- Reboot node mid-outage (spool recovery).
- Restore MQTT connectivity and verify replay drains under throttles.
- Assert controller liveness does **not** flap offline during replay.

Results:
- Node 1: `PASS`.
- Node 2: initial run failed due to stale node config containing unknown sensor IDs (ACK could not advance; `acked_seq` stuck). Remediation:
  - edited `/opt/node-agent/storage/node_config.json` to keep only known sensor `91af9586787db88cbe609222`
  - cleared `/opt/node-agent/storage/spool` to reset the stream
  - restarted services
  - rerun: `PASS`

## Follow-ups

- Tier‑B clean-host validation remains tracked as `OT-13` (blocked until a clean host is available; do not attempt on the production-installed controller host).

