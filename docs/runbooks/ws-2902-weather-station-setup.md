# WS-2902 Weather Station Setup (One‑Click + 30s Trending)

This runbook configures a WS‑2902‑class weather station console to push observations to the controller over HTTP, and trends the default weather sensors at ~30 second cadence.

The controller integration is push‑based (station uploads to a custom server endpoint) because it is simpler and more reliable than polling.

---

## 1) Prerequisites

- The controller (Farm Dashboard) is installed and running.
- Your weather station console/app supports “custom server” uploads (Weather Underground / Ambient‑style querystring uploads).
- The station can reach the controller over the LAN (same network segment or routable LAN).

---

## 2) Create the integration in the dashboard

1) Open the web dashboard.
2) Go to **Nodes**.
3) In **Discovered controllers**, click **Add weather station (WS‑2902)**.
4) Enter:
   - **Station nickname** (e.g. “Barn weather”)
   - **Upload protocol** (start with Weather Underground unless you know you need Ambient‑style)
   - **Upload interval** (default 30s)
5) Click **Create integration**.

The wizard will show:
- Host
- Port
- Path (token embedded)
- Full URL

---

## 3) Configure the station custom server upload

In your station app/console settings, find the custom server upload configuration and set:

- **Host**: the controller hostname/IP shown in the wizard (no scheme)
- **Port**: the controller port shown in the wizard
- **Path**: the exact path shown in the wizard (includes the token)
- **Interval**: 30 seconds (or the closest available)

Save the settings and wait for the next upload.

---

## 4) Validate + troubleshoot

Back in the wizard:

1) Click **Refresh status**.
2) Click **Test ingestion** and wait for the next station upload.

Tip: if you don’t have hardware yet, click **Send sample upload** to validate the ingest pipeline end‑to‑end without leaving the dashboard.

If the upload is received, the status will show:
- `Last upload` timestamp
- Any `Missing fields` (if the station didn’t include one of the required values)

### Simulate an upload (no hardware)

You can validate end‑to‑end without hardware by sending a WU‑style payload:

```bash
curl "http://<host>:<port>/api/ws/<token>?dateutc=now&tempf=72.5&humidity=44&windspeedmph=5.4&windgustmph=8.1&winddir=180&dailyrainin=0.12&rainin=0.01&uv=3.1&solarradiation=455&baromin=29.92"
```

Then confirm:
- Sensors exist under the weather station node in **Sensors & Outputs**
- Trends render for those sensors

### Common errors

- **403 Integration disabled**: re‑enable the integration (or recreate it).
- **404 Unknown token**: token is wrong or has been rotated; use the current token from the wizard.
- **Missing fields**: the station protocol selection doesn’t match the firmware/app payload; try switching protocol and re‑creating the integration.

---

## 5) Token rotation (security)

The ingest token is embedded in the URL path. If you need to reconfigure the station or suspect the token leaked:

1) Go to **Nodes** and open the weather station node details (the node is named like “Weather station — <nickname>”).
2) Click **Rotate token / setup**.
3) Click **Rotate token**, then update the station’s server path with the new token.

Note: after rotation, the old token/path should return **404 Unknown token**.
