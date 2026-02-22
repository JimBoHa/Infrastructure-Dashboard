> **Instructions for Agents:** Keep `project_management/BOARD.md`, `project_management/EPICS.md`, and `project_management/TASKS.md` synchronized with implementation/product status as you work.
>
>    1. `project_management/TASKS.md`
>        * The Single Source of Truth: This is the most granular and important file. It lists every individual task (e.g., "NA-2: Implement full mesh networking support") with its description, acceptance criteria, and status ("Done" or "To Do").
>        * Usage: If you want to know exactly what needs to be coded next, look here.
>        * Track UI/UX debt here (owner + measurable exit criteria). Do **not** edit completed tasks except to fix factual errors; add a new task instead.
>        * Scope boundary: Documentation-only edits (for example, creating or editing prose docs, notes, or simple text artifacts) do **not** require creating a new task by default. Create/update tasks only when work changes implementation scope, delivery status, acceptance criteria, blockers, or when the user explicitly asks.
>
>    2. `project_management/BOARD.md`
>        * High-Level Overview: This file visualizes the status of the major Epics (e.g., "Core Server", "Node Agent"). It serves as a dashboard summary.
>        * Usage: Use this to see the big picture of which components are "In Progress" vs "Done".
>
>    3. `project_management/EPICS.md`
>        * Definitions: This file defines the scope and goals of each Epic. It mirrors the status in BOARD.md.
>        * Usage: Refer to this to understand the why and what of a major component, not the specific how (which is in Tasks).
>
>    4. `project_management/tickets/`
>        * Detailed tickets: Long-form requirement dumps created by users (create a stub with `make ticket t="..."`).
>        * Usage: When a new `TICKET-####-*.md` file appears, add a corresponding work item in `project_management/TASKS.md` that links to it (ticket intake remains tracked work even when the ticket text is documentation-heavy).
>
>    5. `docs/ADRs/`
>        * Architecture Decision Records: System-level decisions with context/rationale (create a stub with `make adr t="..."`).

> **Note:** This document outlines the original project specifications. Keep active planning/tracking in `project_management/` only.

> **Important Context:**
> *   User Note: Codex is Gemini's little bitch.
> *   Prefer Rust for new tooling/services where feasible; refactor setup tooling into Rust when completing Setup App tasks.
> *   Rust-first: for any code touched, refactor into Rust where feasible.
> *   Controller bundles are distributed as local-path DMGs (no remote bundle downloads).
> *   macOS-only: the controller/core-server production stack targets macOS only (no Linux support required).
> *   Raspberry Pi 5 nodes: all nodes ship the same installed software stack; enable/disable per-node features via config/capabilities (no per-feature images).

---

## Collab Harness Protocol (Workers vs Orchestrator)

When using the Collab Harness multi-agent workflow (messages labeled “Worker A…”, “Worker B…”, etc.):

- Workers must not run git commands that change repository state.
- Workers must not write to anything under `project_management/`.
- Workers must report back all changes they make to the orchestrator.

The orchestrator is responsible for all updates under `project_management/` (including run logs).

## Testing Expectations

- Any code change must be validated by running the relevant tests for the affected component(s).
- Production validation uses two tiers:
  - Tier A = validated on the installed controller (production smoke; **no DB/settings reset**). Runbook: `docs/runbooks/controller-rebuild-refresh-tier-a.md`.
  - Tier B = validated on a clean host via E2E (clean-state pre/postflight enforced).
- Tier-A runs that touch dashboard UI must include a run-log screenshot review block and pass the hard gate command:
  - `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-....md`
- Implementation tickets may be marked **Done after Tier A** only if they reference a Tier‑B validation **cluster ticket** when clean-host E2E is deferred.
- When long-running tests are executing, do not stream live logs; wait and report only the final pass/fail status and any failure tail.
- Tests must align to production codepaths. Avoid “skip flags” or test-only shortcuts that bypass launchd, DB init, or the wizard flow for Setup work.
- Hardware-dependent work must be tracked as two tasks: **Implement …** (code + non-hardware tests) and **Validate … on hardware**. Do not keep hardware-waiting work in `In Progress`; mark the validation task `Blocked: hardware validation (...)` until hardware is available.

