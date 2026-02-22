<proposed_plan>
# Alarms: “Operator‑Grade” Incident + Rule Builder (Full‑Featured, Polished)

## Summary (what “done” means)
Rebuild the existing `/alarms` page into a complete, operator-friendly, production-quality Alarms feature that:
- Provides a first-class **Incident workflow** (grouping, assign/snooze/close, notes, search/filter, and triage).
- Provides a **guided alarm rule builder** with strong visual/statistical guidance (mean/median, ±1/2/3σ bands, robust stats, histogram), plus **Backtest** to tune rules safely.
- Provides **deep investigation** for triggered alarms: open an incident/event and see **co-occurring signals** across *all sensors by default* (not “visible-only” truncation), ranked by **significance + proximity in time**, plus contextual non-sensor events (schedule/action logs + node context).
- Matches existing dashboard **theme + UI conventions** (reuse current components/patterns).
- Does **not** modify the `/alarms2` page implementation (route remains untouched). Nav label may be adjusted to “Experimental” per your choice.

---

## Non-goals (this iteration)
- Outbound notification routing (email/SMS/webhooks/escalations) — explicitly deferred.
- Replacing or editing `/alarms2` page code.
- Mobile app UX.

---

## Key product decisions (locked from your answers)
- **Backend scope:** full stack (new APIs + new DB tables/migrations).
- **Default baseline for stats guidance:** last **7 days**.
- **Triggered-alarm related-sensor search:** **controller-wide**, include **all sensors by default**, with UI filters (no “visible-only” candidate truncation).
- **Incident workflow:** full incident management (assign/snooze/close + notes) + richer investigation.
- **Builder guidance:** time-series + bands + histogram **plus** a stats table.
- **Permissions:** incident management restricted to `config.write`.
- **Backtest:** required as a first-class builder step.
- **Incident grouping:** by `(rule_id + target_key)` with a time-gap rule.
- **Context window:** auto by sensor interval (bounded/capped).
- **Include non-sensor events:** yes (action logs + node context), with easy search/filter.
- **Stats method:** show both classic (mean/std) and robust (median/MAD/percentiles).
- **Include derived + public-provider sensors:** included by default, user-filterable.
- **Alarms v2 nav:** keep link but label **Experimental**.

---

## UX / IA (Information Architecture)

### `/alarms` top-level layout
A consistent page shell using existing dashboard components:
- `PageHeaderCard` with:
  - Title: “Alarms”
  - Subtext: “Incidents, rules, and investigation.”
  - Primary action: `Create rule` (requires `config.write`)
- Two main tabs (or segmented controls):
  1. **Incidents** (default)
  2. **Rules**

### Incidents tab (operator workflow)
**Left panel (list):**
- Search box (matches: incident title, rule name, node name, sensor name, message text).
- Filters:
  - Status: `Open`, `Snoozed`, `Closed`
  - Severity: `Critical/Warning/Info`
  - Assigned: `Me/Unassigned/Anyone`
  - Time range (e.g., 24h/7d/custom)
  - Origin (threshold/predictive/schedule/etc)
- Sort:
  - Default: “Highest severity, newest activity”
  - Optional: “Most frequent”, “Longest open”, “Most sensors impacted”

**Right panel (detail drawer or split view):**
- Incident header:
  - Status pill + severity
  - Assignment control (user picker)
  - Snooze control (preset durations + custom until)
  - Close/Reopen
  - “Ack all events in incident” (calls existing bulk-ack on the incident’s active events)
- Sections (collapsible cards):
  1. **Timeline / Context Chart**
     - Focus sensor series around trigger time
     - Overlay: threshold/band lines if applicable
     - Quick range presets (auto, ±1h, ±6h, ±24h) with auto default
  2. **Related Signals (co-occurrence across all sensors)**
     - Runs controller-wide related-sensors analysis (details below)
     - Default sort: **Combined** (significance + proximity)
     - Filters: same node, same unit, same type, include/exclude derived, include/exclude public-provider, exclude specific sensors
     - Clicking a candidate opens an overlay preview (focus vs candidate alignment + event overlays)
  3. **Other Events**
     - Schedule/action executions from `action_logs` within the context window
     - Node context (best-effort from existing node fields: status, last_seen; plus any new node event table if added later)
     - Search + filters (event type, node)
  4. **Event History**
     - The raw alarm events tied to the incident (with per-event details)
  5. **Notes**
     - Append-only notes with author + timestamp
     - Fast-add box + markdown/plain text (plain text is fine; markdown optional)

