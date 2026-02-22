# FEATURE-001: Pi 5 Local Display (Basic Status + Live Values)

## Summary
Add an **optional** “local display” mode for Raspberry Pi 5 nodes that drives an attached HDMI/touch display and shows **node health + connectivity + latency/jitter + live sensor values**. The displayed content is **configurable from the main dashboard web UI** (per-node “display profile”), and the node remains fully functional when no display is attached.

## Business goal
Reduce on-site troubleshooting time by letting a non-technical operator validate that a node is online, talking to the core server/MQTT, and producing sane sensor values without needing a laptop.

## Raw inputs (from feature checklist)
- Optional display on a Pi 5 node, configurable from the web interface.
- Show:
  - Status of communication back to server.
  - Network latency to server IP.
  - Jitter (variability in ping over time).
  - Names + live values for sensors configured on that node (including solar production data).

## Current state in the repo (do not rebuild)
- Node-agent already exposes operational status/config surfaces (e.g., `/v1/status`, `/v1/config`) intended for adoption/monitoring flows.
- SD-card imaging and first-boot automation tooling exist (overlay + first-boot script + Pi Imager profile generator).
- Dashboard-web already has node detail and adoption UX; it is the natural place to add a “Display profile” editor.

This ticket focuses on **wiring an operator-facing fullscreen display** on Pi 5 nodes using what already exists (node-agent HTTP server + existing config sync), not building a new dashboard stack.

## Recommended build strategy (reuse-first)
### Display stack (choose one)
1) **Raspberry Pi OS + Chromium kiosk** (preferred)
   - Pros: minimal licensing risk, small delta, works with current deployment approach.
   - Approach: install/enable a systemd unit that launches Chromium in kiosk mode pointed at `http://localhost:<node-agent-port>/display`.
2) **FullPageOS-like kiosk image** (optional alternative)
   - Pros: purpose-built to boot to a fullscreen URL.
   - Cons: separate distro pipeline + licensing/maintenance considerations.

### UI stack (do not invent a new frontend)
- Implement a dedicated **node-agent route** (e.g., `/display`) that renders a kiosk-friendly view using the node-agent’s existing status/config data sources.

## Scope
### In scope
- “Display profile” config model (per node) and dashboard UI editor.
- Node-agent kiosk display route(s) that render:
  - core communication status
  - latency + jitter
  - live sensor list filtered by the display profile
- Optional kiosk auto-start at boot (Pi 5 only), controlled by config.

### Out of scope
- Output control from the display (tracked in FEATURE-002).
- Trend charts beyond “current value” (tracked in FEATURE-002).
- Any changes to the iOS/watch apps.

## Functional requirements
### FR1. Display enable/disable
- Default: **disabled** for all nodes.
- Enablement is a **per-node setting** in node config (see “Config model”).
- If enabled but no display is attached, node-agent continues normally; kiosk service may fail but must not crash node-agent.

### FR2. Display content (MVP)
The display view must show, at minimum:
1) **Node identity**
   - Node name, node id, software version (if available), IP address (best-effort).
2) **Core communication health**
   - “Connected/Degraded/Offline” with an explanation string (e.g., “MQTT connected”, “MQTT reconnecting”, “last publish 3m ago”).
3) **Network health to core**
   - Latency (ms) to configured core server target.
   - Jitter (ms) computed over the last N samples.
4) **Sensors**
   - A list of “tiles” with sensor name + current value + units + stale indicator.
   - List contents are controlled by the display profile (default profile can show “all sensors on node”).

### FR3. Latency/jitter calculation
- Use a method that does **not** require privileged ICMP:
  - Preferred: TCP connect latency to the core server API port and/or MQTT broker port.
- Jitter definition (explicit):
  - Jitter = standard deviation of the last N latency samples (ms).
- The display must show N and sampling interval (small text is fine).

### FR4. Refresh behavior
- Display refresh interval is configurable (default: 2s for UI refresh; 10s for latency sampling).
- If the node is offline from core, keep displaying the last known sensor values but mark them stale.

## Config model
Extend the node configuration schema with a `display` section (stored centrally and synced to the node via the existing config mechanism):

```json
{
  "display": {
    "enabled": false,
    "kiosk_autostart": false,
    "ui_refresh_seconds": 2,
    "latency_sample_seconds": 10,
    "latency_window_samples": 12,
    "tiles": [
      { "type": "core_status" },
      { "type": "latency" },
      { "type": "sensor", "sensor_id": "<deterministic-sensor-id>" }
    ]
  }
}
```

Notes:
- `tiles` is ordered and drives the layout.
- If `tiles` is empty, node-agent uses a safe default layout.

## UX requirements
- Kiosk view must be readable at 1–2 meters:
  - Large typography, high contrast, no dense tables.
- Must render without external internet access.
- Must degrade gracefully:
  - If a sensor disappears, tile shows “Missing” instead of breaking the page.

## Non-functional requirements
- **Performance:** kiosk UI must remain usable on Pi 5 with <10% sustained CPU overhead attributable to display mode.
- **Reliability:** kiosk failure must not affect telemetry publish, adoption, provisioning, or mesh.
- **Offline:** if the core server is unreachable, display continues to show local data and indicates offline state.

## Observability
- Node-agent logs a structured event when:
  - display mode enables/disables
  - kiosk autostart script/service is installed/removed
  - latency probe fails repeatedly (with backoff)
- `/v1/status` should include a `display` summary (enabled, kiosk status, last latency sample age).

## Repo boundaries (where work belongs)
- `apps/node-agent/`
  - Add kiosk display route and latency/jitter sampling.
  - Add status export for display state.
- `apps/core-server/`
  - Persist/serve `display` section as part of node config; ensure it flows through existing config APIs.
  - If config schema is contract-first, update the canonical OpenAPI + generated clients accordingly.
- `apps/dashboard-web/`
  - Add a “Display profile” editor on the node detail/config surface.
- `project_management/tickets/` + `project_management/TASKS.md`
  - Add linked work item(s) and acceptance gate references.

## Acceptance criteria
1) Dashboard UI can enable display mode for a node and select at least **3 sensors** to show.
2) Node receives updated config and renders `/display` accordingly within **60 seconds**.
3) Display shows:
   - core communication status
   - latency + jitter (explicit N and interval)
   - live sensor tiles with stale indicators
4) When core is unreachable, display shows “Offline” state and continues showing last sensor values.
5) If kiosk autostart is enabled, rebooting the Pi launches fullscreen display automatically.
6) `make e2e-web-smoke` remains green (no regressions).
7) Node-agent unit tests include coverage for jitter calculation and “missing sensor” rendering behavior.

## Test plan
- Unit tests (node-agent):
  - jitter computation over a known sample set
  - latency sampler failure/backoff
- Integration:
  - Pi 5 simulator: render `/display` with a synthetic config and synthetic sensors.
- Manual (hardware):
  - confirm kiosk autostart on Pi 5 with HDMI display attached.

## Dependencies
- Existing node config distribution must support adding the `display` section without breaking older nodes.
- Dashboard needs a safe capability model for config edits (use existing auth/capabilities).

## Risks / open questions
- Confirm the most reliable “latency target” (core API port vs MQTT broker port) given typical deployments.
- Decide whether `/display` should be reachable only on localhost or LAN (security posture; FEATURE-002 needs stricter controls).
