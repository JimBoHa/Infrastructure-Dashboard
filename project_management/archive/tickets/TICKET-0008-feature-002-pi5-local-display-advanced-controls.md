# FEATURE-002: Pi 5 Local Display (Advanced Controls + Trends)

## Summary
Extend FEATURE-001 local display to support **touch-first navigation**, **trend visualization**, and **optional output controls**, while preserving the platform’s existing authorization/capability model and avoiding any new “shadow control plane”.

## Business goal
Enable on-site operators to (a) diagnose issues faster by viewing recent trends and (b) take limited corrective actions locally without pulling out a laptop or phone.

## Raw inputs (from feature checklist)
- “Really advanced features”:
  - controlling outputs from the touch screen
  - viewing trend data

## Current state in the repo (do not rebuild)
- Output control already exists in the platform (core API + capability checks).
- Dashboard-web already renders trends and output controls in a full browser environment; we should reuse data shapes and authorization rules rather than inventing new ones.

## Scope
### In scope
- Add additional display pages/routes:
  - Trends page (sparkline/small chart) for selected sensors.
  - Outputs page (optional) to issue output commands.
- Add a display navigation model appropriate for touchscreen.
- Add safe gating for output controls (explicit enable + local confirmation).

### Out of scope
- Complex multi-user authentication flows on the Pi display (do not re-implement OAuth/login on the node).
- Editing schedules/alarms from the Pi display.
- Full dashboard parity on the node.

## Functional requirements
### FR1. Touch-first navigation
- Kiosk UI provides a simple navigation bar with at least:
  - Status
  - Sensors
  - Trends
  - Outputs (only if enabled)
- Minimum touch target size: 44px equivalent.

### FR2. Trend visualization
- For each sensor selected in the display profile, show:
  - current value
  - mini trend over a selectable range: 1h / 6h / 24h
- Data source priority:
  1) Core-server trend query endpoint (authoritative historical view).
  2) Local node ring-buffer cache (best-effort fallback when offline).

### FR3. Output control (safety + auth)
Output control must be **off by default** and requires:
1) Node config flag: `display.outputs_enabled=true`
2) Local confirmation step:
   - Option A: short PIN configured in node config (`display.local_pin_hash`)
   - Option B: “press-and-hold 2s” + secondary confirm (acceptable for MVP)
3) Command path:
   - Use the same core-server output command endpoint and capability checks as the main dashboard.
   - Commands must be audited with an explicit actor label (e.g., `actor=local_display`), and include node id.

### FR4. Failure modes
- If trend fetch fails, show a clear error and retry with backoff; do not lock the UI.
- If output command fails, show error details and the last-known output state.

## Config model additions (extends FEATURE-001)
```json
{
  "display": {
    "outputs_enabled": false,
    "trend_ranges": ["1h","6h","24h"],
    "trends": [
      { "sensor_id": "<id>", "default_range": "6h" }
    ],
    "local_pin_hash": null
  }
}
```

## Security requirements
- Output commands must never be executable without:
  - explicit enablement
  - explicit local confirmation
  - existing server-side capability authorization
- Consider binding the display routes to localhost by default; if LAN-accessible, output control must still be protected.

## Non-functional requirements
- Trend visualization must remain smooth on Pi 5:
  - avoid heavyweight charting if it causes frame drops
  - default to sparklines and limited point counts
- No increase to telemetry publish latency beyond 5% in steady state.

## Repo boundaries (where work belongs)
- `apps/node-agent/`
  - Add local trend cache (optional) and kiosk UI pages for trends/outputs.
  - Integrate with existing core trend APIs for historical fetch.
- `apps/core-server/`
  - Ensure trend APIs and output command endpoints support the data needed by the local display.
  - Maintain contract-first OpenAPI + generated clients if schema changes are required.
- `apps/dashboard-web/`
  - Extend display profile editor to include trends + output enablement + PIN configuration (if chosen).
- `project_management/tickets/` + `project_management/TASKS.md`
  - Add linked work items.

## Acceptance criteria
1) With FEATURE-001 enabled, the display shows a Trends page with at least **2 sensors** and selectable ranges (1h/6h/24h).
2) Trend page loads the correct time range from the core server (validated by comparing start/end timestamps).
3) With outputs disabled, Outputs page is not visible.
4) With outputs enabled:
   - local confirmation is required before a command is sent
   - commands are rejected server-side if the user/token lacks the required capability
   - successful commands update the UI state within **5 seconds**
5) All actions are logged/audited (core logs show actor label).
6) `make e2e-web-smoke` remains green; add unit tests for the confirmation gate logic.

## Test plan
- Unit tests:
  - local confirmation gate behavior (PIN or press-and-hold)
  - trend range query construction
- Integration:
  - Sim lab / simulator end-to-end: issue an output command from local display and confirm command reaches the platform command path.
- Manual:
  - Touchscreen usability check on Pi 5 (scroll, tap, confirm flows).

## Dependencies
- FEATURE-001 completed (display route and profile config exist).
- Core capability/auth model must remain the single source of truth.

## Risks / open questions
- Decide whether trends are fetched purely from core or whether a local cache is required for acceptable offline UX.
- Decide the minimum acceptable safety mechanism for output control (PIN vs press-and-hold) for MVP.