### Rules tab (builder + library)
- Rule library list with:
  - Enabled toggle
  - Severity
  - Scope summary (“N sensors”, node, filter summary)
  - Last eval time / errors (existing “Rule health” concepts retained but polished)
  - Quick actions: Edit / Duplicate / Delete
- “Create rule” opens a **guided builder** (sheet/drawer) with steps:
  1. **Basics**
  2. **Target & Condition**
  3. **Guidance (Stats + Visuals)**
  4. **Backtest**
  5. **Review & Save**
- Keep an **Advanced JSON** mode, but make it a clearly labeled expert-only section.

---

## Backend work (core-server-rs + infra migrations)

### DB migrations
Add `infra/migrations/043_incidents_v1.sql` (new) with:

1) `incidents` table (grouped work unit)
- `id bigserial primary key`
- `rule_id bigint references alarm_rules(id) on delete set null`
- `target_key text` (nullable until fully wired)
- `severity text not null`
- `status text not null` enum-like: `open | snoozed | closed`
- `title text not null` (default from rule name + target)
- `assigned_to uuid references users(id) on delete set null`
- `snoozed_until timestamptz`
- `first_event_at timestamptz not null`
- `last_event_at timestamptz not null`
- `closed_at timestamptz`
- `created_at/updated_at timestamptz not null default now()`

2) Extend `alarm_events`
- Add `incident_id bigint references incidents(id) on delete set null`
- Add `target_key text` (copy from `alarms.target_key` at insert time)
- Add indexes to support filtering: `(incident_id)`, `(rule_id, target_key, created_at desc)`, `(created_at desc)`

3) `incident_notes` table
- `id bigserial primary key`
- `incident_id bigint references incidents(id) on delete cascade`
- `created_by uuid references users(id) on delete set null`
- `body text not null`
- `created_at timestamptz not null default now()`
- Index `(incident_id, created_at desc)`

**Incident grouping logic (server-side)**
When inserting a new alarm event (alarm engine, schedule engine, predictive, etc.):
- Determine `(rule_id, target_key)` (prefer event.rule_id + alarms.target_key via alarm_id join; fall back gracefully).
- Find an existing incident where:
  - `status != closed`
  - same `(rule_id, target_key)`
  - `last_event_at >= now() - INCIDENT_GAP_SECONDS` (default **30 minutes**)
- If found: update `last_event_at`, ensure status is `open` unless currently snoozed and `snoozed_until > now()`.
- Else: create new incident with `first_event_at=created_at`, `last_event_at=created_at`, `status=open`.
- Set `alarm_events.incident_id` on insert.

### New/updated API endpoints (Rust)
Add `apps/core-server-rs/src/routes/incidents.rs` and merge into router + OpenAPI.

**Incidents**
- `GET /api/incidents`
  - Query params: `status`, `severity`, `assigned_to`, `unassigned=true`, `from`, `to`, `search`, `limit`, `cursor`
  - Auth: require `config.write` OR `alerts.view`? (Decision: **read allowed for `alerts.view` and `config.write`**, but **mutations require `config.write`**.)
- `GET /api/incidents/{id}`
  - Returns incident detail + recent linked alarm_events + notes summary counts
- `POST /api/incidents/{id}/assign` body: `{ user_id: uuid | null }` (null = unassign) — requires `config.write`
- `POST /api/incidents/{id}/snooze` body: `{ until: timestamptz | null }` (null = unsnooze) — requires `config.write`
- `POST /api/incidents/{id}/close` body: `{ closed: boolean }` (false = reopen) — requires `config.write`

**Notes**
- `GET /api/incidents/{id}/notes` (paged)
- `POST /api/incidents/{id}/notes` body: `{ body: string }` — requires `config.write`

**Other events**
- `GET /api/action-logs`
  - Query: `from`, `to`, optional `node_id`, optional `schedule_id`, `limit`
  - Auth: `alerts.view` or `config.write`

### Alarm rule “stats” guidance endpoint (new)
Add `POST /api/alarm-rules/stats` (requires `config.write`):
- Request:
  - `target_selector` (same schema as rule selector)
  - `start`, `end`
  - `interval_seconds` (optional; server can choose sensible default)
  - `bucket_aggregation_mode` (optional; default `auto`)
- Response per sensor_id:
  - `n`, `min`, `max`, `mean`, `stddev`, `median`
  - percentiles: `p01,p05,p25,p75,p95,p99`
  - robust: `mad`, `iqr`
  - suggested bands:
    - classic: `mean ± k*stddev` for k=1,2,3
    - robust-sigma approximation: `median ± k*(1.4826*mad)` for k=1,2,3
  - metadata for UI: unit, coverage %, missingness