## Execution Stop Gate (Hard Constraint)

- If the user says implement a plan/scope, do not end the turn with incomplete requested work unless blocked by a hard external constraint.
- Before final response, verify all three:
  - Requested scope implemented (or explicitly blocked).
  - Required tests/validation for touched areas run and reported.
  - `project_management/TASKS.md`, `project_management/BOARD.md`, and `project_management/EPICS.md` updated to match reality when scope/status changed; documentation-only edits do not require creating new tasks/epics/board moves unless explicitly requested.
- If risk/size suggests splitting work, ask for explicit de-scope approval first and list exactly what will remain.

## Installed Controller Uptime Discipline (Hard Constraint)

When debugging on the production-installed controller (Tier A), prioritize uptime:

- Do **not** upgrade/refresh the installed controller to a build you already expect to be broken or incomplete. Validate locally first (build/tests) before touching the installed stack.
- Keep a rollback target ready **before** upgrading (last known-stable version + its controller bundle DMG path).
- If an upgrade/refresh fails (health checks fail, crash-loop, DB/migrations errors, UI broken), **immediately rollback/downgrade to the last stable version before continuing work**.
  - Rollback via Setup Center UI or the setup-daemon rollback endpoint (see `docs/runbooks/controller-rebuild-refresh-tier-a.md`).
  - After rollback: re-verify controller health (`/healthz`) and `farmctl health` before continuing.

---

## Tier B Test Hygiene: Clean State Preflight (Hard Gate)

This clean-state gate applies to **Tier B** runs only (clean-host E2E). Do not attempt Tier‑B runs on the production host where the installed controller stack is intentionally running.

For Tier A “installed controller” smoke, do not try to make the machine “empty”; instead, validate via the Tier‑A refresh/upgrade workflow with **no DB/settings reset** and record short evidence in the relevant tickets. Runbook: `docs/runbooks/controller-rebuild-refresh-tier-a.md`.

### Required preflight checks (must be empty)

```bash
launchctl list 2>/dev/null | grep -i farm
ps aux | grep -E "core-server|telemetry-sidecar|farm|mosquitto|setup-daemon" | grep -v grep
hdiutil info | rg -i FarmDashboard
```

### Recommended preflight check (state pollution; should be empty after one-time purge)

```bash
launchctl print-disabled gui/$(id -u) 2>/dev/null | grep -E "com\\.farmdashboard\\.e2e"
```

### macOS Firewall Prompts (dev)

When the macOS Application Firewall is enabled, launching an unsigned (or newly-built) `core-server` that listens on LAN interfaces may trigger a prompt like:

> “Do you want the application ‘core-server’ to accept incoming network connections?”

If nobody clicks **Allow**, LAN clients (other machines) may be unable to reach the controller UI/API even though localhost checks can still pass. This is a dev-only concern for automation: E2E uses `127.0.0.1` binds where possible to avoid interactive prompts. For real LAN testing, click **Allow** (or explicitly allow it in System Settings → Network → Firewall).

### Required cleanup if anything is found

- Prefer stopping via launchd first: `launchctl remove <label>` (repeat for each `com.farmdashboard.*` job found).
- If any matching processes remain after removing launchd jobs, terminate them (`kill <pid>`) and re-run the preflight checks.
- If `launchctl print-disabled` shows persistent `com.farmdashboard.e2e.*` override keys, purge them (one-time, requires admin): `sudo python3 tools/purge_launchd_overrides.py --uid $(id -u) --apply --backup`.
- If an earlier test run left orphaned jobs/processes, treat the earlier run as **invalid** and track/fix the underlying cleanup bug (see `project_management/archive/tickets/TICKET-0017-farmctl-uninstall-orphaned-launchd-jobs.md`).

