# RUN-20260112 — Tier A Auth/Roles/Permissions (installed controller `0.1.9.101`)

**Goal:** Validate auth/roles/capability gating on the already-installed controller (Tier A). **No DB/settings reset.**

## Upgrade / refresh (installed controller)

- Built controller bundle DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.101.dmg`
- Pointed setup-daemon at the new DMG:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.101.dmg"}'`
- Upgraded:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
  - `curl -fsS http://127.0.0.1:8800/api/status` → `current_version: 0.1.9.101`

## Tier A checks (auth/roles/permissions)

- Login bootstrap no longer depends on `/api/users`:
  - `GET /api/auth/bootstrap` returns `{"has_users": true}` on a seeded controller.
- Capability-gated UX:
  - Admin navigation is hidden for non-admin capability sets (`config.write`, `users.manage`).
  - Outputs command actions show read-only UX when `outputs.command` is missing.
- Regression tests:
  - `cd apps/dashboard-web && npm run test:playwright -- auth-gating.spec.ts` (pass)

## Evidence (screenshots)

- Captured via `apps/dashboard-web/scripts/web-screenshots.mjs` against the installed controller:
  - Folder: `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/`
  - **Viewed in this session:** `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/users.png`
  - **Viewed in this session:** `manual_screenshots_web/tier_a_auth_0.1.9.101_20260112_1725/users_add_modal.png`
