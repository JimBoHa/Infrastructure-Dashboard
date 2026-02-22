# FEATURE-006: Renogy BT-2 — One-Click Setup (Dashboard UX)

## Summary
Provide a **one-click (or near one-click)** user experience in the dashboard to enable Renogy BT-2 telemetry on a Pi 5 node and auto-create the default Renogy sensors at **30s** intervals, targeting non-technical operators. This must reuse the repo’s existing Renogy collector and deployment tooling rather than re-implementing BLE protocol work.

## Business goal
Make it realistic for a non-technical person to bring a Renogy charge controller online (data trending every 30 seconds) without editing JSON and without running CLI tools.

## Raw inputs (from feature checklist)
- Add a button to the webpage used to configure sensors on a Pi 5 node:
  - connects to a Renogy BT-2 bluetooth module
  - pulls default data points (from provided screenshot)
  - trends at 30 second intervals
  - close to one-click configuration

## Current state in the repo (already implemented at the collector/tooling layer)
- A Renogy BT-2 telemetry collector on Pi 5 nodes exists.
- A Renogy Pi 5 deployment bundle tool exists (generates node config + first-boot assets + renogy-bt service).
- A runbook exists describing Renogy deployment and required bundle outputs.

This ticket focuses on the missing piece: **dashboard UX that makes Renogy configuration easy**.

## Scope
### In scope
- Dashboard UI: “Connect Renogy BT-2” action on a node config/sensors page.
- Core config update path that:
  - enables the Renogy collector for that node
  - creates the default sensor set with 30s interval
  - avoids duplicates if run twice
- Optional: a “Renogy preset” in the dashboard provisioning generator that emits the same bundle as the CLI tool.

### Out of scope
- Implementing the BLE protocol (already exists).
- Supporting arbitrary Renogy devices beyond the known charge controller scope for MVP.

## Functional requirements
### FR1. One-click action (happy path)
From the node detail/config page:
1) User clicks “Connect Renogy BT-2”.
2) User provides the BT-2 BLE MAC address (or selects from a discovered list if discovery is supported).
3) User confirms.
4) System:
   - updates node config to enable Renogy collection
   - creates default Renogy sensors with `interval_seconds=30`
   - pushes config to the node (or instructs node to pull)
5) Within 2 minutes, Renogy sensors appear with live values and trending.

### FR2. Default Renogy sensor set
- The default sensor list must match the repo’s existing Renogy bundle defaults (single source of truth).
- If the default list changes in the CLI tool, the dashboard preset must be updated in lockstep (add a parity test).

### FR3. Idempotency
If the user clicks the button twice:
- sensors are not duplicated
- config is not duplicated
- the UI reports “already configured” with a link to the existing sensors

### FR4. Error handling
The UI must surface actionable errors:
- “BT-2 not reachable” (with suggested checks: power, range)
- “BLE disabled on node”
- “Collector not installed” (if relevant)
- “MQTT/core not reachable”

### FR5. Security/auth
- This action is a config mutation and must require normal dashboard auth + capability checks.
- The BT-2 MAC address is not a secret, but any Wi-Fi credentials must never be handled in this flow.

## Non-functional requirements
- First success should require no SSH.
- Configuration time target:
  - < 60s operator time (input MAC + confirm)
  - < 120s until first data point arrives (LAN dependent)

## Repo boundaries (where work belongs)
- `apps/dashboard-web/`
  - Add the Renogy setup action and UI flow.
  - If adding a provisioning preset, extend the provisioning generator UI.
- `apps/core-server/`
  - Add/extend config mutation endpoint(s) to apply the Renogy preset safely.
  - Maintain contract-first OpenAPI + generated clients.
- `apps/node-agent/`
  - Ensure Renogy collector enablement can be toggled via config and reports status.
- `docs/runbooks/`
  - Update Renogy setup docs to include the dashboard flow.

## Acceptance criteria
1) A dashboard user can configure a node for Renogy by providing only:
   - BT-2 MAC
   - (optional) poll interval override, default 30s
2) Default Renogy sensors are created with 30s interval and appear in the node sensor list.
3) Trend data is visible for these sensors.
4) Running setup twice is idempotent (no duplicates).
5) Errors are actionable (UI contains a “What to check” list).
6) Existing Renogy CLI deployment flow remains valid; no regressions to `make e2e-web-smoke`.

## Test plan
- Unit tests:
  - preset sensor list parity (dashboard vs CLI tool templates)
  - idempotent config mutation behavior
- Integration:
  - Pi 5 simulator with Renogy ingest enabled publishes telemetry and dashboard shows data at 30s cadence.

## Dependencies
- Node Agent epic is hardware-blocked; validate on real Renogy hardware before marking Done.

## Risks / open questions
- Whether the dashboard should support BT-2 discovery (scan) from the node, or require manual MAC entry for MVP.
- Determine the canonical source for the “default sensor set” to avoid drift (templates vs shared JSON).