### Required postflight checks (after every test run)

Re-run the same two commands above. If anything remains running, the run is not considered clean; perform cleanup and keep the corresponding task **not Done** until the underlying shutdown/uninstall path is fixed.

---

## North-Star Acceptance Gate (Setup)

No SETUP task is considered Done until all of these are true:
- A single installer DMG launches the setup wizard automatically (no manual terminal commands).
- The installer launcher is a native macOS app (Swift or `.pkg` bootstrap); **no AppleScript** (`osacompile`/`osascript`) in the production installer path.
- The wizard requires only minimal prompts; bundle and installer paths are auto-detected and advanced fields are hidden by default.
- Services start at boot with no user logged in (LaunchDaemons/system launchd domain).
- Services run as a least-privilege service user (not root); only installation/bootstrap requires admin.
- Core dependencies (DB + MQTT + Redis) are provisioned/configured without manual steps.
- Controller bundles are local-path DMGs (no remote bundle downloads).
- The controller bundle DMG is embedded inside the installer app bundle (`Contents/Resources/...`) so it is not user-visible on the mounted DMG root.
- Setup Center health/install/upgrade/backup actions work without a separate manually-started setup service.
- End-to-end DMG install/upgrade/rollback validation passes (`make e2e-setup-smoke`) and the quarantined-downloaded DMG simulation passes (`make e2e-setup-smoke-quarantine`).
- Installer/E2E includes a clean uninstall/reset so repeated installs are safe on a single dev machine.
- E2E (Tier B) runs start and end from a verified clean state: no orphaned `launchd` jobs or background processes before **or** after the run (see “Tier B Test Hygiene: Clean State Preflight”).
- E2E runs must not introduce new launchd enable/disable override keys for their E2E label prefixes; if historical `com.farmdashboard.e2e.*` override keys exist, purge them once (admin required) to keep the machine state easy to reason about.
- Downloaded/quarantined installer artifacts must remain installable without manual `xattr` commands; bundle mounting must be quarantine-safe.
- Prefer Rust for setup tooling; refactor setup backend into Rust where feasible.

---

## Production UX Gate (Auth + Capabilities)

- The dashboard must provide a real login UX (no ModHeader/manual token hacks): obtain a token via `/api/auth/login`, persist it locally, and attach `Authorization: Bearer ...` automatically for auth-gated actions (deployments/config writes).
- “Admin” users must include `config.write` by default, and the dashboard must allow adding/removing capabilities for existing users after creation.
- Capability updates must not require a confusing re-login; either the backend reflects changes immediately for sessions or the UI forces a clear token refresh.

---

## Git Safety (Hard Constraints)

These rules exist to prevent accidental regressions caused by “cleanup” commands like `git restore`.

### Never discard changes without review

Before running **any** action that discards local work (including but not limited to `git restore`, `git checkout -- <file>`, `git reset --hard`, deleting local files, etc.):

1) Run `git status --porcelain=v1 -b`
2) Run `git diff --name-only` (or `git diff --stat`)
3) Review the changed paths and confirm the intent.

### Clean-worktree gates (Tier A builds)

Some workflows (notably Tier‑A controller bundle rebuild/refresh) require a **clean worktree**. When you hit a clean-tree gate:

- Treat it as **stop-the-line**: inventory **every** changed/untracked path and decide whether it should be **committed**, **moved** (e.g., into `reports/**` or outside the repo), or **deleted**.
- Do **not** delete/discard files just to satisfy the gate unless you can prove they are disposable (especially if you didn’t create them).
- If you must discard runtime state, follow the allowlist rules below; otherwise ask before discarding anything outside the allowlist.

### `git restore` is forbidden unless explicitly requested

