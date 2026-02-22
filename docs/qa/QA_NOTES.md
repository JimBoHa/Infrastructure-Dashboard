# QA Notes

This document serves as a place to record Quality Assurance (QA) efforts, including manual testing, exploratory testing, and any observations or risks identified during the development process.

## How to Use

When performing QA for a specific task or feature:

1.  Refer to the relevant task in `project_management/TASKS.md` for acceptance criteria.
2.  Record your testing activities, including:
    *   **Scope:** What was tested.
    *   **Environment:** Details about the testing environment (e.g., OS, browser, device, demo mode vs. real mode).
    *   **Steps:** A clear, concise list of steps taken.
    *   **Expected Results:** What should have happened.
    *   **Actual Results:** What actually happened.
    *   **Observations/Risks:** Any unexpected behavior, potential issues, or areas for improvement.
    *   **Logs/Screenshots:** Include relevant logs or screenshots to support your findings.
    *   **E2E Evidence:** Note the exact E2E command or manual flow used to validate the running app/stack.

## Quick Checklists

- Map tab upgrade smoke: `docs/qa/map-tab-upgrade-smoke.md`.

## Example Entry

### Task: Implement User Authentication (CS-2) - Manual Test - 2025-12-10

*   **Scope:** Manual testing of user registration, login, and logout.
*   **Environment:** `dashboard-web` (local development server), `core-server` (local demo mode).
*   **Steps:**
    1.  Navigate to `/register`.
    2.  Attempt to register with existing email (expected: error).
    3.  Register with new valid credentials.
    4.  Log in with new credentials.
    5.  Log out.
*   **Expected Results:**
    - Error message on duplicate email.
    - Successful registration and login.
    - Successful logout.
*   **Actual Results:** All expected results observed.
*   **Observations/Risks:** None.

---

For automated regression test results, refer to the `reports/` directory. For overall project status and remaining gaps, consult `project_management/BOARD.md` and `project_management/TASKS.md`.
