# RUN-20260124 — Tier A — DW-194 Trends “Key” jargon expansion (0.1.9.213)

- **Date:** 2026-01-24
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.213
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.213.dmg`

## Scope

- DW-194: Update the Trends “Key” panels so any jargon/abbreviations used are introduced on first use for a general scientific audience:
  - Expand **Time‑Series Similarity Engine (TSSE)**.
  - Expand **median absolute deviation (MAD)**.
  - Expand **F1 score** (precision/recall).
  - Define correlation coefficient **r** and overlap count **n**.

## Web build + smoke (repo)

```bash
make ci-web-smoke-build
```

Result: `PASS`.

## Additional CI (non‑iOS)

```bash
make ci-smoke
make ci-web-full
make ci-node
make ci-farmctl
```

Result: all `PASS`.

## Refresh installed controller (Upgrade)

Preflight health checks:

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
curl -fsS http://127.0.0.1:6333/healthz
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

Installed version before: `0.1.9.212`.

Bundle build:

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.213 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.213.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.213.log
```

Refresh/upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.213.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

Installed version after: `0.1.9.213` (previous: `0.1.9.212`).

Installed stack smoke:

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier‑A screenshots (captured + viewed)

### Sidebar sweep (baseline)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt \
  --out-dir=manual_screenshots_web/tier_a_0.1.9.213_dw194_20260124_192400Z
```

Saved screenshots to `manual_screenshots_web/tier_a_0.1.9.213_dw194_20260124_192400Z`.

### Trends Keys (focused evidence)

```bash
cd apps/dashboard-web
FARM_PLAYWRIGHT_AUTH_TOKEN="$(cat /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt)" \
FARM_TIER_A_VERSION="0.1.9.213" \
npx playwright test playwright/trends-auto-compare-tier-a.spec.ts --project=chromium-desktop

FARM_PLAYWRIGHT_AUTH_TOKEN="$(cat /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt)" \
FARM_TIER_A_VERSION="0.1.9.213" \
npx playwright test playwright/trends-cooccurrence-tier-a.spec.ts --project=chromium-desktop

FARM_PLAYWRIGHT_AUTH_TOKEN="$(cat /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt)" \
FARM_TIER_A_VERSION="0.1.9.213" \
npx playwright test playwright/trends-event-match-tier-a.spec.ts --project=chromium-desktop

FARM_PLAYWRIGHT_AUTH_TOKEN="$(cat /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt)" \
FARM_TIER_A_VERSION="0.1.9.213" \
npx playwright test playwright/trends-relationships-tier-a.spec.ts --project=chromium-desktop
```

Results: all `PASS`.

Evidence (opened and visually reviewed):
- `manual_screenshots_web/tier_a_0.1.9.213_trends_auto_compare_2026-01-24_193342626Z/01_trends_auto_compare_key.png`
  - Verified: Related sensors Key spells out **Time‑Series Similarity Engine (TSSE)** and defines focus sensor/candidates/episodes/buckets.
- `manual_screenshots_web/tier_a_0.1.9.213_trends_cooccurrence_2026-01-24_193523567Z/01_trends_cooccurrence_key.png`
  - Verified: Co-occurrence Key expands **median absolute deviation (MAD)** on first mention.
- `manual_screenshots_web/tier_a_0.1.9.213_trends_event_match_2026-01-24_193600108Z/01_trends_event_match_key.png`
  - Verified: Events/Spikes matching Key expands **F1 score** and clarifies lag as a time shift in seconds.
- `manual_screenshots_web/tier_a_0.1.9.213_trends_relationships_2026-01-24_193138186Z/01_trends_relationships_key.png`
  - Verified: Relationships Key defines correlation coefficient **r** and overlap count **n** (number of overlapping Interval time buckets).
