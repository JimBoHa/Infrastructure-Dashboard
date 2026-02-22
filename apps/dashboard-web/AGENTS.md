> Dashboard Web Notes
>
> - The System Setup Center is the control surface for setup, credentials, diagnostics, and onboarding.
> - Reuse existing API hooks/components where possible; avoid duplicating logic from Nodes/Provisioning.
> - If new tooling is required, implement it in Rust and expose via `farmctl`.
>
> UI/UX guardrails (anti-patterns: design drift, design debt, feature creep, IA breakdown, weak hierarchy, component inconsistency):
> - Govern UI evolution: every UI change must fit the existing page pattern (`DashboardLayout` in `src/app/(dashboard)/layout.tsx` + `PageHeaderCard` in `src/components/PageHeaderCard.tsx` + card-based sections) and token set (Tailwind spacing/typography/color; no inline styles or raw hex colors in components). If it doesn’t fit, update the shared pattern/component and refactor impacted screens in the same PR.
> - Treat UI debt as work: any knowingly inconsistent “quick win” must have a `DW-*` task with an owner + measurable exit criteria before merge.
> - Add features through templates: place new capabilities into existing page regions only—`PageHeaderCard` `actions`, section cards, drawers, or modals—and reuse existing navigation/controls (no bespoke side panels).
> - Keep IA task-first: every new concept must declare where it lives (top-level tab route vs drawer/modal within an existing tab) and use user-facing terminology; update `SidebarNav` (`src/components/SidebarNav.tsx`) + the Overview “Where things live” map (`src/app/(dashboard)/overview/page.tsx`) when adding/removing top-level areas.
> - Codify hierarchy: each view gets at most one primary action (`NodeButton` in `src/features/nodes/components/NodeButton.tsx`, `variant="primary"` max 1). Use the existing heading + spacing rhythm (`PageHeaderCard` title/description, `p-6` cards, `space-y-*` section spacing) and avoid stacking multiple “high emphasis” treatments in the same viewport.
> - Enforce one component system: build from shared components (`src/components` and `src/features/**/components`). Add new variants/states centrally (e.g., extend `NodeButton`) instead of re-implementing locally.
