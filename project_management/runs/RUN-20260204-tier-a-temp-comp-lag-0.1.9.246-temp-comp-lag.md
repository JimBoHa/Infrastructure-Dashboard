# RUN: Tier‑A validation — Temp Compensation lag (installed controller; no DB reset)

Date: 2026-02-04  
Operator: Codex

Scope:
- Dashboard: DW-216, DW-217 (Analytics → Temp Compensation)
- Core: CS-99, CS-100 (forecast_points derived history + per-input `lag_seconds`)

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md`
- `project_management/TASKS.md`

---

## Preconditions (hard gates)

- ✅ No DB/settings reset performed (Tier‑A rule).
- ✅ Setup daemon reachable:
  - `curl -fsS http://127.0.0.1:8800/healthz` → `{"status":"ok"}`
- ✅ Core server reachable:
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
- ✅ Repo worktree clean (Tier‑A bundle build hard gate):
  - `git status --porcelain=v1 -b` → `## main...origin/main`
  - `git diff --stat` → (empty)

---

## Build + refresh (Tier‑A)

### Version

- Installed version before:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - `current_version`: `0.1.9.245-temp-comp-detrend`
  - `previous_version`: `0.1.9.244-major-bug-fixes`
- New version built + applied:
  - `0.1.9.246-temp-comp-lag`

### Commands run

- Build controller bundle (stable path):
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.246-temp-comp-lag --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.246-temp-comp-lag.dmg --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.246-temp-comp-lag.log`
- Point setup daemon at the bundle:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.246-temp-comp-lag.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Verify installed version:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - `current_version`: `0.1.9.246-temp-comp-lag`
  - `previous_version`: `0.1.9.245-temp-comp-detrend`

### Health verification

- ✅ `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- ✅ Installed smoke:
  - `make e2e-installed-health-smoke` → PASS

---

## Evidence (Tier‑A)

### UI evidence (captured AND viewed)

- Screenshot (VIEWED):
  - `manual_screenshots_web/20260204_041605_temp_comp/analytics_compensation_temp_lag.png`

### Real-data sanity (raw vs compensated)

Sensors:
- Raw reservoir depth: `ea5745e00cb0227e046f6b88` (“Reservoir Depth”, unit `ft`)
- Temperature reference: `93d88fc2f3187504891ba04a` (“Weather temperature (°C)”, unit `degC`, `source: forecast_points`)
- Old derived temp-comp (no lag): `645dfc54ab3aa62e38a9f8d9`
- New derived temp-comp (lagged): created `0875e5462a1165e8d2e11c09` (“Reservoir Depth (temp comp lag 155m)”) with `inputs[].lag_seconds = 9300`

Metrics query window:
- 72h (bucket interval: 300s) via `/api/metrics/query`

Summary (P95–P5 swing):
- Old derived (no lag): ~12.8% reduction vs raw (under-compensated).
- New derived (lag 155m): raw swing `0.1022 ft` → compensated swing `0.0598 ft` (~41.5% reduction).

Notes:
- The compensation UI auto-selected `155 min` lag for this pair (raw[t] aligned to temp[t−lag]) and showed swing reduction improving from ~16% @0-lag to ~41% with lag (matches screenshot).

---

## Result

- ✅ Tier‑A rebuild/refresh succeeded (installed controller upgraded to `0.1.9.246-temp-comp-lag`).
- ✅ Tier‑A health smoke passed.
- ✅ Temp Compensation lag UI is present and auto-selects a meaningful thermal lag for the reservoir depth vs weather temperature pair.
- ✅ Lagged derived sensor history returns non-empty series via `/api/metrics/query` and materially reduces daily swing on real data.

