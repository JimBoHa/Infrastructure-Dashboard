# Run Logs

This folder stores long-form run logs and evidence artifacts (Tier A installs/upgrades, Tier B clean-host E2E runs, and major production validations).

## Convention

- Prefer one run log per validation run (not per ticket).
- Use filenames like `RUN-YYYYMMDD-<short-topic>.md`.
- Tickets in `project_management/TASKS.md` should include short **Evidence** lines that link to these run logs instead of embedding long logs inline.

## Template

- **Context:** <what changed / why>
- **Host:** <clean host vs installed controller>
- **Commands:** <exact commands run>
- **Result:** Pass/Fail + brief notes
- **Artifacts:** <links to logs/screenshots/reports>

## Tier-A Screenshot Hard Gate

For Tier-A runs that include dashboard UI changes, the run log must include a
`## Tier A Screenshot Review (Hard Gate)` section with checked `REVIEWED` entries,
visual check bullets, findings, and reviewer declaration. Validate with:

`make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-....md`
