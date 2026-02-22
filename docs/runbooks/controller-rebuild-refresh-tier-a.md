# Tier-A Rebuild & Refresh (Installed Controller)

This runbook documents the fast “Tier‑A” workflow used during development to rebuild a controller bundle from source and refresh the already-installed controller at `http://127.0.0.1:8000` **without admin privileges**.

Tier‑A is **not** a substitute for the installer E2E gates; treat it as the quickest way to validate that “what I just built is what the installed app is serving”.

## SOP (copy/paste)

Use this section as the default workflow. Read “Troubleshooting” only if something breaks.

1) Verify the installed controller is up (Tier‑A = no DB/settings reset):
   - `curl -fsS http://127.0.0.1:8800/healthz`
   - `curl -fsS http://127.0.0.1:8000/healthz`

2) Ensure the repo is clean (Tier‑A hard gate):
   - `cd /Users/FarmDashboard/farm_dashboard`
   - `git status --porcelain=v1 -b`
   - `git diff --stat`
   - Expect: no changes. `farmctl bundle` refuses Tier‑A builds from a dirty worktree.
   - Exception: `reports/**` is allowed (logs/local artifacts; not bundled).
   - Recommendation: write logs outside the repo (example: `/Users/Shared/FarmDashboardBuilds/logs/`) to keep commits clean.
   - **If the worktree is not clean:** do **not** “clean it up” by blindly discarding files.
     - Classify every changed/untracked path and choose the correct action:
       - **Real work (source/config/docs/project_management):** stage + commit it (and push if needed) before bundling.
       - **Validation artifacts:** move them under `reports/**` (allowed dirty for Tier‑A) or outside the repo (recommended).
       - **Generated/runtime junk:** only delete if you can prove it is disposable. If you did not create it, treat it as suspicious until confirmed.
     - Avoid `git restore` / destructive cleanup commands as a default. If you must discard runtime state, follow the allowlist rules in `AGENTS.md`.

3) Pick a new version (increment the patch/build number):
   - Example: if installed is `0.1.9.196`, use `0.1.9.197`.
   - Check installed: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`

4) Build a controller bundle DMG (stable path; do NOT use `/Volumes/...`):
   - `mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs`
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.<dev> --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.<dev>.dmg --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.<dev>.log`

5) Point the setup daemon at the new DMG:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{\"bundle_path\":\"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.<dev>.dmg\"}'`

6) Upgrade (refresh the installed controller):
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
   - Verify version: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`

7) Validate Tier‑A (record evidence):
   - Installed smoke: `make e2e-installed-health-smoke`
   - UI evidence: `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt`
   - Draft/update run log under `project_management/runs/RUN-....md`.
   - View screenshots under `manual_screenshots_web/` (Tier‑A validation requires *viewing* the screenshots).
   - Run screenshot hard gate:
     - `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-....md`
   - Tier‑A is not complete until the screenshot hard gate passes.

## Why this works without admin (“reset farmctl”)

In production installs, the setup daemon (`farmctl serve`) runs as the controller service user (default: `_farmdashboard`) under launchd and can run `farmctl upgrade` headlessly.

The **Upgrade** action swaps the installed release symlinks and relies on launchd `KeepAlive` to restart services, which refreshes the installed UI/API without you needing to bootout LaunchDaemons.

## Preconditions

- The controller is already installed (LaunchDaemons running).
- Setup daemon is reachable: `curl -fsS http://127.0.0.1:8800/healthz`.
- Core server is reachable: `curl -fsS http://127.0.0.1:8000/healthz`.
- **Hard gate:** your repo worktree is clean before rebuilding/refreshing. If you build the controller bundle into `/Users/Shared/FarmDashboardBuilds`, `farmctl bundle` will refuse to run unless `git status --porcelain=v1` is empty (except `reports/**`, which is allowed for Tier‑A builds). If the worktree is dirty, inventory + classify changes (commit real work; move artifacts) — do not blindly discard files to satisfy the gate.
- You have a local controller bundle DMG path on disk (not a remote URL).

## Important: use a stable DMG path (not `/Volumes/...`)

Do **not** point the bundle path at a DMG sitting under `/Volumes/...` (mounted installer DMG). That mount path is transient and disappears after unmount, leaving the setup daemon configured with a dead path.

Recommended stable location:
- `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-<version>.dmg`

## Step 1 — Build a controller bundle DMG from source

Example (re-using the installed native deps for speed):

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.<dev> \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.<dev>.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Notes:
- The `--version` string becomes the installed `current_version` in `/usr/local/farm-dashboard/state.json`.
- If you need to rebuild native deps too, see `docs/runbooks/controller-bundle.md`.

## Step 2 — Point the setup daemon at the new bundle (stable path)

### Option A (recommended): Setup Center UI

1) Open the dashboard: `http://127.0.0.1:8000`
2) Go to **Setup Center** → **Controller configuration**
3) In **Controller bundle DMG**, set **Bundle path (DMG)** to your stable DMG path
4) Click **Save**

### Option B (CLI): call the setup-daemon directly

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.<dev>.dmg"}'
```

## Step 3 — Upgrade (refresh the installed controller)

### Option A (recommended): Setup Center UI

Go to **Setup Center** → **Installer actions** → click **Upgrade**.

### Option B (CLI): call the setup-daemon directly

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

## Step 4 — Verify the refresh worked

- Health: `curl -fsS http://127.0.0.1:8000/healthz`
- Confirm the UI assets changed by hard-refreshing the browser tab.
- **If the change touches the dashboard UI:** capture **and view** at least one screenshot of the affected page/section and store it under `manual_screenshots_web/` (reference that file path in the ticket’s Tier‑A Evidence).
- **Hard gate:** complete the run-log screenshot review block and run:
  - `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-....md`
- Confirm installed version (optional):
  - `/usr/local/farm-dashboard/state.json` → `current_version`
  - or `curl -fsS http://127.0.0.1:8800/api/status`

### Required run-log block (for screenshot hard gate)

Add this section to the Tier‑A run log before running the gate:

```md
## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/<TIMESTAMP>/<FILE>.png`
- [x] REVIEWED: `manual_screenshots_web/<TIMESTAMP>/<FILE>.png`

### Visual checks (required)
- [x] PASS: `<FILE>.png` <what was checked and why it passes>
- [x] PASS: `<FILE>.png` <what was checked and why it passes>
- [x] PASS: `<FILE>.png` <what was checked and why it passes>

### Findings
- <Issue/finding or explicit "No blocking issues found" note>

### Reviewer declaration
I viewed each screenshot listed above.
```

## Troubleshooting

- **`bundle_path is required`**: set the bundle path first (Step 2).
- **Setup Center says “Setup daemon base URL not configured”**: the core-server must have `setup_daemon_base_url` configured; the Setup Center proxy route (`/api/setup-daemon/*`) depends on it.
- **Quarantine/xattr warnings**: Tier‑A bundles built locally should not be quarantined. If you pointed at a downloaded artifact that has quarantine metadata, clear it as the file owner and retry:
  - `xattr -dr com.apple.quarantine /path/to/FarmDashboardController-<ver>.dmg`
