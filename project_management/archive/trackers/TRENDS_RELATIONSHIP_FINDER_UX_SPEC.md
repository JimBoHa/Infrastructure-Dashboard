# Trends: Relationship Finder (Unified Analysis UX) — Implementation Spec

Created: 2026-01-26
Owner: (unassigned)
Status: **Implemented** (2026-01-27)

## Product intent

Replace multiple separate analysis containers on Trends with a single cohesive UX so users can:
- pick a focus sensor once,
- run different relationship discovery strategies,
- see a single unified “related sensors” list,
- drill into a single preview area for “why”, and
- add candidates to the chart consistently.

This is a UX merge/refactor. It should reuse existing backend job types where possible.

## User decisions (locked)

1) **Correlation must remain a matrix UI** and keep the **pair analysis scatter plot** (very helpful).
2) **Unified related sensors list**: all strategies feed one candidate-sensor list (not separate bucket-only views).
3) Default strategy tab: **Similarity (TSSE)**.

## High-level layout (desktop-first)

### Trends page content order
1) Trend chart (primary, top)
2) Relationship Finder panel (secondary but large, below chart)

### Responsive behavior
- When viewport is narrow, switch to a two-tab view inside the panel:
  - “Results” and “Preview”
  - Preview becomes full width.

## Shared concepts (use consistent wording everywhere)

- **Focus sensor**: sensor you compare everything against.
- **Candidate pool / scope**: which sensors are eligible to be related.
- **Window**: current Range (start→end).
- **Interval**: current chart interval (bucket size).
- **Overlap (n)**: aligned buckets used (or strategy-specific equivalent).
- **n_eff**: effective sample size (where available).
- **p / q**: p-value and BH-FDR q-value (where available).

## Panel structure (single component)

Component: `RelationshipFinderPanel` (single `CollapsibleCard`, default open).

### Header area (always visible)
- Title: “Relationship Finder”
- Short description: “Discover related sensors and why, using the current Range/Interval.”
- Status strip: “Computed through …” + job status pill

### Control area (always visible)

**Row 1: Focus + Strategy**
- Focus sensor dropdown (populated from currently-selected sensors in the Sensor picker)
  - default: if exactly 1 sensor is selected → auto-select it
  - otherwise user must pick
- Strategy tabs (segmented control):
  1) Similarity (TSSE) **(default)**
  2) Correlation
  3) Events
  4) Co-occurrence

**Row 2: Candidate pool + Run**
- Scope: Same node / All nodes / (Optional) Selected sensors only
- Filters (shared):
  - Same unit only
  - Same type only
  - Exclude public provider sensors (optional)
- Run / Cancel button
- Job status + progress (consistent UI across strategies)

**Row 3: Strategy-specific controls**
- Visible for the active strategy.
- “Advanced” section collapsed by default.

### Body area (always visible after controls)
- Left: unified **Results list** (sensors)
- Right: unified **Preview pane**

## Unified Results list (sensors only)

Every strategy outputs a list of **candidate sensors** with consistent UI/behavior.

### Result row fields (normalized)
- `sensor_id`
- `label` (sensor name)
- `node` / `type` / `unit` (secondary line)
- `rank` (1..N)
- `score` (normalized to 0..1 if possible; otherwise show strategy-specific score with label)
- `badges` (small pills; varies by strategy)
- Actions:
  - “Preview”
  - “Add to chart” (stable: never disappears; becomes “Added” when already selected)

### Sorting (consistent)
- Primary: strategy score desc
- Secondary: coverage/overlap desc (if available)
- Tie-break: stable by `sensor_id`

## Unified Preview pane

Preview is always driven by the selected candidate sensor (from results list).

Always show:
- Focus sensor label + candidate label
- Computed-through timestamp and effective params used (echoed from backend)
- Add-to-chart action (same behavior as results list)
- Clear empty state: “Select a result to preview.”

## Strategy: Similarity (TSSE) — default tab

Backend: `related_sensors_v1`

Controls (basic):
- Max candidates
- Max lag (buckets)
- Candidate filters (reuse existing scope filters)

Preview content:
- Episode list (click selects episode)
- Episode timeline preview (focus vs candidate aligned)
- “Why ranked” breakdown:
  - show `p_raw`, `p_lag`, `q`, `n_eff`, `m_lag` (when available)

Result badges (example):
- Score
- Coverage
- Best lag
- `p_lag`, `q`, `n_eff`

## Strategy: Correlation (Matrix + scatter)

Backend: `correlation_matrix_v1`

Controls:
- Method: Pearson / Spearman
- Min overlap (n)
- Min significant n
- Significance alpha

