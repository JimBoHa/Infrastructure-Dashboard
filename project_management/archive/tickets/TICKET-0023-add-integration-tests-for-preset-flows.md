Add Integration Tests for Preset Flows

  Priority: P2 (Quality)
  Status: Done
  Estimated Effort: Large (8-16 hours)

  Problem

  No integration tests exist for the Renogy BT-2 and WS-2902 preset flows. These are complex multi-step operations:

  1. Load preset definitions from shared/presets/integrations.json
  2. Apply configuration to node-agent via HTTP
  3. Upsert sensors into database
  4. Return validation checklist

  Solution

  Implement a production-path integration harness that runs against an installer-provisioned stack (no Docker):

  - Add `tools/e2e_preset_flows_smoke.py` which validates:
    - Renogy BT-2 preset apply: node-agent `/v1/config` GET/PUT + core sensor upsert.
    - WS-2902 integration create + ingest + token rotation + metrics persistence.
  - Wire it into the hard gate: `tools/e2e_installer_stack_smoke.py` runs it after installed health and before web smoke.
  - Keep it runnable standalone once a preserved installer-path stack exists (e.g. after `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`).

  Acceptance Criteria

  - Test harness covers both preset flows and runs without container runtimes.
  - Renogy BT-2 test uses a local node-agent mock and verifies core sensor upsert.
  - WS-2902 test verifies ingest, redaction, status, token rotation, and metrics query.
  - Included in the installer-path E2E gate (`make e2e-installer-stack-smoke`).

  Verification

  - `make ci-web-smoke`
  - `make e2e-installer-stack-smoke` (includes `e2e_preset_flows_smoke`; log: `reports/e2e-installer-stack-smoke/20260104_050252`)

  Test Cases

  Renogy BT-2:
  - Apply preset to node without existing config
  - Apply preset to node with conflicting sensor IDs
  - Apply preset with invalid BT-2 address format
  - Verify sensor creation in database

  WS-2902:
  - Apply preset with new API credentials
  - Apply preset with token rotation
  - Verify redaction of sensitive fields in responses
  - Test weather data ingest endpoint
