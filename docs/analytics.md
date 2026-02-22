# Analytics

Analytics is the controller’s “at a glance” view that composes:
- Local telemetry (node-agent → MQTT → telemetry-sidecar → TimescaleDB)
- Controller-owned integrations (for example Emporia cloud) ingested on a schedule
- Forecast-backed sensors when forecast ingestion is enabled

## Configure (operator path)

1) Open the dashboard → **Setup Center**.
2) Under **Controller configuration**:
   - Ensure **Analytics feeds** is enabled.
   - Set a poll interval (server enforces a minimum).
3) Under **Credentials / Integrations**:
   - Configure Emporia (if used).
4) For location-dependent panels (weather / PV):
   - Set the node’s location in **Nodes** → **Node detail** (latitude/longitude).

## Verify

- Feed health: `GET /api/analytics/feeds/status`
- Manual poll (requires `config.write`): `POST /api/analytics/feeds/poll`
- Power composition: `GET /api/analytics/power`
- UI: **Analytics** loads without “Failed to load …” errors and shows recent timestamps.

## Troubleshooting

- **Feeds show `missing`:** credential not configured (use Setup Center).
- **Feeds show `error`:** verify outbound HTTPS/DNS, then re-run the Setup Center login flow.
- **Charts are empty:** confirm sensors exist and are ingesting (Nodes/Sensors pages; telemetry-sidecar health).

## References

- `docs/analytics_feeds.md`
- `docs/runbooks/emporia-cloud-api.md`
- `docs/runbooks/controller-rebuild-refresh-tier-a.md`
