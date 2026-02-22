# Node Agent Instructions

## Context
This directory (`apps/node-agent`) contains the software that runs on the actual IoT hardware (e.g., Raspberry Pi 4/5, Zero 2 W). It acts as both a local controller for hardware sensors and a gateway to the Core Server.

## Generic Pi 5 Stack (Project Requirement)
- Every Raspberry Pi 5 node must ship the **same installed software stack** (same image/kit + same systemd units).
- Features are enabled/disabled per node via config/capabilities (no per-feature images).
- Optional secondary services (e.g., `renogy-bt.service`) should be **installed everywhere** but **disabled by default** unless configured.
- Avoid install-time network fetches for optional services; prefer in-repo/offline-capable packaging.
- Optional services must be toggleable via config updates (no SSH required after install). Use the systemd watcher:
  - `node-agent-optional-services.path` (watches `/opt/node-agent/storage/node_config.json`)
  - `node-agent-optional-services.service` → `/usr/local/bin/node-agent-optional-services`

## Tech Stack
- **Runtime:** Python 3.11+
- **Framework:** FastAPI (runs a local web interface)
- **Key Libraries:** `aiomqtt`, `psutil`, `zeroconf` (mDNS), `bleak` (BLE).
- **Hardware Interaction:** `gpiozero` (Pi 5 compatible GPIO), `spidev` (SPI), Serial (for solar controllers).

## Project Structure
- `app/main.py`: FastAPI app creation + lifespan wiring (drivers/services, launchd/systemd integration) and router registration.
- `app/routers/`: HTTP route handlers for `/` and `/v1/*` endpoints.
- `app/schemas.py`: Pydantic request/response payloads for the HTTP API.
- `app/ui.py`: Embedded HTML/JavaScript for the local "Emergency/Setup" dashboard.
    - *Warning:* The HTML/JS is embedded as a Python string. Be careful when editing.
- `app/hardware/`: Drivers for specific sensors (ADS1263 HAT analog, Pulse Counters, etc.).
- `app/services/`: Background tasks (Mesh adapter, BLE provisioning, Telemetry publisher).
- `app/config.py`: Settings management (Pydantic-settings).

## Development Conventions

### Project Management Status (Hardware Work)
- Hardware-dependent work should be tracked as two tasks: **Implement …** (code + non-hardware tests) and **Validate … on hardware** (`Blocked: hardware validation (...)`). Do not leave hardware-waiting work as `In Progress`.

### Embedded Local Dashboard
- The Node Agent serves a standalone HTML/JS dashboard at `http://<ip>:9000`.
- This UI allows configuration *without* a connection to the Core Server.
- Code for this lives in `app/ui.py` (render + script bundle) and is served by `app/routers/root.py`. **Preserve this functionality.**

### Connectivity & Provisioning
- **mDNS:** The agent advertises itself as `_iotnode._tcp.local.`.
- **BLE:** Uses `bleak` to listen for provisioning credentials (WiFi SSID/Pass, Tokens) from the iOS App.
- **MQTT:** Publishes telemetry to the broker defined in settings. Buffer data if offline.
- **Systemd Watchdog:** The service must ping systemd via `sd_notify` (handled by `app.utils.systemd.SystemdNotifier`). Blocking the main loop for too long will trigger a restart by the OS.

### Persistence
- Configuration is stored in `storage/config.json`.
- Do not hardcode node-specific settings; rely on the `ConfigStore` service.
- **Git Hygiene:** `storage/*.json` files are runtime state and should be ignored (use `.example.json` for templates).

## Reference Documentation
- **Root Guide:** `../../docs/DEVELOPMENT_GUIDE.md` (Runtime files, Git hygiene)
- **Node Docs:** `../../docs/node-agent.md`

## Common Tasks
- **Adding a Driver:**
    1. Create class in `app/hardware/`.
    2. Instantiate in `lifespan` in `app/main.py`.
    3. Register with `TelemetryPublisher`.
- **Updating Local Dashboard:**
    1. Edit the `script_bundle()` or HTML string in `app/ui.py`.
    2. Ensure the JS uses the local API (`/v1/...`).
