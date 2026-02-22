# Major Bug Audit (Undocumented in `project_management/*`)

Date: 2026-02-03  
Reviewer: Codex (static scan + manual inspection)

## Definition of “undocumented” (per request)

- A bug is considered documented **only** if it is called out as a bug/work item in `project_management/*`.
- Documentation outside `project_management/*` (code comments, `docs/`, etc.) does **not** count as “documented”.

## Scope (production code only)

Included (reviewed):
- `apps/core-server-rs/src/**`
- `apps/node-agent/app/**` (excluding `generated_api/**`)
- `apps/dashboard-web/src/**` (auth-related API usage only; no deep UI audit)
- `apps/node-forwarder/src/**` (light scan)
- `apps/telemetry-sidecar/src/**` (light scan)
- `apps/wan-portal/src/**` (light scan; NOTE: this scaffold was removed on 2026-02-12 as part of ARCH-6 pruning)

Excluded:
- `apps/ios-app/**`
- any watchOS code/reports
- developer-only routes/components where clearly labeled (e.g., core-server “dev activity” endpoints)

Constraints during audit:
- No code changes were made.
- No builds/tests were run (to avoid generating artifacts inside the repo).

---

## Undocumented major bugs (including security)

### CS-SEC-001: Unauthenticated credential metadata leak via `GET /api/setup/credentials`

Severity: Critical  
Category: Security (secret disclosure)

Where:
- `apps/core-server-rs/src/routes/setup.rs:143` (`list_credentials`)
- `apps/core-server-rs/src/routes/setup.rs:314-315` (Emporia `refresh_token` stored in `setup_credentials.metadata`)

What / impact:
- `GET /api/setup/credentials` is unauthenticated and returns `SetupCredential.metadata` for every row.
- For Emporia, metadata includes a stored `refresh_token` (and username/site ids/devices metadata). Any LAN client that can reach the controller can retrieve these.
- This defeats the point of capability-gated setup endpoints (e.g., `/api/setup/emporia/login` is auth-gated) because secrets can be exfiltrated from the credential inventory endpoint.

Project_management check:
- Searches: `/api/setup/credentials`, `list_credentials`, `SetupCredentialsResponse`
- Result: no matches found under `project_management/*`.

Suggested fix direction:
- Require bearer auth + `config.write` for listing credentials, OR return only `{name, has_value, created_at, updated_at}` with metadata fully redacted.
- If metadata must be returned, explicitly strip/denylist secret keys (e.g., `refresh_token`, `api_token`, etc.).

---

### CS-SEC-002: Unauthenticated dashboard snapshot leaks users + inventory; also enables trivial DoS via expensive work

Severity: Critical  
Category: Security (data disclosure) + Reliability/DoS

Where:
- `apps/core-server-rs/src/routes/dashboard.rs:58` (`dashboard_state`)
- `apps/core-server-rs/src/routes/dashboard.rs:90` (scans backup root on every call)
- `apps/core-server-rs/src/routes/dashboard.rs:241` (triggers mDNS discovery scan inside the request)

What / impact:
- `GET /api/dashboard/state` is unauthenticated.
- It returns a full snapshot including:
  - users (email, role, capabilities)
  - nodes/sensors/outputs/users/schedules/alarms snapshot
  - backup inventory entries including filesystem `path`
  - adoption candidates
- Because it also performs backup directory scanning and a 1s mDNS scan, any unauthenticated caller can repeatedly hit this endpoint to force repeated disk + network discovery work (DoS amplification).

Project_management check:
- `/api/dashboard/state` is heavily referenced for functionality, but no explicit mention of missing auth was found:
  - Search: `dashboard/state` + (`unauth`|`auth`|`bearer`|`token`|`security`)
  - Result: no matches indicating a known missing-auth bug.

Suggested fix direction:
- Require bearer auth for `/api/dashboard/state`.
- Consider splitting the endpoint:
  - “cheap snapshot” (DB-only) vs “expensive discovery/backup scan” (explicit action/cached result)