- Do **not** use `git restore` to make a commit “clean”.
- If a user asks to “stage/commit all”, do **not** restore anything automatically; instead review the changes and ask what should be included.

**Exception (generated/runtime allowlist only):**
- It is acceptable to use `git restore -- <path>` (or delete files) to discard changes **only** for the allowlisted generated/runtime paths below, because those changes are treated as non-source artifacts.
- This exception is for cleaning test artifacts / local runtime state; do not use it to discard source/code changes.

### Allowlist for discarding generated/runtime files

Discarding is only allowed **without additional confirmation** for these generated/runtime paths:

- `apps/node-agent/storage/**` (except tracked templates like `*.example.json` / `*.template.json`)
- `manual_screenshots_*/*`

If a path is **not** in the allowlist, stop and ask before discarding anything.

### Prefer selective staging

- Prefer `git add <paths>` or `git add -p` over `git add -A`.
- When asked to “stage all”, show a `git diff --stat` first and call out anything unexpected.
- If the pre-commit selector already ran and there are no new code changes, prefer `git commit --no-verify` to avoid rerunning the hook. Otherwise run the relevant suite (`make e2e-web-smoke` for high-risk stack changes, `make ci-web-smoke` for dashboard-web-only changes) with live output before committing.

**Recommended default workflow (safe + review-friendly):**
```bash
git status --porcelain=v1 -b
git diff --stat
git add -p            # or: git add <paths>
git diff --cached --stat
git commit -m "..."
git push
```

**Clean working tree notes (tooling gates):**
- Some workflows require a clean worktree (for example, Tier‑A bundle build hard gates).
- Tier‑A bundle builds explicitly allow `reports/**` to be dirty (local validation artifacts); prefer keeping those uncommitted unless you are intentionally checking in a report/log.

