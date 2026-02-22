# Core Server Production Setup (Installer DMG)

This runbook installs the Farm Dashboard controller stack on a fresh Mac mini using a single installer DMG and native launchd services. It is the only supported production path.

At the end, you will have:
- Native Postgres/TimescaleDB, Mosquitto, and Redis running under launchd.
- Core server and telemetry sidecar running as services (the dashboard UI is served by the core server as static assets).
- Services start at boot with no user logged in (LaunchDaemons / `system` launchd domain).
- Services run as a dedicated least-privilege service user (not root). Only bootstrap/installation requires admin.
- A Setup Center health UI plus guided node onboarding.

## North-Star acceptance (production install)

- The installer DMG + bundled installer app launches the setup wizard (no Terminal steps required for installation).
- Minimal prompts; bundle/farmctl path auto-detected; advanced fields hidden by default.
- Controller bundles are local-path DMGs (no remote bundle downloads).
- The install is repeatable and supports clean uninstall/reset.

## Service model (production)

- Production uses LaunchDaemons so the controller behaves like an appliance and survives reboots without a logged-in user.
- Long-running services do not run as root. The installer creates a dedicated service user (default: `_farmdashboard`) and services run under that user/group.
- Only the bootstrap step prompts for admin in order to create LaunchDaemons and the service user.

## What you need (before you start)

- The installer DMG on disk (example: `FarmDashboardInstaller-1.2.3.dmg`).
- A backup destination (default: `/Users/Shared/FarmDashboard/storage/backups`).
- A stable LAN identity for the Mac mini (recommended: DHCP reservation).

Notes:
- The installer DMG embeds a controller bundle and auto-detects it. If you override the bundle, it must be a local DMG path.
- Default paths:
  - Install root: `/usr/local/farm-dashboard`
  - Data root: `/Users/Shared/FarmDashboard`
  - Setup state/config: `/Users/Shared/FarmDashboard/setup/config.json`

## 1) Install from the DMG (recommended path)

1) Double-click the installer DMG to mount it.
2) In the Finder window, open `Farm Dashboard Installer.app`.
3) The setup wizard should open automatically in your browser.
4) Confirm the wizard auto-detects:
   - The embedded controller bundle DMG.
   - The embedded `farmctl` binary path.
5) Provide only the minimal prompts (install root, data root, backup root). Keep Advanced closed unless you are changing ports/paths intentionally.
6) Click **Install**.

What the installer does:
- Unpacks the controller bundle under the install root.
- Installs/initializes native dependencies (Postgres/TimescaleDB, Mosquitto, Redis).
- Writes launchd plists (LaunchDaemons) and starts services.
- Performs health checks and captures logs/diagnostics locations.

If the wizard does not open automatically:
- Relaunch `Farm Dashboard Installer.app` from the mounted DMG (it will reuse an already-running setup daemon if present).
- Or open `http://127.0.0.1:8800` after the setup daemon starts.

## 2) Verify health (Setup Center)

1) Open the dashboard (default: `http://127.0.0.1:8000`).
2) Sign in on the `/login` screen. The dashboard requires login per browser session.
3) Open **Setup Center** (`/setup`) and confirm these are green:
   - Core API
   - Database
   - MQTT
   - Redis
4) Go to **Nodes** and confirm the page loads and “Scan for nodes” is available.

Optional CLI checks (advanced):

```bash
/usr/local/farm-dashboard/bin/farmctl health --json
/usr/local/farm-dashboard/bin/farmctl status
```

Optional launchd checks (advanced):

```bash
sudo launchctl list | rg com\\.farmdashboard
sudo launchctl print system/com.farmdashboard.core-server
```

## 3) Bootstrap admin login (fresh installs)

Fresh installs pre-create a bootstrap admin user:
- Email: `admin@farmdashboard.local`
- Temporary password: printed once in the installer wizard output during **Install**

After signing in, change the password from the dashboard: **Users** → **Set password**.

Advanced API (optional):

```bash
curl -sS -X POST http://127.0.0.1:8000/api/users \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Admin",
    "email": "admin@example.com",
    "role": "admin",
    "capabilities": ["users.manage", "outputs.command", "schedules.write"],
    "password": "CHANGE_ME_LONG_RANDOM"
  }'
```

Notes:
- The `admin` role always includes `config.write` by default.
- You can add/remove capabilities for existing users from the dashboard **Users** tab after signing in (including `config.write`).

## 4) Upgrades and rollback

Preferred: mount the new installer DMG and use the wizard.

Recommended upgrade flow:
1) Double-click the new installer DMG.
2) The installer will detect a running setup daemon and open the wizard.
3) The wizard updates the bundle path to the DMG-embedded controller bundle automatically.
4) Click **Upgrade** and wait for health checks to return green.

Engineering note (dev loop, no admin): to rebuild a controller bundle from source and refresh an already-installed controller using the running setup daemon (stable DMG path + Upgrade), see `docs/runbooks/controller-rebuild-refresh-tier-a.md`.

Advanced CLI (optional):

```bash
# Tip: run privileged CLI actions from a normal directory to avoid `sudo` getcwd warnings.
cd ~

# If you need CLI-only upgrades, mount the new installer DMG and point `--bundle`
# at the DMG-embedded controller bundle (no separate controller download).
hdiutil attach /path/to/FarmDashboardInstaller-<version>.dmg -nobrowse -readonly
sudo /usr/local/farm-dashboard/bin/farmctl --profile prod upgrade --bundle "/Volumes/FarmDashboardInstaller-<version>/Farm Dashboard Installer.app/Contents/Resources/FarmDashboardController-<version>.dmg"
sudo /usr/local/farm-dashboard/bin/farmctl --profile prod rollback
hdiutil detach "/Volumes/FarmDashboardInstaller-<version>"
```

## 5) Backups and diagnostics

Preferred: use **Setup Center** (backup/export actions).

Advanced CLI (optional):

```bash
sudo /usr/local/farm-dashboard/bin/farmctl --profile prod diagnostics --output /Users/Shared/FarmDashboard/support.zip
```

## 6) Guided node onboarding

1) Open the dashboard and go to **Nodes**.
2) Click **Scan for nodes** and follow the guided adoption flow.
3) If using the iOS app for BLE provisioning, ensure the phone is on the same LAN and can reach the controller.

## 7) Clean uninstall / reset (advanced, destructive)

Use this when you need to remove the controller from a Mac mini (or to return to a clean slate during troubleshooting).

```bash
cd ~
sudo /usr/local/farm-dashboard/bin/farmctl --profile prod uninstall --remove-roots --yes
```

## 8) Troubleshooting

- Start with Setup Center health.
- Logs live under `/Users/Shared/FarmDashboard/logs`.
- `mqtt_host` is LAN-facing (for nodes) while `database_url` stays localhost (DB is local-only). This is expected and correct.
- macOS may show “Background Items Added” with the signer name of a bundled dependency (for example, the Postgres build vendor). This is expected; re-signing all bundled components under a single Developer ID is a future hardening step.
- If ports are already in use, open Advanced settings in the wizard, change ports, and re-apply.
- If the setup wizard does not open, rerun `Farm Dashboard Installer.app` from the DMG to restart the bootstrap handoff.
- If issues persist, export diagnostics and attach them to the support ticket.