- Do not include `users` (or user emails) unless the caller has `users.manage`.

---

### CS-SEC-003: Core read endpoints are largely unauthenticated, leaking sensitive operational data

Severity: High  
Category: Security (unauthorized read access)

Where (examples; all are GET and lack `AuthUser`/`OptionalAuthUser`):
- Nodes:
  - `apps/core-server-rs/src/routes/nodes.rs:170` (`GET /api/nodes`)
  - `apps/core-server-rs/src/routes/nodes.rs:413` (`GET /api/nodes/{node_id}`)
- Outputs:
  - `apps/core-server-rs/src/routes/outputs.rs:139` (`GET /api/outputs`)
  - `apps/core-server-rs/src/routes/outputs.rs:160` (`GET /api/outputs/{output_id}`)
- Schedules:
  - `apps/core-server-rs/src/routes/schedules.rs:63` (`GET /api/schedules`)
  - `apps/core-server-rs/src/routes/schedules.rs:320` (`GET /api/schedules/calendar`)
- Alarms:
  - `apps/core-server-rs/src/routes/alarms.rs:158` (`GET /api/alarms`)
  - `apps/core-server-rs/src/routes/alarms.rs:171` (`GET /api/alarms/history`)
- Analytics:
  - `apps/core-server-rs/src/routes/analytics.rs:169` (`GET /api/analytics/feeds/status`)
  - `apps/core-server-rs/src/routes/analytics.rs:839` (`GET /api/analytics/power`)
  - `apps/core-server-rs/src/routes/analytics.rs:1519` (`GET /api/analytics/status`)

What / impact:
- These endpoints expose:
  - device identifiers (MACs), last-known IPs, and node config blobs
  - output metadata (including MQTT command topics in config)
  - schedules and alarm history
  - analytics summaries and feed status/history
- This undermines the “login + capabilities” model by allowing unauthenticated reads of operational data.

Project_management check:
- `project_management/TASKS.md` tracks unauthenticated access for **metrics** + **backups** (and node-agent), but not the broader core read surface:
  - Search: “without authentication” (only found backups + node-agent)
  - Search: “Require auth for /api/nodes|/api/outputs|/api/schedules|/api/alarms|/api/analytics” (no matches)

Suggested fix direction:
- Decide an explicit policy:
  - Either “everything except `/healthz` requires auth” (recommended), OR
  - Public read endpoints must be explicitly scoped + redacted.
- Add `AuthUser`/`OptionalAuthUser` + capability checks for these routes.

---

### CS-SEC-004: Fresh-install takeover risk — `POST /api/users` can create a user without auth when DB is empty

Severity: Critical  
Category: Security (privilege takeover)

Where:
- `apps/core-server-rs/src/routes/users.rs:171` (`create_user`)
- `apps/core-server-rs/src/routes/users.rs:181` (allows unauth create when `users_exist == false`)

What / impact:
- When no users exist, `create_user` permits unauthenticated user creation.
- If the controller API is reachable on a LAN during initial setup, an attacker can race to create the first admin user and permanently control the system.

Project_management check:
- There is an intent to avoid this path in production (“initial admin automatically created”), but no bug ticket documents the current unauthenticated fallback:
  - `project_management/TASKS_DONE_2026.md:343` mentions securing `/api/users` behind `users.manage`
  - `project_management/TASKS_DONE_2026.md:5843` mentions auto-creating an initial admin on fresh installs
  - No task found documenting “unauth create_user allowed when users table is empty.”

Suggested fix direction:
- In production builds, remove/disable unauthenticated user creation.
- If a bootstrap path is required, gate it behind an installer/setup secret, localhost-only restriction, or a time-limited one-time token.

---

### CS-FUNC-001: Predictive endpoints are stubbed/non-functional

Severity: High  
Category: Functional correctness (feature claims vs reality)

Where:
- `apps/core-server-rs/src/routes/predictive.rs:68-70` (`GET /api/predictive/trace` returns `[]`)
- `apps/core-server-rs/src/routes/predictive.rs:172-197` (`POST /api/predictive/bootstrap` returns zeros and does no work)