**Unified related sensors list mapping**
- Still show a candidate-sensor list (left pane) even though the backend returns a matrix:
  - candidates = sensors in the matrix excluding focus
  - ranking = by `abs(r)` when available; group by status:
    - Ok first, then NotSignificant, then InsufficientOverlap/NotComputed last
  - show badges: `r`, `q`, `n`, `n_eff`

Preview content (right pane) has two sections:
1) **Pair analysis** (always visible once a candidate is selected)
   - scatter plot
   - lag correlation plot
2) **Correlation matrix** (collapsible)
   - sticky headers
   - internal horizontal scroll only
   - clicking a cell selects that pair (if it involves focus; otherwise optional behavior)

## Strategy: Events (Spikes) Matching

Backend: `event_match_v1`

Controls:
- Polarity (up/down/both)
- Z-threshold
- Min separation buckets
- Max lag buckets
- Max events / max episodes

Unified related sensors list mapping:
- candidates returned by the job ranked by event-match score
- badges: episodes count, best lag, coverage

Preview content:
- Episode list + preview of strongest episode
- Add-to-chart action

## Strategy: Co-occurrence

Backend: `cooccurrence_v1`

Important: this strategy must still feed the unified sensor list.

Controls:
- z-threshold
- tolerance buckets
- polarity
- max results

Unified related sensors list mapping (aggregation rule):
- From co-occurrence buckets that include the focus sensor F:
  - for each other sensor S ≠ F in the bucket:
    - `sensor_score[S] += |z_F| × |z_S|`
    - where `z_F` = z-score of focus sensor's event, `z_S` = z-score of candidate's event
  - rank sensors by accumulated `sensor_score` descending
  - secondary sort: `co_occurrence_count` (number of buckets where F and S co-occurred)
  - also track per-sensor: `max_bucket_z` (strongest single co-occurrence event)

Rationale: The pairwise severity product captures co-occurrence strength—two sensors with strong simultaneous events (high |z|) score high; if either has a weak event, contribution is low.

Preview content:
- show top co-occurrence timestamps for the candidate sensor
- optional “show markers on chart” toggle
- Add-to-chart action

Optional secondary view (collapsible):
- “Top buckets” list for timestamp browsing (but not the primary output)

## Strategy: Motifs / Anomalies — excluded from v1

Backend: `matrix_profile_v1`

**Research conclusion (2026-01-26):** Matrix profile is fundamentally a **single-sensor analysis**. It finds repeating patterns (motifs) and unique patterns (anomalies) within one sensor's time series by computing self-similarity. It does NOT compare sensors to each other and cannot produce a sensor-ranked related list.

**Exclusion rationale:** The unified Relationship Finder model requires each strategy to output a ranked list of candidate sensors related to the focus sensor. Matrix profile operates on a single sensor and outputs timestamp windows (motifs/anomalies), not sensor relationships.

**Future consideration:** If multi-sensor matrix profile (comparing patterns across sensors) is implemented, it could be added. Until then, Motifs remains a standalone single-sensor tool outside Relationship Finder.

## Error + empty state rules (consistent across strategies)

- If focus sensor not selected: show “Select a focus sensor to run analysis.”
- If insufficient data: show “Focus sensor has insufficient data in this window.”
- If job fails: show backend error message + “Retry” and “Reduce window/candidates” guidance.
- Never label `r=null` or missing metrics as “not significant”.
  - Use “Not computed” with a reason: insufficient overlap, zero variance, or unsupported.

## Backend job mapping (no backend rewrite required to start)

- Similarity tab → `/api/analysis/jobs` with `job_type=related_sensors_v1`
- Correlation tab → `job_type=correlation_matrix_v1`
- Events tab → `job_type=event_match_v1`
- Co-occurrence tab → `job_type=cooccurrence_v1`

## Implementation checklist (frontend)

1) Create `RelationshipFinderPanel.tsx` hosting:
   - focus selector
   - strategy tabs
   - shared controls
   - results list + preview layout
2) Extract each strategy into its own module:
   - `strategies/tsse.ts`
   - `strategies/correlation.ts`
   - `strategies/eventMatch.ts`
   - `strategies/cooccurrence.ts`
3) Add a normalization layer:
   - `NormalizedCandidate[]` and a shared `PreviewModel`
4) Ensure Add-to-chart is a single shared component so behavior is identical.
5) Remove old standalone analysis containers once feature-parity is reached.

## Migration & Rollout Strategy

### Feature flag
- Flag name: `use_unified_relationship_finder`
- Default: `false` (off)
- Storage: user preferences (localStorage) + server-side override for beta cohort

### Rollout phases

