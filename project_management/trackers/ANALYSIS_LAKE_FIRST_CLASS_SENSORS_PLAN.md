# Analysis Lake: Derived Sensor Support — Revised Plan

Created: 2026-01-26
Revised: 2026-01-26
Status: Plan (not yet implemented)

## Problem statement

- Trends chart data comes from `GET /api/metrics/query` and supports:
  - raw `metrics`
  - `derived` sensors (computed on the fly)
  - `forecast_points` sensors (queried with as-of semantics)
- Analysis jobs (correlation matrix, TSSE, event match, co-occurrence, matrix profile) read bucketed series from the **analysis lake only** via DuckDB over Parquet, and currently only the `metrics/v1` dataset is present/queried.
- Result: a sensor can be visible and "populated" in Trends, but show `Insufficient overlap (n=0)` in Relationships if it is not represented in the lake.

**Concrete example that triggered this plan:**
- Controller Temp (raw): `025eeb6985bca09f87e4a85d`
- Node1 DC loads power (derived): `af370b03a396ea19681af459`
- Window: 2026-01-18 23:30 → 2026-01-25 23:30 (7 days)
- Interval: 10 min (600s), buckets: ~1008
- Chart shows values for both, but correlation matrix overlap is `n=0`.

## Scope decisions

### In scope: Derived sensors
Derived sensors will be supported via **query-time computation** from lake-backed inputs. No new lake dataset required.

### Out of scope: Forecast/public sensors
Forecast-backed sensors are **explicitly disabled** for analysis jobs in this plan. Rationale:
- Forecast data has different semantics (issued_at, as-of resolution) that add complexity
- Usage in analysis is rare compared to raw/derived sensors
- Can be added later if needed

**Behavior for disabled sensors:**
- If an analysis job includes a forecast-backed sensor, return a clear error:
  - code: `unsupported_sensor_source`
  - message: "Forecast-backed sensors are not supported in analysis jobs: {sensor_ids}"
- If a derived sensor transitively depends on a forecast input, the same error applies.

## Goal / Acceptance (Definition of Done)

**Functional**
- Raw and derived sensors that can appear in Trends are usable by lake-backed analyses.
- If the chart shows overlapping data for a raw + derived sensor pair at the same Range/Interval, correlation matrix overlap `n` must be > 0.
- Forecast-backed sensors return a clear error (not silent `n=0`).

**Performance**
- Derived computation adds minimal overhead to analysis jobs.
- Target: "2 sensors / 7 days / 10-minute buckets" completes in < 1s (record actual numbers).

**Consistency**
- Derived bucket values match what the chart would show for the same inputs and interval.

## Approach: Query-time derived computation

Instead of materializing derived sensors into a new lake dataset, **compute derived values on-the-fly** when analysis jobs request them.

**Why this approach:**
- Derived sensor inputs (raw sensors) are already in the lake (`metrics/v1`)
- The heavy I/O (reading input buckets) is already lake-backed
- Derived expressions are simple arithmetic (O(1) per bucket)
- No incremental recompute logic needed
- No late-arrival handling needed
- Always consistent with current input data

**How it works:**
1. Analysis job requests buckets for sensor_ids (including derived sensor D)
2. Unified reader looks up D's config, sees it's derived with expression `A + B`
3. Recursively reads input buckets for A and B from `metrics/v1`
4. Aligns buckets by epoch, applies expression
5. Returns computed derived buckets to analysis job

## Derived computation semantics (for analysis)

**Bucket-level computation (simpler than point-level):**
- Analysis jobs work with pre-bucketed data at a requested interval
- For derived sensor D with inputs A, B:
  - Read bucketed values for A and B at the requested interval
  - For each bucket epoch where **all** inputs have values, compute D
  - If any input is missing at a bucket, D is missing at that bucket

**This differs from chart point-level semantics:**
- Chart uses "union of input timestamps" with as-of joins
- Analysis uses aligned bucket epochs (simpler, appropriate for correlation/TSSE)

---

## Phase 0 — Inventory + prep

### 0.1 Enumerate derived sensors + dependencies
Run (manual):
```sql
SELECT sensor_id, name, config
FROM sensors
WHERE config->>'source' = 'derived'
  AND deleted_at IS NULL;
```
For each derived sensor, record:
- expression
- input sensor_ids
- whether inputs include other derived sensors (nested)
- whether inputs include forecast sensors (will be blocked)

### 0.2 Enumerate forecast sensors (to block)
Run (manual):
```sql
SELECT sensor_id, name
FROM sensors
WHERE config->>'source' = 'forecast_points'
  AND deleted_at IS NULL;
```
These sensor_ids will return errors if used in analysis jobs.

### 0.3 Extract common job utilities
Before implementing the unified reader, extract duplicated code from analysis jobs into a shared module.

**File:** `apps/core-server-rs/src/services/analysis/jobs/common.rs`

**Extract:**
```rust
// Currently duplicated in correlation_matrix_v1, event_match_v1, embeddings_build_v1
pub struct SensorMeta {
    pub sensor_id: String,
    pub name: String,
    pub unit: String,
    pub node_id: uuid::Uuid,
    pub sensor_type: String,
}

// Currently duplicated in all 5 analysis jobs
pub fn parse_time_range(start: &str, end: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), JobFailure>

// Currently duplicated in 3+ jobs
pub async fn fetch_sensor_metadata(
    db: &PgPool,
    sensor_ids: &[String]
) -> Result<HashMap<String, SensorMeta>, JobFailure>
```

**Benefit:** Reduces ~30 lines of duplicated code per job, ensures consistent error messages.