What / impact:
- The endpoints exist and are wired into the API surface, but provide no operational capability.
- This contradicts expectations that bootstrap can generate predictive alarms (best effort) when predictive is enabled.

Project_management check:
- `project_management/TASKS_DONE_2026.md` asserts bootstrap can generate alarms, but no bug ticket documents that the current Rust endpoints are still stubbed.

Suggested fix direction:
- Either implement bootstrap/trace end-to-end, OR return `501`/explicit errors and remove the feature from “Done” status until implemented.

Resolution (2026-02-03):
- Implemented `predictive_trace` storage + real `/api/predictive/trace` responses (no empty stub).
- Implemented a best-effort `/api/predictive/bootstrap` flow that evaluates recent DB metrics and emits predictive alarm events (no zero-count false success).
- Validated via `make ci-core-smoke` (pass).

---

### NA-FUNC-001: Node-agent apply_config bypasses validation for key timing fields

Severity: High  
Category: Reliability + potential DoS via misconfig

Where:
- `apps/node-agent/app/services/config_store.py:75-101` (`apply_config`)
- `apps/node-agent/app/services/config_store.py:87-95` (direct `setattr` for `heartbeat_interval_seconds` / `telemetry_interval_seconds`)

What / impact:
- `apply_config` mutates `Settings` in-place and sets heartbeat/telemetry intervals directly from persisted JSON without validation/clamping.
- If either value is 0 or negative, scheduling loops can degrade into near-tight polling/publishing and/or incorrect cadence.

Project_management check:
- Searches: `heartbeat_interval_seconds`, `telemetry_interval_seconds`, `apply_config`, `ConfigStore apply`
- Result: no bug entry found under `project_management/*` (only general mentions of ConfigStore).

Suggested fix direction:
- Re-validate payload through Pydantic (`Settings.model_validate` or explicit field validators) and clamp intervals to sane minimums.
- Treat invalid persisted configs as “reject + keep last-known-good” instead of mutating in-place.

Resolution (2026-02-03):
- `apply_config` is now transactional (validate candidate config before mutating live settings) and `heartbeat_interval_seconds` / `telemetry_interval_seconds` are validated/clamped to sane bounds.
- Node-agent config/provisioning endpoints now require bearer auth, so the “control plane” is not exposed to unauthenticated LAN callers.
- Provisioning Wi‑Fi apply is now non-blocking (runs in a background thread) so slow `nmcli`/`wpa_cli` calls cannot stall the asyncio loop / watchdog pings.
- Validated via `make ci-node` (pass) and core-server integration via `make ci-core-smoke` (pass).

---

## Tracking + status (post-audit)

The findings above are now tracked in `project_management/TASKS.md` (project-management is the source of truth).

Implemented (Tier A validated installed `0.1.9.244-major-bug-fixes`; Tier B DT-59):
- CS-90 (metrics query/ingest auth + capabilities)
- CS-91 (backups read auth + capabilities)
- CS-92 (backups/restore run + restore worker; remove stub endpoints)
- CS-93 (DST-safe schedule block evaluation; no silent skips)
- CS-94 (setup credentials inventory auth + metadata redaction)
- CS-95 (dashboard snapshot auth + remove scan hot-path)
- CS-96 (require auth for core read surfaces)
- CS-97 (remove unauthenticated “first user wins” bootstrap user creation)
- CS-98 (predictive trace + bootstrap; remove stubbed “success” responses)
- NA-66 (node-agent config/provisioning endpoints bearer-auth + secret redaction)
- NA-67 (provisioning Wi‑Fi apply is non-blocking; queued/in-progress state)
- NA-68 (transactional apply_config + interval clamping; no partial apply)

Tier A evidence:
- `project_management/runs/RUN-20260203-tier-a-major-bug-fixes-0.1.9.244-major-bug-fixes.md`

Still open:
- None (as of 2026-02-03; Tier A hardware validation is still pending for unrelated Node Agent tasks)
