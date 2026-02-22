# Emporia Cloud API Setup (Emporia Vue)

This guide walks through connecting an Emporia Vue monitor to Farm Dashboard
using the Emporia cloud API. The integration uses the Emporia AppAPI
`getDeviceListUsages` endpoint with a Cognito `id_token` passed as the
`authtoken` header.

## Quick path (Setup Center)

- Open the web dashboard → Setup Center → Integrations.
- Use **Emporia cloud token** to enter the Emporia username/password; the controller derives the
  Cognito token server-side and saves only the `id_token` + refresh token (no password storage).
- The controller immediately kicks an analytics feed poll (`/api/analytics/feeds/poll`) so Emporia
  readings land in `analytics_power_samples` and `analytics_integration_status`.

## Prerequisites
- An Emporia account with access to the target Vue monitor(s).
- Core server reachable to the Emporia cloud API (`https://api.emporiaenergy.com`).
- Analytics feeds enabled on the core server.

## Step 1: Get an authtoken and deviceGids
You need the Emporia `id_token` (used as `authtoken`) plus the `deviceGid`
values for the monitor(s) you want to ingest.

Notes:
- When polling multiple meters, Farm Dashboard sends Emporia a comma-separated `deviceGids` list (e.g., `1234,2345`).
- On multi-site accounts, missing meters are usually caused by an incomplete deviceGid list or a mis-encoded multi-device request; use the Setup Center meter list below to confirm everything is enabled.

### Option A: Use PyEmVue (recommended)
PyEmVue can authenticate and write the Cognito tokens to a JSON file.

1) Install PyEmVue (outside the repo is fine):
```bash
python3 -m pip install pyemvue
```

2) Run a helper script to log in and list device IDs:
```bash
python3 - <<'PY'
import json
from pyemvue import PyEmVue

vue = PyEmVue()
vue.login(username="you@email.com", password="your-password", token_storage_file="emporia_tokens.json")

devices = vue.get_devices()
device_ids = sorted({str(d.device_gid) for d in devices})
print("device_gids:", ",".join(device_ids))

with open("emporia_tokens.json", "r", encoding="utf-8") as handle:
    tokens = json.load(handle)
print("id_token:", tokens.get("id_token", "")[:20] + "...")  # do not share full token
print("refresh_token:", "present" if tokens.get("refresh_token") else "missing")
PY
```

3) Use:
- `tokens["id_token"]` -> `CORE_ANALYTICS_EMPORIA__AUTH_TOKEN`
- `tokens["refresh_token"]` -> `CORE_ANALYTICS_EMPORIA__REFRESH_TOKEN`
- `device_gids` -> `CORE_ANALYTICS_EMPORIA__SITE_IDS`

### Option B: Call the API directly (already have authtoken)
```bash
curl -H "authtoken: $EMPORIA_ID_TOKEN" \
  https://api.emporiaenergy.com/customers/devices
```
Use the returned `deviceGid` values in `CORE_ANALYTICS_EMPORIA__SITE_IDS`.

### Option C: Use the helper script
The repo includes a small helper that reads the authtoken and prints device IDs.

```bash
python tools/emporia_device_ids.py --authtoken "$EMPORIA_ID_TOKEN"
```

You can also set `EMPORIA_AUTHTOKEN` in your environment:
```bash
export EMPORIA_AUTHTOKEN="$EMPORIA_ID_TOKEN"
python tools/emporia_device_ids.py
```

## Step 2: Configure the controller (Setup Center)

The production Rust controller reads Emporia credentials from the controller database (`setup_credentials`), populated by Setup Center.

Use one of these:
- **Setup Center UI (recommended):** Setup Center → Integrations/Credentials → Emporia cloud login.
- **Advanced API (requires `config.write`):** `POST /api/setup/emporia/login` then review/update meters via `GET/PUT /api/setup/emporia/devices`.

## Step 3: Verify ingestion
1) Trigger a poll (optional):
```bash
curl -H "Authorization: Bearer <token>" -X POST http://<core-server-host>:8000/api/analytics/feeds/poll
```

2) Check feed status:
```bash
curl http://<core-server-host>:8000/api/analytics/feeds/status
```
Look for `Emporia Vue` with `status: ok` and `sites_polled` in the metadata.

3) Confirm data in analytics:
```bash
curl http://<core-server-host>:8000/api/analytics/power
```

## Optional: Exclude meters from summaries / group by address
If your Emporia account contains meters for multiple street addresses, you can keep all meters visible while excluding specific meters from system-wide totals and grouping the remainder by address.

- Open the web dashboard → Setup Center → Integrations → **Emporia meters & totals**.
- For each meter, set:
  - **Address group**: label used for the Analytics “Emporia meters by address” breakdown and Power node selector.
  - **Poll**: whether the meter is ingested at all.
  - **Hidden**: hides the meter/circuits from the rest of the dashboard while still ingesting when Poll is enabled.
  - **In totals**: whether it contributes to system totals (`/api/analytics/power`).
- Click a meter row to expand circuits and configure the same **Poll / Hidden / In totals** controls per circuit.
  - Default: **Mains in totals** is enabled and individual circuits are not included unless explicitly selected.
  - Advanced: disable **Mains in totals** and include specific circuits to build system totals from only the selected loads.

## Troubleshooting
- **401 / Unauthorized**: The `AUTH_TOKEN` (id_token) is expired or wrong.
  Regenerate tokens with PyEmVue and update `AUTH_TOKEN`/`REFRESH_TOKEN`.
- **No device IDs found**: `SITE_IDS` must be Emporia `deviceGid` values. Use
  PyEmVue or `/customers/devices` to discover them.
- **No new readings**: Emporia may return the same `instant` for several polls.
  Increase `CORE_ANALYTICS_FEED_POLL_INTERVAL_SECONDS` or wait for fresh data.
- **Password auth fails**: Some accounts require MFA. Use PyEmVue to obtain
  tokens and set `AUTH_TOKEN` directly.

## Security Notes
- Treat Emporia tokens like passwords. Store them in a secret manager or
  protected `.env` file, and avoid committing them to git.
