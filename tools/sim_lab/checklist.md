# Sim Lab Run & Verification Checklist

Quick reminders to run **every time** before saying Sim Lab is “done”.

- Read `AGENTS.md` and this checklist before you start work.
- Run the exact workflow you are reporting on (e.g. `make demo-live`) and confirm it completes successfully.
- If a command fails, **stop and decide** the path: patch the issue (e.g. idempotent migration) or reset state (stop local mocks / native services) before retrying. Do not proceed without resolving.
- After success, confirm:
  - Core server, dashboard-web, and all simulated nodes are running.
  - Seeded nodes show live telemetry and online status.
  - Mesh diagnostics show simulated nodes (health=simulated, non-zero mesh nodes).
  - BLE provisioning status reports ready (simulated) for node dashboards.
  - Adoption works end-to-end (token issuance + adopt + node restarts with adopted UUID).
  - Output commands reach simulated nodes and acks/state topics update.
  - Forecast + utility rate feeds return data (no external API keys required).
- Document whether the workflow was actually run; if not, say “not run yet” and why.
