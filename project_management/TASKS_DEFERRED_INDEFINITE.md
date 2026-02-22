# Farm Dashboard — Indefinitely Deferred / Deprecated Tasks

This file contains work items that have been explicitly removed from the active backlog because they are **deprecated for production**, **superseded**, or otherwise **not planned**.

**Where things live:**
- Active work items: `project_management/TASKS.md`
- Completed work items: `project_management/TASKS_DONE_2026.md`
- High-level rollup: `project_management/BOARD.md`
- Epic definitions: `project_management/EPICS.md`

**Archiving rules:**
- Keep original ticket IDs stable; do not renumber.
- Move the full ticket text here (no stubs left behind in `TASKS.md`).
- Add an explicit **Archive Reason** and **Moved** date.

---

## Deprecated for Production

- **DT-48: Pi 5 network-boot provisioning workflow (deprecated for production)**
  - **Archive Reason:** Deprecated for production (production Pi deployment is Deploy-over-SSH).
  - **Moved:** 2026-02-05 (from `project_management/TASKS.md`)
  - **Description:** This workflow is no longer part of the production Pi 5 deployment policy (production uses dashboard Deploy over SSH). Keep as a prototype option for future R&D only.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0011-feature-005-deployment-network-boot.md`
    - `docs/runbooks/pi5-network-boot-provisioning.md`
  - **Acceptance Criteria:**
    - Documentation clearly states this is not a supported production path.
    - Any future work is tracked explicitly as R&D and does not block production deployment milestones.
  - **Status:** Deferred indefinitely (deprecated for production)


- **DT-49: Pi 5 “preconfigured media” deployment workflow (deprecated for production)**
  - **Archive Reason:** Deprecated for production (production Pi deployment is Deploy-over-SSH).
  - **Moved:** 2026-02-05 (from `project_management/TASKS.md`)
  - **Description:** This workflow is no longer part of the production Pi 5 deployment policy (production uses dashboard Deploy over SSH). Keep docs only as historical/dev reference to avoid confusion in the field.
  - **References:**
    - `project_management/archive/archive/tickets/TICKET-0009-feature-003-deployment-preconfigured-media.md`
    - `docs/runbooks/pi5-preconfigured-media.md`
  - **Acceptance Criteria:**
    - Documentation clearly states this is not a supported production path.
    - The canonical runbook for production Pi deployment is deploy-over-SSH (`docs/runbooks/pi5-deployment-tool.md`).
  - **Status:** Deferred indefinitely (deprecated for production)


- **DT-47: Re-enable iOS/watch pre-commit gating**
  - **Archive Reason:** Deferred indefinitely (iOS/watch work moved off `main`; no mobile paths are staged/tested on `main`).
  - **Moved:** 2026-02-06 (from `project_management/TASKS.md`)
  - **Description:** Re-enable iOS/watch smoke validation in the staged-path pre-commit selector once iOS/watch default-to-production behavior is fixed and the smoke suite is stable again.
  - **Acceptance Criteria:**
    - `tools/git-hooks/select-tests.py` runs `make ci-ios-smoke` when iOS/watch paths are staged.
    - Docs clearly state when iOS/watch is enforced by pre-commit again.
  - **Status:** Deferred indefinitely (mobile scope paused on `main`; preserved in `freeze/ios-watch-2026q1`)


- **IOS-30: Validate BLE provisioning against real node BLE service**
  - **Archive Reason:** Deferred indefinitely (native mobile work paused on `main`; preserved on freeze branch).
  - **Moved:** 2026-02-06 (from `project_management/TASKS.md`)
  - **Description:** Validate the full CoreBluetooth provisioning flow against a real node BLE service.
  - **Acceptance Criteria:**
    - iOS app discovers the node BLE service and completes the provisioning handshake.
    - Wi‑Fi credentials and adoption token are applied successfully.
    - Adopted node appears in the dashboard with expected metadata.
  - **Status:** Deferred indefinitely (mobile scope paused on `main`; preserved in `freeze/ios-watch-2026q1`)


- **IOS-31: Validate end-to-end parity against real backend/hardware**
  - **Archive Reason:** Deferred indefinitely (native mobile work paused on `main`; preserved on freeze branch).
  - **Moved:** 2026-02-06 (from `project_management/TASKS.md`)
  - **Description:** Validate the iOS app against the real backend/hardware to confirm parity beyond demo mode.
  - **Acceptance Criteria:**
    - Analytics, Backups, and Settings tabs display real data without demo fallbacks.
    - Output commands and alarm actions work against the live backend.
    - BLE provisioning validation (IOS-30) passes in the same environment.
  - **Status:** Deferred indefinitely (mobile scope paused on `main`; preserved in `freeze/ios-watch-2026q1`)
