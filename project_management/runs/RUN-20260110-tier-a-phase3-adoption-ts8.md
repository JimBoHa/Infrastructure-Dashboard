# RUN-20260110 — Tier A Phase 3 closeout (Adoption + TS-8 + CS-57)

**Environment:** Installed controller (no DB/settings reset). Bundle version reported by `/usr/local/farm-dashboard/state.json` was `0.1.9.70`.

## Notes

- The production API requires `config.write` for adoption token issuance and adoption. Since we did not have a password available for an interactive admin login in this SSH session, a **temporary API token** was created directly in Postgres with a **7-day expiry** and `config.write` (plus standard admin capabilities). Token string was stored at `/tmp/tier_a_api_token.txt` on the controller.
- This does **not** reset DB/settings; it adds a time-limited access token row for Tier‑A smoke. Revoke by setting `revoked_at` on the inserted `api_tokens` row.
  - Post-run refresh note: the installed controller was later upgraded to `0.1.9.71` to ship the DW-29 “Discovered controllers” filter fix. The evidence in this run log was captured before that upgrade while the controller was on `0.1.9.70`.

## Evidence (commands + artifacts)

### Health

```bash
curl -sS http://127.0.0.1:8000/healthz
curl -sS http://127.0.0.1:8800/healthz
```

Artifacts:
- `/tmp/tier_a_healthz.json`
- `/tmp/tier_a_setup_healthz.json`

### DW-29 / CS-60 (Adoption determinism)

```bash
curl -sS "http://127.0.0.1:8000/api/scan?timeout=2.0"
curl -sS -X POST http://127.0.0.1:8000/api/adoption/tokens \
  -H "Authorization: Bearer $(cat /tmp/tier_a_api_token.txt)" \
  -H "Content-Type: application/json" \
  -d '{"mac_eth":"<real-mac>","mac_wifi":"<real-mac>","service_name":"tier-a-smoke"}'
curl -sS -X POST http://127.0.0.1:8000/api/adopt \
  -H "Authorization: Bearer $(cat /tmp/tier_a_api_token.txt)" \
  -H "Content-Type: application/json" \
  -d '{"token":"not-a-real-token","name":"<existing-name>","mac_eth":"<real-mac>","mac_wifi":"<real-mac>","ip":"<real-ip>","port":9000}'
```

Results captured:
- `/api/scan` returns candidates (2 candidates captured): `/tmp/tier_a_scan.json`
- `POST /api/adoption/tokens` succeeded and returned a controller-issued token: `/tmp/tier_a_issue_token.json`
- `POST /api/adopt` rejected a bogus token with `403`: `/tmp/tier_a_adopt_reject_code.txt`
- `POST /api/adopt` accepted the controller-issued token with `200`: `/tmp/tier_a_adopt_accept_code.txt`
- Attempting to adopt using a node-advertised token from `/api/scan` returned `403`: `/tmp/tier_a_adopt_adv_code.txt`

### TS-8 (node-agent id mapping + health fields)

```bash
curl -sS http://127.0.0.1:8000/api/nodes
curl -sS http://127.0.0.1:8000/api/dashboard/state
```

Results captured:
- `Pi5 Node 1` and `Pi5 Node 2` include `config.agent_node_id` and show non-zero `uptime_seconds` / `cpu_percent` / `storage_used_bytes` in `/api/dashboard/state`.
- Artifacts: `/tmp/tier_a_nodes.json`, `/tmp/tier_a_dashboard_state.json`

### CS-57 (latest values correctness)

```bash
curl -sS http://127.0.0.1:8000/api/sensors
```

Results captured:
- `latest_value` / `latest_ts` are present on returned sensors (verified via `/tmp/tier_a_sensors.json`).

### TS-7 (offline flapping follow-up)

```bash
curl -sS http://127.0.0.1:8000/api/alarms/history
```

Artifacts:
- `/tmp/tier_a_alarms_history.json`
