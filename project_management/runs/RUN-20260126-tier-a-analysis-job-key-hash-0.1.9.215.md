# RUN-20260126 — Tier A refresh: analysis job_key hash fix (0.1.9.215)

## Summary

- **Goal:** Fix `Request failed (500): Database error` when creating analysis jobs with very large `job_key` payloads (observed via Trends “Co-occurring anomalies” / “Events (Spikes) Matches” UX when many sensors are involved).
- **Root cause:** Postgres unique index on `(job_type, job_key)` can fail for very large `job_key` values, causing `POST /api/analysis/jobs` to return `500 Database error`.
- **Fix:** Store a fixed-size SHA-256 `job_key_hash` for dedupe/indexing and drop the old `(job_type, job_key)` unique index.
- **Tier:** A (installed controller refresh; **no DB/settings reset**).

## Versions

- Previous installed version: `0.1.9.214`
- New installed version: `0.1.9.215`

## Artifacts

- DMG: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.215.dmg`
- Bundle build log: `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.215.log`

## Build (local)

Built with repo `farmctl` (the installed `/usr/local/farm-dashboard/bin/farmctl bundle` was missing dashboard build inputs and failed during `npm run build`):

```bash
cd /Users/FarmDashboard/farm_dashboard

# Build farmctl used for bundling
cd apps/farmctl && cargo build --release

# Build controller DMG
cd /Users/FarmDashboard/farm_dashboard
./apps/farmctl/target/release/farmctl bundle \
  --profile prod \
  --version 0.1.9.215 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.215.dmg \
  --native-deps /usr/local/farm-dashboard/native |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.215.log
```

## Upgrade (installed controller)

Configure setup-daemon to point at the new DMG:

```bash
curl -sS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.215.dmg"}'
```

Trigger upgrade (setup-daemon runs `farmctl upgrade`):

```bash
curl -sS -X POST http://127.0.0.1:8800/api/upgrade -H 'Content-Type: application/json' -d '{}'
```

## Validation (Tier A)

- `GET http://127.0.0.1:8000/healthz` → `200 {"status":"ok"}`
- `make e2e-installed-health-smoke` → PASS

### Bug reproduction / verification

On `0.1.9.214`, creating a job with a very large `job_key` could return `500 Database error`.

On `0.1.9.215`, the same request succeeds (`200 OK`) because dedupe is now keyed by `job_key_hash`:

```bash
TOKEN="$(cat /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt)"
python3 - <<'PY' > /tmp/big_job_key.json
import json
ids=[("%024x"%i) for i in range(20000)]
print(json.dumps({"v":1,"candidates":ids}))
PY

curl -sS -X POST http://127.0.0.1:8000/api/analysis/jobs \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d "$(jq -nc --arg jk "$(cat /tmp/big_job_key.json)" '{job_type:"noop_v1",dedupe:true,job_key:$jk,params:{steps:1}}')"
```

