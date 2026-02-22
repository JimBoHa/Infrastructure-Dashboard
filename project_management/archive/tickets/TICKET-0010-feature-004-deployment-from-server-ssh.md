# FEATURE-004: Deployment Option — Deploy From Server via SSH (Hardening + UX)

## Summary
Enhance the existing **remote Pi 5 deployment** capability (already implemented in core-server + dashboard-web) to make it safer, more operator-friendly, and more maintainable. This ticket is not “build remote deploy”; it is “finish it like a product”.

## Business goal
Allow a non-expert to bring a freshly installed Pi 5 online using a single guided flow: “enter IP + credentials → deploy node-agent → adopt”.

## Raw inputs (from feature checklist)
- Deploy from server:
  - load default Pi 5 OS 64-bit on storage in Pi 5
  - boot it on the network
  - discover from server
  - server uses SSH to connect and deploy node software

## Current state in the repo (already done, build on it)
- Core-server already exposes a remote Pi 5 deployment job API that installs node-agent over SSH, enables services, and returns adoption metadata.
- Dashboard-web already provides a UI to start a remote deployment and stream progress/logs.

This ticket covers missing product-grade pieces: discovery, credential handling, idempotency guarantees, and operator guidance.

## Scope
### In scope
- “Discover Pi candidates” UX (mDNS + subnet scan optional) to reduce typing IPs.
- Safer SSH handling (host key verification, avoid persisting secrets).
- Idempotent deploy behavior and clearer error messages.
- Ability to choose a deployment profile (generic node vs Renogy bundle vs kiosk display) if profiles exist.

### Out of scope
- Implementing the underlying deployment API from scratch.
- Network boot (FEATURE-005).

## Functional requirements
### FR1. Pi discovery
Provide at least one discovery mechanism:
- mDNS/Bonjour (e.g., `raspberrypi.local`) discovery, and/or
- subnet scan (configurable CIDR) for hosts with SSH open.

The UI should present a list of candidates with:
- IP/hostname
- SSH port
- basic reachability

### FR2. Host key verification
- First connection to a host must surface the SSH host key fingerprint.
- Operator must explicitly approve (or provide a known_hosts entry).
- Subsequent deploys must warn/fail on host key mismatch.

### FR3. Credential handling
- Support password and SSH key auth.
- Any secrets entered in the UI must be:
  - used for the deployment job
  - not stored long-term in the database
  - excluded from logs
- Prefer ephemeral, in-memory handling and clear redaction.

### FR4. Idempotency + rerun safety
- Re-running deploy on the same host must:
  - not duplicate services/config
  - converge to the correct installed state
- Failure mode should recommend next steps (e.g., “SSH auth failed”, “disk full”, “systemd enable failed”).

### FR5. Operator guidance
- UI must include explicit prerequisites checklist:
  - Raspberry Pi OS Lite 64-bit installed
  - SSH enabled
  - network reachable
  - correct user credentials

## Non-functional requirements
- Deployment jobs must remain observable:
  - step-level progress
  - logs are streamed and stored
- Keep `make e2e-web-smoke` green.

## Repo boundaries (where work belongs)
- `apps/core-server/`
  - extend deployment API for discovery support and host key verification if required server-side.
- `apps/dashboard-web/`
  - add discovery UI + improved deploy UX.
- `docs/runbooks/`
  - update deployment runbook(s) with the supported “golden path”.

## Acceptance criteria
1) Operator can select a Pi candidate from a discovery list (no manual IP entry required in the happy path).
2) First-time deploy requires host key confirmation; host key mismatch is detected and blocked.
3) Credentials are not persisted; logs redact secrets.
4) Deploy is idempotent: running deploy twice results in the same installed state and does not break services.
5) UI provides clear prerequisites and post-deploy next steps (adoption token visibility).
6) Existing remote deploy flows remain functional; `make e2e-web-smoke` stays green.

## Test plan
- Unit tests: credential redaction + host key mismatch behavior.
- Integration: simulate deploy reruns and verify convergent behavior.
- Manual: deploy to a real Pi 5 twice; confirm idempotency.

## Dependencies
- Remote deploy API/UI already exists; this ticket depends on those foundations.

## Risks / open questions
- Whether discovery belongs in the core-server (server-side scan) or in the dashboard client (browser limitations).
- How to support environments with strict SSH hardening (non-default ports, key-only auth).