---

## Phase 1 — Unified bucket-series reader

### 1.1 New unified API
**File:** `apps/core-server-rs/src/services/analysis/bucket_reader.rs` (new)

```rust
pub async fn read_bucket_series_for_sensors(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    sensor_ids: Vec<String>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval_seconds: i64,
) -> Result<Vec<MetricsBucketRow>, BucketReaderError>
```

**Error type:**
```rust
pub enum BucketReaderError {
    UnsupportedSensorSource { sensor_ids: Vec<String>, source: String },
    DerivedCycleDetected { sensor_id: String, cycle: Vec<String> },
    DerivedDepthExceeded { sensor_id: String, depth: usize },
    DerivedInputMissing { sensor_id: String, missing_input: String },
    LakeReadFailed { source: anyhow::Error },
}
```

### 1.2 Sensor source routing
For each sensor_id:
1. Query `sensors.config->>'source'` to determine type
2. Route:
   - `NULL` or empty → raw sensor → read from `metrics/v1`
   - `"derived"` → compute from inputs (see 1.3)
   - `"forecast_points"` → return `UnsupportedSensorSource` error

### 1.3 Derived sensor computation
When a derived sensor is requested:

1. **Parse config:** Extract expression and input sensor_ids from `sensors.config->'derived'`
2. **Check for cycles:** Track visited sensor_ids; if revisited, return `DerivedCycleDetected`
3. **Check depth:** If recursion depth > 10, return `DerivedDepthExceeded`
4. **Recursively read inputs:** Call `read_bucket_series_for_sensors` for input sensor_ids
5. **Align buckets:** Group input buckets by epoch
6. **Evaluate expression:** For each epoch where all inputs have values:
   - Use `derived_sensors::compile_derived_sensor()` (existing module)
   - Apply expression to input values
   - Emit derived bucket row
7. **Return:** Computed bucket rows for the derived sensor

### 1.4 Bucket alignment
All bucket epochs use the same formula regardless of source:
```
bucket_epoch = floor(unix_timestamp / interval_seconds) * interval_seconds
```

---

## Phase 2 — Wire into analysis jobs

### 2.1 Correlation matrix job
**File:** `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs`

Changes:
- Replace `duckdb.read_metrics_buckets_from_lake(...)` with `read_bucket_series_for_sensors(...)`
- Handle `BucketReaderError::UnsupportedSensorSource` → convert to `JobFailure::Failed` with clear message
- Handle other `BucketReaderError` variants appropriately

### 2.2 Other analysis jobs
Apply same changes to:
- `related_sensors_v1.rs`
- `event_match_v1.rs`
- `cooccurrence_v1.rs`
- `matrix_profile_v1.rs`

### 2.3 "Returned sensors" consistency
**Note:** TSSE-31 already fixed the "insufficient overlap" vs "not significant" vs "not computed" distinction. Ensure the unified reader's error handling is consistent with this.

When a sensor has 0 buckets in the result:
- If it's a derived sensor with missing inputs → include in error details
- If it's a raw sensor with no data in range → include in `missing_sensor_ids` (optional field for UX)

---

## Phase 3 — Validation + tests

### 3.1 Unit tests
**File:** `apps/core-server-rs/src/services/analysis/bucket_reader.rs`

Add tests for:
- Raw sensor: reads from lake correctly
- Derived sensor: computes from inputs correctly
- Nested derived: A depends on B (derived), B depends on C (raw)
- Cycle detection: A → B → A returns error
- Depth limit: deeply nested derived returns error
- Forecast sensor: returns `UnsupportedSensorSource` error
- Derived with forecast input: returns error (transitive)
- Mixed request: raw + derived in same call works

### 3.2 Regression reproduction test
Recreate the original failure:
- Create raw sensor R with data
- Create derived sensor D = R * 2
- Request correlation matrix for R and D
- Assert overlap `n > 0`

### 3.3 Tier-A acceptance (manual)
On installed controller:
- Select the known raw + derived sensors from problem statement
- Range = 7d, Interval = 10m
- Correlation matrix overlap `n` must be > 0 (not `n=0`)

---

## Phase 4 — Cleanup

- Remove any TODOs or temporary code from unified reader
- Update analysis job documentation to note derived sensor support
- Ensure forecast sensor error messages are user-friendly

---

## Implementation order

1. **Phase 0.3:** Extract common job utilities (reduces noise in later diffs)
2. **Phase 1:** Implement unified bucket reader with derived support
3. **Phase 2:** Wire all analysis jobs to use unified reader
4. **Phase 3:** Add tests + Tier-A validation
5. **Phase 4:** Cleanup

---

## References

- **TSSE-6 run log:** "replication targets `metrics` only (forecast/derived series not yet decided)" — this plan resolves that decision
- **TSSE-31:** Already fixed "insufficient overlap" vs "not significant" vs "not computed" distinction
- **Existing derived evaluation:** `apps/core-server-rs/src/services/derived_sensors.rs` — reuse for expression compilation/evaluation

---

## Future considerations

### Forecast sensor support (deferred)
If forecast sensors become needed for analysis:
1. Add `forecast_points/v1` dataset to lake
2. Export forecast_points keyed by sensor_id
3. Extend unified reader to resolve as-of issuance at bucket level
4. Remove the `UnsupportedSensorSource` block

### Derived sensor caching (if needed)
If query-time derived computation becomes a bottleneck:
1. Add optional in-memory cache for computed derived buckets
2. Key: (sensor_id, start, end, interval)
3. TTL: short (seconds) since inputs may change
4. Only implement if profiling shows derived computation is actually slow
