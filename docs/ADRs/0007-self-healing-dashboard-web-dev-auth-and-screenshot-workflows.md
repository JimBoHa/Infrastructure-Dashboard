# 0007. Self-healing dashboard-web dev auth and screenshot workflows

* **Status:** Accepted
* **Date:** 2026-01-31
* **Implementation:** DW-202..DW-205 (`project_management/tickets/TICKET-0047-dashboard-web-dev-auth-and-screenshots-auto-login-port-discovery-stub-fallback.md`)

## Context
Dashboard debugging and screenshot capture repeatedly burn time on the same “paper cuts”, especially for new agents:

- Dashboard auth is session-bound (token stored in `sessionStorage`), so new browser contexts (Playwright/screenshots) often start unauthenticated.
- Agents fall back to ad-hoc/manual token hacks (copy/paste tokens, ModHeader, etc.), which is slow and error-prone.
- Dev tooling uses inconsistent defaults (e.g., Next dev on `:3000`, scripts assuming `:3005`), so “the app is running but nothing works” is common.
- Starting a second Next dev server can fail (e.g., `.next/dev/lock`), which looks like random breakage unless you already know the trap.

These are not product issues; they are developer workflow issues. The goal is to make the common “debug UI + capture screenshots” loop *self-healing* so it does not depend on remembering runbooks or tribal knowledge.

## Decision
Adopt a self-healing dashboard-web developer workflow that minimizes manual steps and auth troubleshooting:

1) **Dev-only “Login as Dev” affordance in the UI**
   - When `NODE_ENV=development` and the user is unauthenticated, show a small helper (banner/button) to log in with dev credentials.
   - Clicking the action calls `POST /api/auth/login`, stores the token via the existing token helper, and reloads/navigates to the requested page.
   - This must be gated so it cannot ship or be reachable in production builds.

2) **Screenshot tooling auto-auth with clear fallback behavior**
   - Screenshot harness attempts to authenticate via `POST /api/auth/login` using dev/test credentials (from environment variables).
   - If the API is unreachable or auth fails, the harness falls back to the existing stub mode *only when explicitly allowed* (or at minimum logs the mode loudly to avoid masking real auth regressions).

3) **Port unification + auto-discovery**
   - Standardize the dashboard-web dev default to `:3000`.
   - Tooling probes for an already-running server (`:3000`, `:3005`, and an explicit override) and reuses it rather than starting a second instance.

4) **No production auth model changes**
   - This ADR is about dev ergonomics and evidence capture. Production auth, capability gating, and installer flows remain unchanged.

## Consequences
**Benefits**
- Eliminates the recurring “how do I get auth working?” loop for UI debugging and screenshots.
- Screenshot evidence becomes more reliable because tooling can authenticate itself.
- Reduces context burden: fewer runbook lookups and fewer environment-specific gotchas.

**Risks / Tradeoffs**
- Dev-only login affordance is a potential security footgun if it accidentally ships.
  - Mitigation: hard gate on `NODE_ENV`, `window.location.hostname` (`localhost` / `127.0.0.1` only), and an explicit env flag (e.g., `NEXT_PUBLIC_ENABLE_DEV_LOGIN=1`).
  - Add a small test/smoke check to ensure the element is not present in production builds.
- Auto-fallback to stub data can mask real auth/API regressions.
  - Mitigation: make stub fallback opt-in (flag/env), and always print which mode was used in script output.

**Alternatives Considered**
- “Just document it better” (runbooks only): does not scale; agents routinely miss docs under time pressure.
- “Require manual token hacks”: high friction and brittle, and it blocks screenshot automation.
- “Use Playwright storageState only”: helps automation, but does not help humans debugging UI in the browser.
