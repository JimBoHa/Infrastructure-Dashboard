# TICKET-0026: installer-launcher-rewrite-(swift-no-applescript-embedded-controller-dmg-preflight-quarantine)

**Status:** Done

## Description
Replace the current AppleScript-based installer launcher with a standard macOS-friendly launcher that:

- does **not** use AppleScript (`osacompile`, `do shell script … with administrator privileges`)
- embeds the controller bundle DMG **inside** the launcher app bundle (`Contents/Resources/...`) so it is **not** visible on the mounted DMG root
- opens the Setup wizard via `NSWorkspace` (not AppleScript)
- prompts for admin **only** when installing LaunchDaemons (wizard runs as the user)
- avoids false “warn” preflight results on a clean install (no self-port warning; not-root is informational)
- removes the need for manual quarantine stripping (`xattr -dr com.apple.quarantine ...`) by making controller DMG mounts quarantine-safe (copy + strip xattr before `hdiutil attach`)

## Scope
- [x] Replace AppleScript launcher build in `farmctl installer`/`farmctl dist` with a native Swift launcher build.
- [x] Update installer DMG layout so only the launcher app is visible on mount.
- [x] Update controller DMG auto-detection to support embedded DMG in the app bundle.
- [x] Fix preflight semantics so a clean install shows no warnings.
- [x] Make controller DMG mounting robust to quarantine (no manual `xattr`).
- [x] Update docs + PM tracking accordingly.

## Acceptance Criteria
- [x] Mounted installer DMG shows only the launcher app (no visible controller DMG).
- [x] Launcher starts the wizard and opens the browser via `NSWorkspace`.
- [x] Production: admin prompt happens only when installing LaunchDaemons (not when opening the wizard).
- [x] Preflight shows **no** warnings on a clean machine with only the wizard running.
- [x] Installing from a downloaded/quarantined DMG no longer fails with `hdiutil: attach failed - Resource temporarily unavailable`.
- [x] `make e2e-installer-stack-smoke` passes from a verified clean state (preflight/postflight test hygiene).

## Follow-up
- When code signing/notarization is in place, replace the current stopgap escalation with a proper privileged helper (`SMJobBless`) or a `.pkg` install step.

## Notes
- Preferred admin escalation mechanism: **standard macOS authorization prompt**, not AppleScript. If SMJobBless is not feasible yet (code signing / notarization not in place), use the best available stopgap and track a follow-up to migrate to SMJobBless once signing is available.
- This ticket is the implementation anchor for: `SETUP-10`, `SETUP-22`, `SETUP-24`, `SETUP-25`, `SETUP-26` in `project_management/TASKS.md`.
