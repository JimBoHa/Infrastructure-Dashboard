# FEATURE-003: Deployment Option — Preconfigured Storage Media (Pi 5)

## Summary
Ship a **versioned, operator-friendly** Pi 5 “preconfigured media” workflow so a non-technical user can create a bootable SD/USB device with node-agent installed in **<= 5 minutes** and **<= 10 clicks**, without SSH and without manual post-flash file surgery.

This ticket is explicitly about **productizing and simplifying** the existing imaging + first-boot approach already present in the repo, not inventing a brand-new deployment system.

## Business goal
Reduce installation friction and support burden by making “add a new node” a repeatable, low-skill workflow.

## Raw inputs (from feature checklist)
- Preconfigured storage media with node software already loaded from a Pi 5 flasher (or similar app).

## Current state in the repo (do not rebuild)
- The repo already contains:
  - SD-card imaging and flashing scripts for generic nodes.
  - An overlay + first-boot script flow and a Raspberry Pi Imager profile generator.
  - Renogy-specific bundle generation that relies on “copy files to boot volume” after flashing.

This ticket closes the usability gap: **remove or minimize the manual copy steps**, and align the Pi imaging artifacts with the broader “versioned bundle” philosophy used by the installer/setup stack.

## Scope
### In scope
- A single downloadable “Pi 5 node image kit” per release that contains:
  - Raspberry Pi Imager profile JSON
  - overlay payload(s)
  - first-boot script(s)
  - checksums + version metadata
- A simplified operator workflow that requires:
  - flash
  - insert/boot
  - scan/adopt
- Documentation/runbook updates and “happy path” video/gif optional.

### Out of scope
- Network boot (FEATURE-005).
- Remote SSH deployment (FEATURE-004; already exists but may be leveraged as an alternative path).
- Any new hardware drivers.

## Recommended build strategy (reuse-first)
1) Continue using **Raspberry Pi Imager** as the flashing UX (do not build a flasher app from scratch).
2) Use the existing overlay + first-boot mechanism, but:
   - consolidate artifacts into a single “kit” zip
   - provide a clear, minimal “operator steps” flow
   - optionally add a helper script that copies required files to the boot volume automatically on macOS.

## Functional requirements
### FR1. Versioned image kit artifact
For each release, publish a directory or zip containing:
- `node-agent-imager.json` (profile)
- `node-agent-overlay.tar.gz`
- `node-agent-firstboot.sh`
- `SHA256SUMS` (or equivalent)
- `VERSION` file (explicit semver + git commit)

### FR2. Zero-config adoption path (preferred)
- The flashed node must be able to boot and enter a discoverable/adoptable state without preloading:
  - adoption tokens
  - site-specific node_config
- Adoption + configuration should flow from existing scan/adopt + config push mechanisms.

### FR3. Optional offline pre-seeding (secondary)
If the operator wants to pre-seed configuration (e.g., remote site without a running core server at first boot), support copying a small file set to the boot partition:
- `node-agent-firstboot.json`
- optional `node_config.json`

But this must remain optional.

### FR4. Operator workflow (documented)
Document a “happy path” that requires:
1) Open Raspberry Pi Imager
2) Select OS image (Pi OS Lite 64-bit recommended)
3) Apply the provided profile and first-boot script
4) Flash
5) Boot node; adopt via dashboard

## Non-functional requirements
- Artifacts must be reproducible enough for support:
  - version is visible on the node
  - checksums exist
- No secrets embedded in the generic kit.
- The imaging workflow must not require developer tools (no Python/Node installs on the operator machine).

## Repo boundaries (where work belongs)
- `tools/`
  - existing image/profile generation should be extended to emit a single “kit” output with version metadata.
- `docs/runbooks/`
  - add/update a “Pi 5 preconfigured media” runbook.
- `apps/setup-app` (optional integration)
  - optionally surface a download link to the correct kit from Setup Center, but do not block on this.

## Acceptance criteria
1) A release artifact exists that contains the full image kit and checksums.
2) A non-technical operator can flash a Pi 5 using Raspberry Pi Imager and the kit with no CLI usage.
3) The node boots, is discoverable, and can be adopted/configured from the dashboard.
4) The node reports its version/build info in `/v1/status` or equivalent.
5) If optional offline pre-seeding files are present on the boot volume, first boot consumes them successfully and the node runs with that configuration.
6) Existing E2E gates remain green (`make e2e-web-smoke`, installer gates unaffected).

## Test plan
- CI: validate that kit generation is deterministic at least at the “structure + checksums present” level.
- Manual: run the documented operator workflow on a real Pi 5.

## Dependencies
- None (uses existing tooling), but should align with release/versioning conventions used by the installer.

## Risks / open questions
- How far to go to eliminate post-flash file copying on macOS (helper script vs accept one manual copy step).
- Whether the kit should include a fully baked `.img` (large) or stick to overlay+Imager (smaller, more flexible).