**Phase 1: Ship alongside existing (flag off)**
- Deploy `RelationshipFinderPanel` as new component
- Old panels remain primary UI
- Internal testing with flag enabled manually

**Phase 2: Beta users (flag on for cohort)**
- Enable flag for beta users via server-side config
- Gather feedback for 2-4 weeks
- Track: usage metrics, error rates, support tickets

**Phase 3: Default on with deprecation warnings**
- Flip default to `true`
- Add deprecation banner to old panels: "This view will be removed. Try the new Relationship Finder."
- Keep old panels accessible via flag override for 4 weeks

**Phase 4: Remove old panels**
- Delete old panel components (~6,700 lines across 5 files)
- Remove feature flag and fallback logic
- Update documentation

### Files to deprecate (Phase 4)
- `SimilarityAnalysisContainer.tsx`
- `CorrelationMatrixContainer.tsx`
- `EventMatchContainer.tsx`
- `CooccurrenceContainer.tsx`
- `MatrixProfileContainer.tsx` (standalone tool, may remain separate)

## Responsive Breakpoints

### Desktop (≥1024px)
- Side-by-side layout: Results list (40%) + Preview pane (60%)
- All controls visible in single row groups
- Matrix view scrolls internally

### Tablet (768–1023px)
- Stacked layout: Results list (full width) above Preview pane
- Preview pane collapsible (default collapsed after initial selection)
- Strategy tabs wrap to 2×2 grid if needed

### Mobile (<768px)
- Tab switcher replaces side-by-side: `[Results] [Preview]`
- Only one pane visible at a time
- Selecting a result auto-switches to Preview tab
- "Back to results" button in Preview header
- Controls collapse into expandable accordion

### Breakpoint constants (CSS/Tailwind)
```css
--bp-mobile: 767px;
--bp-tablet: 1023px;
--bp-desktop: 1024px;
```

## Loading & Progress States

Each strategy has distinct backend phases. Show granular progress to set user expectations.

### UI pattern
- Animated progress bar (indeterminate until phase reports progress)
- Phase label: current step name
- Items indicator: "Processing X of Y" where applicable
- Elapsed time shown after 5 seconds

### Strategy: Similarity (TSSE)

| Phase | Label | Progress indicator |
|-------|-------|-------------------|
| `candidates` | "Finding candidates…" | Count of eligible sensors |
| `inference` | "Running inference…" | % of candidate pairs |
| `scoring` | "Scoring results…" | Indeterminate (fast) |

### Strategy: Correlation

| Phase | Label | Progress indicator |
|-------|-------|-------------------|
| `load_series` | "Loading time series…" | Count of sensors loaded |
| `correlate` | "Computing correlations…" | Matrix size (e.g., "45 of 100 pairs") |

### Strategy: Events

| Phase | Label | Progress indicator |
|-------|-------|-------------------|
| `load_series` | "Loading time series…" | Count of sensors |
| `detect_events` | "Detecting events…" | Events found so far |
| `match_candidates` | "Matching candidates…" | % complete |

### Strategy: Co-occurrence

| Phase | Label | Progress indicator |
|-------|-------|-------------------|
| `load_series` | "Loading time series…" | Count of sensors |
| `detect_events` | "Detecting events…" | Events found so far |
| `score_buckets` | "Scoring co-occurrences…" | Buckets processed |

### Completion states
- **Success:** "Completed in X.Xs — N results"
- **No results:** "Completed — no matches found" (then show empty state guidance)
- **Cancelled:** "Cancelled"
- **Failed:** "Failed: [error message]"

## Edge Cases & Constraints

### Max sensors reached (20 on chart)
- "Add to chart" button disabled
- Tooltip: "Chart limit reached (20 sensors). Remove a sensor to add more."
- Badge on disabled button: "Limit"

### Focus sensor removed from chart
- Clear analysis results immediately
- Show prompt: "Focus sensor was removed. Select a new focus sensor to continue."
- Do NOT auto-select a different focus sensor (user intent unclear)

### Window too short for analysis
- Minimum: 10 aligned buckets (configurable per strategy)
- Show: "Insufficient data: need at least 10 data points. Try a longer time range or larger interval."
- Disable Run button until window is sufficient

### Zero candidates found (after successful job)
Strategy-specific empty states:

| Strategy | Empty state message | Suggestion |
|----------|---------------------|------------|
| Similarity | "No similar sensors found in this window." | "Try expanding the candidate scope or adjusting lag settings." |
| Correlation | "No correlated sensors found." | "Try lowering the significance threshold or expanding scope." |
| Events | "No matching event patterns found." | "Try lowering the z-threshold or allowing more lag." |
| Co-occurrence | "No co-occurring events detected." | "Try lowering the z-threshold or expanding the tolerance window." |

