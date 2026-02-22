# Farm Dashboard Feature Tickets — Review Notes (v2)

Date: 2025-12-31

This file summarizes why each FEATURE-* ticket was changed and what repo facts drove the changes.

## Global changes applied to every ticket
- Added: Business goal, raw inputs, “current state in repo”, explicit scope boundaries, repo ownership boundaries, acceptance criteria, and test plan.
- Explicitly aligned with the project rule that `project_management/TASKS.md` is the single source of truth and that requirement tickets live under `project_management/tickets/` (per BOARD/TASKS docs).
- Avoided rebuilding foundations that already exist (remote Pi 5 deploy API/UI, imaging scripts, Renogy collector/tooling).

## Feature-by-feature notes
### FEATURE-001 (Pi 5 Local Display basic)
- New work (not present in TASKS/EPICS today).
- Reuse-first plan: node-agent serves a kiosk route; Pi runs Chromium kiosk or FullPageOS.

### FEATURE-002 (Pi 5 Local Display advanced)
- New work; depends on FEATURE-001.
- Added safety gating for output control and clarified trend data sources.

### FEATURE-003 (Preconfigured media)
- Re-scoped from “invent a flasher” to “productize existing overlay + first-boot + Pi Imager profile tooling”.
- Explicitly calls out optional offline pre-seeding vs default online adoption flow.

### FEATURE-004 (Deploy from server via SSH)
- Re-scoped as hardening/UX because core-server (CS-42) and dashboard-web (DW-44) already implement the baseline.
- Added missing product-grade requirements: host key verification, credential redaction, discovery UX, idempotency.

### FEATURE-005 (Network boot)
- New work; references Raspberry Pi bootloader-supported network install/HTTP boot rather than custom PXE.

### FEATURE-006 (Renogy one-click)
- Re-scoped to focus on dashboard UX because Renogy collector + deployment tooling already exists.
- Added parity + idempotency requirements so the “default sensor set” does not drift between CLI and web UI.

### FEATURE-007 (WS-2902)
- Corrected integration approach to prefer station “push” uploads via standard weather protocols instead of a bespoke TCP polling implementation.
- Added token auth + status/troubleshooting requirements.

### FEATURE-008 (WAN read-only AWS portal)
- New work; added explicit read-only guarantees, caching vs on-demand modes, and reuse of existing auth reverse proxies.
- Added security guidance for minimizing on-prem exposure.

