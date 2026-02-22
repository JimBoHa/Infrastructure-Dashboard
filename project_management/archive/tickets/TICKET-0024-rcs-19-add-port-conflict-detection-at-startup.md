RCS-19: Add Port Conflict Detection at Startup

  Priority: P2 (Reliability)
  Status: Done
  Estimated Effort: Small (2-4 hours)

  Problem

  If a port is already in use (e.g., from an orphaned process), core-server-rs fails with a cryptic bind error. There's no pre-flight check or helpful error message.

  Solution

  Add a listener-bind wrapper that special-cases `AddrInUse` and returns an actionable error message (including guidance to re-run with `--port`).

  Acceptance Criteria

  - Startup fails fast with a helpful message when the listen port is already in use.
  - Covered by a unit test that binds an ephemeral port and asserts the error message is actionable.

  Verification

  - `cd apps/core-server-rs && cargo test -q`
  - `python3 tools/check_openapi_coverage.py`