### Job timeout
- Timeout: 5 minutes (300 seconds)
- Show: "Analysis timed out after 5 minutes."
- Actions: "Cancel" (always available) + "Retry with smaller scope"
- Smaller scope suggestion: reduce candidate pool or shorten time window

### Stale results warning
- If range/interval changed since last run: show banner "Results may be outdated. Run again to refresh."
- Do NOT auto-clear results (user may want to reference them)

## Job Management Behavior

### Deduplication via job_key
- `job_key` = hash of: `(focus_sensor_id, strategy, params_hash, window_start, window_end, interval)`
- If identical job completed within last 5 minutes: reuse cached result
- UI: "Using cached results from X minutes ago" with "Run again" option

### Auto-invalidation triggers
Clear cached results when:
- Range changes (start or end)
- Interval changes
- Focus sensor changes
- Strategy-specific params change

Do NOT invalidate when:
- User switches between strategy tabs (preserve per-strategy cache)
- Preview selection changes
- Panel collapses/expands

### Polling behavior
- Poll interval while job running: 2 seconds
- Stop polling on: completion, failure, cancellation, or timeout
- Exponential backoff on repeated failures: 2s → 4s → 8s → 16s (max)

### Concurrent job handling
- One active job per strategy at a time
- Starting a new run for the same strategy cancels the previous job
- Jobs for different strategies can run concurrently (user switches tabs)
- Show "Cancelling previous job…" briefly when superseding

### Job lifecycle states
```
idle → queued → running → [completed | failed | cancelled | timeout]
```

## Accessibility Requirements

### Strategy tabs
- Container: `role="tablist"`
- Each tab: `role="tab"`, `aria-selected="true|false"`, `tabindex="0|-1"`
- Tab panel: `role="tabpanel"`, `aria-labelledby="[tab-id]"`
- Keyboard: Arrow keys cycle tabs, Enter/Space activates

### Results list
- Container: `role="listbox"`, `aria-label="Analysis results"`
- Each row: `role="option"`, `aria-selected` for preview binding
- `aria-activedescendant` on container points to selected row
- Keyboard: Up/Down navigate, Enter previews/activates

### Preview pane
- Container: `aria-live="polite"` for content updates
- `aria-atomic="false"` (announce changes, not entire region)
- Heading structure: h3 for section titles within preview

### Progress indicators
- Progress bar: `role="progressbar"`, `aria-valuenow`, `aria-valuemin="0"`, `aria-valuemax="100"`
- Indeterminate: `aria-valuenow` omitted
- Phase label: `aria-label` on progress bar includes phase name

### Focus management
- Tab order: Strategy tabs → Scope controls → Run button → Results list → Preview actions
- After job completion: focus moves to first result (if any)
- After adding to chart: focus stays on "Added" button (confirmation)
- Escape while job running: cancels job, focus returns to Run button

### Screen reader announcements
- Job start: "Starting [strategy] analysis"
- Job completion: "[N] results found" or "No results found"
- Job failure: "Analysis failed: [reason]"
- Result selection: "[Sensor name] selected for preview"

### Color contrast
- All text meets WCAG 2.1 AA (4.5:1 for normal text, 3:1 for large)
- Status badges use both color AND text/icon (not color alone)
- Focus indicators visible (2px outline, offset)

## Keyboard Shortcuts

### Global (when panel focused)
| Key | Action |
|-----|--------|
| `1` | Switch to Similarity tab |
| `2` | Switch to Correlation tab |
| `3` | Switch to Events tab |
| `4` | Switch to Co-occurrence tab |
| `R` | Run analysis (when controls area focused) |
| `Escape` | Cancel running job |

### Results list (when list focused)
| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate results |
| `Home` | Jump to first result |
| `End` | Jump to last result |
| `Enter` | Toggle preview / Add to chart (context-dependent) |
| `A` | Add selected sensor to chart |

### Preview pane (when preview focused)
| Key | Action |
|-----|--------|
| `A` | Add candidate to chart |
| `←` | Return focus to results list |

### Discoverability
- Show keyboard hints on hover (e.g., "[R] Run")
- Optional: "Keyboard shortcuts" help tooltip in panel header

## Decisions (closed)

**Correlation candidates list:** Include `not_significant` pairs, clearly labeled, below significant ones.

Rationale:
- Users expect to see sensors they asked about—hiding them creates confusion ("why isn't sensor X listed?")
- "Not significant" is actionable information: weak correlation or insufficient sample size
- Grouping by status (Ok → NotSignificant → InsufficientOverlap) already defined in the strategy section
- Significant results appear first; users can ignore the rest if they choose