Implementation detail:
- Use existing analysis lake/bucket reader (`services/analysis/bucket_reader.rs`) to fetch series efficiently (also covers derived sensors).
- Stats computed server-side to avoid shipping huge series to the browser just for guidance.

### Alarm rule backtest (analysis job)
Implement a new analysis job type in `apps/core-server-rs/src/services/analysis/jobs/`:
- `alarm_rule_backtest_v1`
- Params:
  - rule definition: `target_selector`, `condition_ast`, `timing`
  - window: `start`, `end`
  - `interval_seconds`
  - `bucket_aggregation_mode`
- Output:
  - summary totals + per-target breakdown:
    - count of firings, total firing duration, median firing duration
    - timestamps of transitions (fire/clear) for drill-down
  - any skipped targets + reason (missing data, unsupported sensor source, etc.)

Evaluator details (decision complete):
- Reuse/refactor alarm engine evaluation so backtest uses the same condition semantics:
  - `debounce_seconds`: require the condition be true continuously for debounce duration before “fire”.
  - `clear_hysteresis_seconds`: require condition be false continuously for hysteresis duration before “clear”.
  - `eval_interval_seconds`: determines stepping; if not set, use the backtest `interval_seconds`.
- Backtest operates over bucketed values; missing buckets are treated as “no data” and should:
  - trigger Offline conditions appropriately
  - otherwise not silently assume 0 (explicit missingness handling)

---

## Related-signals analysis for triggered alarms (co-occurrence)

### Use existing “Related Sensors Unified v2” analysis jobs
Frontend will run `POST /api/analysis/jobs` with:
- `job_type = "related_sensors_unified_v2"`
- `params`:
  - `focus_sensor_id` = incident’s focus sensor (from alarm/rule/event)
  - `start/end` = context window around trigger time (auto default, user adjustable)
  - `candidate_source` = `"all_sensors_in_scope"` (controller-wide, not visible-only)
  - `evaluate_all_eligible = true`
  - `filters` default: all `false/null` (include all), but UI toggles map to:
    - `same_node_only`, `same_unit_only`, `same_type_only`
    - `is_derived`, `is_public_provider`
    - `exclude_sensor_ids`
  - `mode` default: `"advanced"` (quality over speed)
  - `deseason_mode` default: `"hour_of_day_mean"` for better quality on diurnal sensors (toggleable)
  - `periodic_penalty_enabled = true` (keep defaults aligned with existing trends relationship finder)

### Ranking requirement: “significance + proximity”
Define and implement deterministic sorting (and expose alternatives):
- Compute `proximity_sec` for each candidate:
  - Take `candidate.top_bucket_timestamps` (ms) when present; compute min `abs(ts - alarm_ts)`; else null.
- Default sort: **Combined**
  1. confidence tier: high > medium > low
  2. blended_score desc
  3. proximity_sec asc (nulls last)
- Provide user sort toggles: `Significance`, `Proximity`, `Combined`.

### Candidate drill-down preview
On selecting a candidate, call `POST /api/analysis/preview` to fetch:
- focus series
- candidate series
- aligned candidate (if available)
- event overlays
Render using existing charting components (same look as Trends).

---

## Context window auto-sizing (decision complete)
Given an alarm event at time `T` and focus sensor interval `s.interval_seconds`:

1) Choose chart/analysis bucket interval `bucket_sec`:
- Start with `raw = max(30, min(1800, s.interval_seconds || 60))`
- Snap to nearest preset in `[30, 60, 300, 900, 1800]` (prefer larger if tie) to keep point counts reasonable.

2) Choose half-window size `half_window_sec`:
- `half_window_sec = clamp(bucket_sec * 180, min=3600, max=43200)`  
  (i.e., 180 buckets each side; min ±1h, max ±12h)

3) Default window:
- `start = T - half_window_sec`
- `end   = T + half_window_sec`

UI allows widening to ±24h and narrowing to ±1h.

---

## Frontend implementation (dashboard-web)

### Don’t touch `/alarms2`
- No edits to `apps/dashboard-web/src/app/(dashboard)/alarms2/**`.
- Update sidebar label only:
  - `SidebarNav.tsx`: change “Alarms v2” label to “Alarms (Experimental)” (href stays `/alarms2`).

### `/alarms` page rebuild
Modify `apps/dashboard-web/src/app/(dashboard)/alarms/AlarmsPageClient.tsx` to:
- Use new tabs: Incidents / Rules
- Route-level data hooks:
  - `useIncidentsQuery`, `useIncidentDetailQuery`, `useIncidentNotesQuery`
  - Existing `useAlarmRulesQuery`, `useAlarmsQuery`, `useAlarmEventsQuery` still used (events may now be presented via incidents)

