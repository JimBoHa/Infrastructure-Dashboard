Verify and Harden Uninstall Process Cleanup

  Priority: P2 (downgraded from P0 - core issue fixed)
  Status: Ready for verification
  Estimated Effort: Small (2-4 hours)

  Background

  The farmctl uninstall process was previously leaving orphaned launchd jobs and processes. Recent commits added:

  1. terminate_service_processes() call before bootout (uninstall.rs:42)
  2. Multi-strategy bootout with fallbacks (bootout_best_effort())
  3. Verification via launchctl_list_contains() (uninstall.rs:63-91)
  4. tools/test_hygiene.py for detection and cleanup

  Current state shows 0 orphaned launchd jobs, suggesting the fix is working.

  Acceptance Criteria

  - Run make e2e-installer-stack-smoke 5 times consecutively
  - After each run, verify python3 tools/test_hygiene.py --check reports clean state
  - Run farmctl uninstall --remove-roots --yes manually and verify no orphans
  - Add port-free verification after process termination (optional hardening)

  Files to Review

  - apps/farmctl/src/uninstall.rs (329 lines)
  - apps/farmctl/src/processes.rs (76 lines)
  - tools/test_hygiene.py (existing hygiene helper)

  Optional Hardening

  // After terminate_service_processes(), verify ports are free:
  fn verify_ports_free(ports: &[u16]) -> Result<()> {
      for port in ports {
          if TcpListener::bind(("127.0.0.1", *port)).is_err() {
              bail!("Port {} still in use after process termination", port);
          }
      }
      Ok(())
  }

  ---
  Summary of Tickets

  | Ticket                             | Priority | Effort | Status          |
  |------------------------------------|----------|--------|-----------------|
  | SETUP-21: Verify uninstall cleanup | P2       | Small  | Ready to verify |
  | RCS-15: SQL error leakage          | P1       | Medium | To Do           |
  | RCS-16: OpenTelemetry tracing      | P2       | Medium | To Do           |
  | RCS-17: Refactor outputs.rs        | P2       | Medium | To Do           |
  | RCS-18: Integration tests          | P2       | Large  | To Do           |
  | RCS-19: Port conflict detection    | P2       | Small  | To Do           |

  Tickets NOT Needed

  - P1: Unify process cleanup - Already done. Both install.rs and uninstall.rs use processes::terminate_processes().