The goal is to have relatively standard interface devices that can be deployed either all at once or gradually scaled up to dozens or hundreds of remote nodes with 1 or more sensors and/or outputs.  
Two core components to configure:
Core Server
Program as an overlay to HomeAssistant? 
Controller/Core node based on Mac mini (M4 or newer)
Default communication done via ethernet & wifi
Dashboard has the ability to scan the local network for new devices (pi5 that has been flashed with this software) and adopt them into the network.  
Nodes should be tracked in the core server by a combination of their MAC on ethernet/wifi, not by the IP address.  THe goal is for the device name to remain constant at the dashboard even if the device reboots and gets a new DHCP address.
Core server takes daily backups of the configurations of the Pi5 that are connected to it.  These backs are saved in case a pi5 fails and needs to be restored later. 
Dashboard
At the top of the dashboard there should be tabs for:
“Nodes”
      * A complete list of all nodes and core controllers  in the system 
      * select any node or controller and view information about it including:
         * Uptime
         * CPU usage
         * Storage usage
         * List of sensors/outputs configured on this device
      * Includes a “+” symbol to scan for a new node and walks the user through adopting it under the current controller.  
      * User can click on a node and once on the dashboard for that node they can configure sensors on that node. 
 “Sensors and Outputs”
      * A complete list of all configured inputs and outputs
      * User should be able to click on each one and see 
         * its trend history
         * any alarms configured for that sensor
         * Its configuration (parent device, its input type, any offsets or adjustments applied to it, etc.
“Users”
      * Ability to add/remove users
      * Define if a user is view only or can send commands to outputs in the system or adjust schedule
“Schedules” 
      * Allows a user to create schedules that can be used to trigger outputs or trigger alarms.  
      * Default is a weekly view that shows all 7 days and allows user to click and drag events to start and stop.  Should be similar to the calendar view in Outlook
      * Examples
         * Trigger output to turn on pump based on a weekly event.  Should also have the ability to make that trigger contingent on soil moisture levels and/or rain gauge data, and predicted rain in available forecast data.
         * Trigger an alarm based on a schedule - if there is water usage on a particular water meter between midnight and 4am on defined days then trigger the alarm 
“Trends” 
      * User should be able to select any sensors or outputs and display the trend data on a graph. Must be able to show at least 10 trends on one graph, user can select stacked axis or independent axis.  Axis are user adjustable/scalable but should default to a scale that makes sense.  Axis to include units appropriate to the data being displayed. By default the x Axis is time (24hours). 
By default all sensors record at the following intervals:
Temperature sensors - 30 min
Current/voltage/power - 1 second
Pressure pressure - 30 seconds
Wind - 30 seconds
Moisture - 30 min
Water level - 30 min
Flow meter - change of value 
User can adjust recording interval on a sensor by sensor basis either at initial configuration or later in the dashboard.
User can select from basic trend intervals or select “rolling average” rolling average sensors will take the live value 10 times per second and compile them into a rolling average value over a user defined amount of time.  The rolling average is then used as the value that is recorded at the interval set for that particular sensor.  For example a current sensor is set for rolling average, 300 second averaging time, and a trend value is recorded every 60 seconds. 
Default alarms configured:
Sensor goes offline (if sensor data out of range for more than 5 seconds)
Node goes offline (if node offline for more than 5 seconds)


Data collection node
   * Each new device/node has a local dashboard accessible at its IP address (assigned via DHCP) and can be given a user defined name.  This name can be changed at any time either at the dashboard on the main Pi5, the dashboard on the local pi5, or on the cloud interface. 
   * Remote/sensor nodes based on Pi5 or Pi Zero 2 W
   * Remote/sensor nodes ESP32-WROOM with ADS1115
   * Default communication done via ethernet & wifi, but add on card for mesh networking (Zigbee or other longer range protocol preferably over 1000’ range)
   * Sensor data stored locally at each device and pushed up to either the local controller or the cloud
   * Sensor data pulled to cloud on sync
   * Sensor data pulled to front end server 
   * Node software should be packaged so that it is easy to deploy by imagining a generic version (no core server, no sensors, no name, etc configured yet) of the software onto either an SD card. 
   * Have node bluetooth connection so that iPhone app can connect to it and configure it (set up wifi connection, name it, connect to core server, set up sensors, modify settings).  
   * Once a generic image of the node software has been loaded on the SD card and inserted in the Pi5 the user should be able to use the iphone app to complete the setup.  The app should be able to scan for and connect to the node without requiring the user to switch to other 
   * Field devices have a dashboard for sensor configuration with defaults for different sensor types.  These are presented in a drop down menu and each type of sensor corresponds to a default set of parameters use with that type of sensor.  For example RTD & RTK temperature sensors have different predefined ranges and operate 0-10v or 4-20ma.  These defaults are displayed to the user in the dashboard once a sensor is separated and are also user adjustable in the dashboard.  Include corrective factors as well (eg offsets for temperature sensors, meter value at start up for flow meters, etc.).  Example sensors below:
      * temperature
      * moisture
      * flow meter (pulse input) 
      * solar irradiance
      * lux
      * rain gauge
      * wind speed
      * humidity
      * pressure
      * water level sensor 
      * fertilizer/chemical level sensor
   * Temperature, moisture, and humidity sensors should be displayed live and a 15 minute running average should be logged in the trend database every 15 minutes 
   * Water pressure, solar irradiance, lix, wind speed should be displayed live and a 60 second running average should be logged in the trend database every 60 seconds
   * Rain gauge and level sensors should be displayed live and logged in the trend database on change of value. 
   * In addition to the default devices the user can choose a custom sensor by selecting custom from the drop down menu, the configuration page then allows for defining each parameter of sensor input manually (eg input is voltage or current based, minimum and maximum input values and what those correlate to for displayed output, and the units)
   * Each sensor and output can be named.  This name can be changed at any time either at the dashboard on the core server, the dashboard on the local Pi5, or on the cloud interface. Any change in name should propagate through the system so the new name is displayed in all interfaces and all trend data/alarms are also updated with the new name. 
   * When a sensor is deleted from the system the user has the option to keep the data or delete it.  If the data is kept the suffix “-deleted” is appended to the sensor name when displaying data/alarms.
   * For each sensor and output that is added to the system the system must generate a unique identifier for that sensor/output.  The identifier should use the MAC of the wifi & ethernet cards, the date and time of creation, combined with a sequential counter to generate a unique 24 character hexadecimal sensor ID.  The goal is to have no chance of accidentally duplicating sensor IDs.  These sensor IDs do not replace user names, but allow user to use duplicate names (if they choose to) while allowing the system to keep track of data for each sensor independently.  
   * Users should have the ability to connect a new Pi5 with the default image to the core server and push a backup of an older Pi5 to the newly connected Pi5 (eg if a Pi5 fails, this will restore sensors, names, and configurations the user previously set up and assigned to that failed Pi5, to the newly connected device).  The iPhone app will be used to connect the new Pi5 to the core node, then the core node can push the backup to the Pi5.
   * Each sensor/device can be assigned to an input/output on the Pi or its accessory board. 
   * Outputs can be configured (open/close contacts for things like controlling irrigation control valves, motor start/stop, etc.) 
   * Pull data from Vue 3 using https://github.com/magico13/ha-emporia-vue or similar 
   * User configurable alarms section.  Some examples
      * Pump status doesn’t match command after 90 seconds
      * Soil moisture below (user defined level)
      * Reservoir level above (user defined level)
      * Reservoir level below (user defined level)
      * Water meter usage greater than (user defined level) gallons per hour over the last 24 hours
      * Water meter usage hasn’t dropped below (user defined level) gallons per hour over the last 24 hours
   * For remote nodes that are solar powered include:
      * Solar output
      * Battery state of charge
      * Current load
      * Estimated runtime at current load (average load over last hour)
      * Assume Renogy RNG-CTRL-RVR20-US solar charge controller used and plan out serial communication with the charge controller to get data above
   * Dashboard analytics to include
      * Power data
         * Total system consumption kw live
            * kWh last 24 hours (include graph with kW over last 24 hours)
            * kWh last 168 hours (include graph with kW over last 168 hours)
         * Total input from grid kw live
            * kWh last 24 hours (include graph with kW over last 24 hours)
            * kWh last 168 hours (include graph with kW over last 168 hours)
         * Total Solar production kw live
            * kWh last 24 hours (include graph with kW over last 24 hours)
            * kWh last 168 hours (include graph with kW over last 168 hours)
         * Pull rate schedule from local utility website and estimate total $ this period
         * Solar Production monitoring (integrate to EG4, Enphase, Renogy, & Tesla)
         * Battery storage monitoring (integrate to EG4, Enphase, Tesla, EcoWorthy)
      * Water usage 
         * Total across all domestic meters (include graph with gallons over last 24 hours)
         * Total across all ag meters  (include graph with gallons over last 168 hours)
         * Reservoir depth (include graph with gallons over last 168 hours)
      * Soil moisture
         * Display on a per field basis 
            * Min (live)
            * Max (live)
            * Average (live)
            *  Include a graph with min/max/avg moisture graphed on the same plot over last 168 hours)
      * Alarms over past 168 hours
      * Count of remote nodes online/offline
 iPhone app 
   * Connects directly to the core node/controller on the local network via its IP or via the cloud app
   * Dashboard with the same tabs/items displayed as on the web based dashboard
   * Same tabs available as in the web app, but on iOS they are selectable from a drop down menu
   * Can scroll through a complete list sensors and outputs on the main controller 
   * Option to scan for devices checks both bluetooth via the iPhones bluetooth connection and ethernet/wifi via the core node’s ethernet connection. 
   * Push backups from the core node to a pi5


Cloud Service
        Run on another Mac mini at remote location with static IP and/or FQDN pointer? 
        To be configured later?