Add new feature modules:
- `apps/dashboard-web/src/features/incidents/**` (new)
  - components: `IncidentListPanel`, `IncidentDetailPanel`, `IncidentNotesPanel`, `OtherEventsPanel`
  - hooks: queries + mutations
  - types: incident DTOs
- Extend `apps/dashboard-web/src/features/alarms/**`
  - Builder steps:
    - `RuleBasicsStep`
    - `RuleTargetConditionStep` (enhanced targeting + match modes)
    - `RuleGuidanceStep` (stats table + histogram + band overlays)
    - `RuleBacktestStep` (analysis job UI)
    - `RuleReviewStep`

### Rule builder upgrades (operator-friendly)
Target selection improvements (must-have):
- Search input + filters consistent with Trends:
  - Search matches: sensor name, sensor_id, type, unit, node name
- Support all selector modes:
  - single sensor
  - node sensors + optional type filter
  - filter (provider/metric/type)
  - sensor set (multi-select)
- Expose match mode when selector can target many sensors:
  - `per_sensor` (default)
  - `any`
  - `all`

Guidance step:
- Fetch stats via new `/api/alarm-rules/stats`.
- Show:
  - Stats table (mean/median/stddev/MAD/percentiles, min/max, n, missingness)
  - Histogram (bucketed distribution)
  - Suggested threshold chips:
    - mean ± 2σ, mean ± 3σ
    - p05/p95
    - median ± 2*(1.4826*MAD)
- Apply-to-rule actions:
  - “Set threshold to …”
  - “Set band to …”

Backtest step:
- Start job (`/api/analysis/jobs`, `alarm_rule_backtest_v1`)
- Poll job status/result
- Present:
  - total fires, fires/day, time-in-alarm
  - per-sensor top offenders
  - clickable transition timeline → opens context preview at that timestamp

Incident detail “Related Signals”:
- Start job `related_sensors_unified_v2` for the incident’s focus sensor and window
- Allow user to:
  - change filters
  - change window
  - rerun analysis (explicit button; do not auto-rerun on every keystroke)

Other events:
- Fetch action logs in-window with `GET /api/action-logs`
- UI: list + search + filter by node/schedule; include timestamps and statuses.

---

## Capabilities / auth
- Viewing incidents:
  - Allow `alerts.view` and `config.write`
- Mutating incidents (assign/snooze/close/notes):
  - Require `config.write` (per your choice)
- Related-sensors analysis job execution:
  - Requires existing `analysis.run` capability today; ensure `config.write` users have it (already appears to be defaulted for admins), or explicitly allow `config.write` to call analysis endpoints (preferred: allow `config.write` through `require_any_capabilities` on analysis routes).

---

## Testing & validation (must run)
### Backend (core-server-rs)
- Add unit tests for:
  - Incident grouping (gap-based merge vs new incident)
  - Backtest evaluator correctness on synthetic bucket series (debounce/hysteresis)
  - Stats calculations (mean/stddev/median/MAD/percentiles) on known datasets
- Run: `make ci-core-smoke`

### Frontend (dashboard-web)
- Add/extend smoke tests (existing `npm run test:smoke`) to cover:
  - `/alarms` renders with Incidents + Rules tabs
  - Create rule flow reaches Guidance + Backtest steps (mock API)
  - Incident detail opens and can add a note (mock)
- Run: `make ci-web-smoke`

### E2E/Tier A/Tier B (follow existing project gates)
- Plan includes Tier A validation for dashboard UI changes using the runbook and screenshot gate when executing (implementation phase).

---

## Project management sync (required when implementing)
When execution starts, create/update work items in:
- `project_management/TASKS.md` (granular tasks for: incidents backend, alarms page rebuild, stats endpoint, backtest job, related signals UI, action-logs endpoint, tests, Tier A run)
- Keep `project_management/BOARD.md` and `project_management/EPICS.md` aligned with actual status once implementation work begins.

---

## “Suggestions to add/modify” (baked into scope, plus optional extras)
Included (core):
- Incident workflow (assign/snooze/close/notes)
- Rule guidance with robust + classic stats
- Backtest to prevent noisy alarms
- Controller-wide related-signal analysis with strong filtering + drill-down previews
- Contextual non-sensor events (action logs)

Optional (explicitly NOT in this iteration, but recommended next):
- Notification routing + escalation policies
- Saved “alarm tuning sessions” / presets for different sensor families
- Incident “playbook” links per rule (operator instructions)
- Automatic suppression during maintenance windows (schedule-gated alarms) once condition language expands

</proposed_plan>

