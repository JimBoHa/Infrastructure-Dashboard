# TICKET-0028: Dashboard Web UI overhaul to Preline (Admin/Settings)

## Summary

Migrate **all** `apps/dashboard-web` pages to use **Preline** (Tailwind-based) “admin/settings” UI templates so the dashboard UI feels like a modern, comfortable macOS-style web app:

- Consistent layout (sidebar + top bar), spacing, typography, and component styling
- Standardized tables/forms/cards/badges/alerts
- Reduced bespoke CSS/gradients/custom “surface-*” styling
- Keep all existing behaviors and flows intact (no functional regressions)

## Background / Why

The dashboard UI currently mixes custom one-off Tailwind class strings and bespoke styling, which:

- makes it harder to maintain consistency across pages,
- increases UI drift during feature additions,
- makes “production polish” expensive.

Preline provides cohesive admin/settings templates that match the desired UX while keeping us in the existing JS/TS + Tailwind stack.

## Scope

### In scope

- Integrate Preline into `apps/dashboard-web` (Tailwind config + any required JS init)
- Replace the global dashboard layout shell with a Preline layout (sidebar + header)
- Convert **all dashboard pages** to Preline patterns:
  - Nodes / Sensors & Outputs / Users / Backups
  - Schedules
  - Trends / Analytics
  - Deployment / Setup Center / Provisioning / Connection
  - Auth pages (login)
- Replace “custom surface components” with Preline cards/badges/toggles/alerts consistently
- Normalize spacing and typography across all pages
- Preserve accessibility and keyboard navigation

### Not in scope (explicitly)

- Changing core product workflows or backend behavior
- Rewriting charts/calendar logic (wrap in Preline cards/layouts only)

## Acceptance Criteria

- All dashboard routes render using Preline UI primitives/layout patterns (no “legacy” bespoke layout sections remaining).
- Preline JS (if required) is initialized correctly in the Next.js app (no broken dropdowns/toggles/collapses).
- Bespoke global gradients/cards/styles are removed or minimized; `globals.css` is trimmed accordingly.
- UI remains readable and usable in both light and dark mode (if supported).
- Validation:
  - `make ci-web-smoke` passes
  - `make e2e-web-smoke` passes (from a clean state per test hygiene policy)

## Implementation Notes / Links

Reference templates (free):

- https://preline.co/examples/layouts-application.html
- https://preline.co/examples/layouts-application-navbars.html
- https://preline.co/examples/application-tables.html
- https://preline.co/examples/application-form-layouts.html
- https://preline.co/examples/application-stats.html
- https://preline.co/examples/charts.html
- https://preline.co/examples/forms-authentication.html

Preline integration note (Tailwind v4):
- Preline’s Tailwind plugin is currently not used (plugin initialization can error under Tailwind v4).
- We rely on Tailwind v4 `@source` scanning and `window.HSStaticMethods.autoInit()` for Preline JS behaviors.
