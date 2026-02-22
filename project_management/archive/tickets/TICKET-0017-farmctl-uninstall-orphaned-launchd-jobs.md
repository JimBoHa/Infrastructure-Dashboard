# TICKET-0017: farmctl uninstall leaves orphaned launchd jobs/processes (E2E + dev Mac pollution)

**Date filed:** 2026-01-02  
**Area:** `apps/farmctl` (installer substrate)  
**Severity:** High (breaks repeatable E2E installs; leaves background services running)

## Summary

`farmctl uninstall` can report success while leaving LaunchAgent/LaunchDaemon jobs loaded and their processes still running. This pollutes the developer machine (port collisions, stale services), breaks repeatable installer E2E runs, and can mask failures by deleting roots/plists even when `launchctl bootout` fails.

## Evidence

Example leftover jobs after E2E runs:

```
launchctl list 2>/dev/null | grep -i farm
58528  0  com.farmdashboard.e2e.48279b19.setup-daemon
30282  1  com.farmdashboard.e2e.529baa56.core-server
30284  1  com.farmdashboard.e2e.529baa56.telemetry-sidecar
... (many more)
```

## Root Cause (current implementation)

### Bug 1: No process termination fallback

`apps/farmctl/src/uninstall.rs` relies on `launchctl bootout` only; it does not attempt to terminate service processes like `install.rs` does.

### Bug 2: Bootout errors silently ignored

`apps/farmctl/src/uninstall.rs` treats *all* `launchctl bootout` failures as non-fatal, instead of only ignoring the “not loaded” case.

### Bug 3: No verification

There is no post-uninstall verification that:
- launchd labels are removed, and
- ports/processes are gone.

### Bug 4: Wrong order of operations

Uninstall deletes plist files and roots even if bootout fails. Because plists are generated with `RunAtLoad=true` and `KeepAlive=true`, a failed bootout can leave an orphan process running with no plist remaining to cleanly unload later.

## Expected Behavior

`farmctl uninstall` must be safe and repeatable:
- Stop services reliably (no orphan processes).
- Unload/unregister launchd jobs reliably (no leftover labels).
- Only remove plists/roots after services are confirmed stopped/unloaded.
- If cleanup fails, return non-zero so E2E preserves artifacts for debugging.

## Acceptance Criteria

- For E2E profile installs, `farmctl uninstall --profile e2e --remove-roots --yes`:
  - Removes all launchd labels matching the install’s `launchd_label_prefix`.
  - Leaves no running processes for core-server/telemetry-sidecar/postgres/redis/mosquitto/setup-daemon.
  - Ensures the configured ports become free within a bounded timeout.
  - Returns non-zero if any of the above fail (no “false green”).
- For prod profile installs, uninstall behaves equivalently (using `sudo` where required).
- `make e2e-installer-stack-smoke` becomes repeatable on a single dev Mac (no accumulating launchd jobs between runs).

## Notes / Constraints

- macOS-only; no Docker.
- E2E must support multiple fresh installs on one machine (random ports + namespaced labels + clean uninstall).
- Prefer a single canonical installer codepath (CLI and daemon behavior must match).

