# RUN-20260206 Tier A — DW-222 Preview Fallback (`0.1.9.252-preview-fallback`)

## Summary
- Date: 2026-02-06
- Scope: Tier A validation for DW-222 (Related Sensors preview fallback when lag-aligned candidate series is too sparse)
- Commit: `506ea11`
- Installed baseline before upgrade: `0.1.9.251-related-unified-v2`
- Upgrade target: `0.1.9.252-preview-fallback`
- Bundle path: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.252-preview-fallback.dmg`

## Build (controller bundle DMG)
```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs
cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.252-preview-fallback \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.252-preview-fallback.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.252-preview-fallback.log
```

Result:
- `created: /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.252-preview-fallback.dmg`

## Upgrade (setup-daemon)
```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.252-preview-fallback.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Upgrade response:
- `ok: true`
- `farmctl stdout: Upgraded to 0.1.9.252-preview-fallback`
- `farmctl returncode: 0`

Version check after upgrade:
```bash
curl -fsS http://127.0.0.1:8800/api/status | jq '{ok: .ok, current_version: .result.current_version, previous_version: .result.previous_version}'
```

Output:
- `current_version`: `0.1.9.252-preview-fallback`
- `previous_version`: `0.1.9.251-related-unified-v2`

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
- `manual_screenshots_web/20260206_015437/`

Viewed evidence:
- `manual_screenshots_web/20260206_015437/trends_related_sensors_large_scan.png`
- `manual_screenshots_web/20260206_015437/trends_related_sensors_scanning.png`

## Targeted backend evidence for user-reported pair
Focus sensor:
- `65988a7bc31fe586e0deb805` (`Mains_C Current`, interval 300s)

Candidate sensor:
- `ea5745e00cb0227e046f6b88` (`Reservoir Depth`, interval 1s)

`related_sensors_unified_v2` (24h window) returned:
- `confidence_tier`: `high`
- `blended_score`: `1.0`
- `cooccurrence_count`: `20`
- `events_overlap`: `13`

`cooccurrence_v1` (24h window) returned:
- `bucket_count`: `70`
- `counts.buckets`: `1440`
- `counts.event_count`: `236`

Interpretation:
- There is real backend co-occurrence evidence for this pair.
- The preview symptom (candidate appears as a single point in some cases) was a frontend rendering issue when lag-aligned series became too sparse; DW-222 fallback now uses raw candidate series in that sparse case.

## Outcome
- Tier A: PASS on installed controller.
- Installed version now serving fix: `0.1.9.252-preview-fallback`.
