# Analytics feeds (controller)

Analytics feeds are controller-run polls that create/refresh “virtual” telemetry (for example Emporia cloud devices) used by the **Analytics** and **Power** pages.

## What exists today

- **Emporia cloud ingest** (supported): configured via Setup Center; tokens are stored in the controller DB (`setup_credentials`) and refreshed automatically.
- Other integrations may exist as node-driven telemetry (for example Renogy via a Pi node) rather than controller polling.

## Enable feeds (controller config)

Environment defaults (used when building/running from source):

```bash
CORE_ENABLE_ANALYTICS_FEEDS=true
CORE_ANALYTICS_FEED_POLL_INTERVAL_SECONDS=300
```

Production installs edit these via **Setup Center → Controller configuration** (no manual env editing required).

Notes:
- The controller enforces a minimum poll interval (to avoid hammering upstream APIs).
- Feed health is visible in Setup Center and via the API below.

## Emporia cloud credential (Setup Center)

Preferred workflow:
1) Dashboard → **Setup Center** → **Emporia cloud** (Credentials/Integrations section).
2) Enter Emporia username/password.
3) The controller stores only tokens (id_token + refresh token) and triggers an immediate poll.

Advanced API (requires `config.write`):
- `POST /api/setup/emporia/login`
- `GET /api/setup/emporia/devices`
- `PUT /api/setup/emporia/devices` (meter/circuit preferences like Poll/Hidden/In totals)

Provider runbook:
- `docs/runbooks/emporia-cloud-api.md`

## Verify

- Feed health: `GET /api/analytics/feeds/status`
- Manual poll (requires `config.write`): `POST /api/analytics/feeds/poll`
- Analytics power payload: `GET /api/analytics/power`

## Troubleshooting

- **Status `missing`:** Emporia credential not configured (run Setup Center login).
- **Status `error` with auth errors:** re-run Setup Center login (refresh token may be missing/expired).
- **No devices / unexpected totals:** review the Emporia devices/circuits preferences screen in Setup Center.
