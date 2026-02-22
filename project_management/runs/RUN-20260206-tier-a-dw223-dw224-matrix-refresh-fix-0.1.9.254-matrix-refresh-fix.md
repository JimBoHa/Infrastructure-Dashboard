# RUN-20260206 Tier A — DW-223 + DW-224 (`0.1.9.254-matrix-refresh-fix`)

## Summary
- Date: 2026-02-06
- Scope: Tier A validation for:
  - DW-223 (Related Sensors matrix-first scan in Simple mode, score-cutoff include + cap)
  - DW-224 (separate Selected Sensors correlation matrix card)
  - Follow-up fix: stop repeated Related Sensors matrix re-submits that caused UI layout jitter.
- Commits:
  - `ba77d51` (DW-223/DW-224 implementation)
  - `2e2463d` (refresh-loop fix in `RelationshipFinderPanel`)
- Installed baseline before upgrade: `0.1.9.253-matrix-visual-scan`
- Upgrade target: `0.1.9.254-matrix-refresh-fix`
- Bundle path: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.254-matrix-refresh-fix.dmg`

## Build (controller bundle DMG)
```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.254-matrix-refresh-fix \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.254-matrix-refresh-fix.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.254-matrix-refresh-fix.log
```

Result:
- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.254-matrix-refresh-fix.dmg`

## Upgrade (setup-daemon)
```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.254-matrix-refresh-fix.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Upgrade response highlights:
- `ok: true`
- `farmctl stdout: Upgraded to 0.1.9.254-matrix-refresh-fix`
- `farmctl returncode: 0`

Version check after upgrade:
```bash
curl -fsS http://127.0.0.1:8800/api/status | jq '{current_version: .result.current_version, previous_version: .result.previous_version}'
```

Output:
- `current_version`: `0.1.9.254-matrix-refresh-fix`
- `previous_version`: `0.1.9.253-matrix-visual-scan`

## Tier A validation
Health checks:
```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
```

Output:
- setup-daemon: `{"status":"ok"}`
- core-server: `{"status":"ok"}`

Installed smoke:
```bash
make e2e-installed-health-smoke
```

Output:
- `e2e-installed-health-smoke: PASS`

## No-refresh-loop verification (installed UI)
Targeted Playwright probe on installed `http://127.0.0.1:8000/analytics/trends`:
- selected two sensors (`PV Power`, `Battery Current`)
- observed analysis-job POSTs for 10 seconds after Related Sensors panel became visible

Observed:
- `callsInWindow: 2`
- one `correlation_matrix_v1` job for Related Sensors matrix
- one `correlation_matrix_v1` job for Selected Sensors matrix
- no repeated re-submission loop in the observation window

## UI evidence (captured + viewed)
Screenshot command:
```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Screenshot directory:
- `manual_screenshots_web/20260206_025805/`

Viewed DW-223 evidence:
- `manual_screenshots_web/20260206_025805/trends_related_sensors_scanning.png`

Viewed DW-224 evidence:
- `manual_screenshots_web/20260206_025805/trends_selected_sensors_matrix_card_result.png`

Additional captured DW-224 state:
- `manual_screenshots_web/20260206_025805/trends_selected_sensors_matrix_card.png` (loading state)

## Outcome
- Tier A: PASS on installed controller.
- Installed version now serving matrix-first + selected-matrix + no-refresh-loop behavior: `0.1.9.254-matrix-refresh-fix`.
