# RUN-20260110 — Tier A Phase 3 re-verification (installed controller `0.1.9.71`)

**Environment:** Installed controller (Tier A; no DB/settings reset).

## Evidence (commands + artifacts)

### Health

```bash
curl -sS http://127.0.0.1:8000/healthz
curl -sS http://127.0.0.1:8800/healthz
```

Artifacts:
- `/tmp/tier_a_phase3/healthz.json`
- `/tmp/tier_a_phase3/setup_healthz.json`

### DW-29 / CS-60 (Adoption determinism)

```bash
curl -sS "http://127.0.0.1:8000/api/scan?timeout=2.0"
curl -sS -X POST http://127.0.0.1:8000/api/adoption/tokens \
  -H "Authorization: Bearer $(cat /tmp/tier_a_api_token.txt)" \
  -H "Content-Type: application/json" \
  -d '{"mac_eth":"<scan-mac>","service_name":"tier-a-reverify"}'

# Reject node-advertised token + accept controller-issued token.
curl -sS -X POST http://127.0.0.1:8000/api/adopt -H "Authorization: Bearer $(cat /tmp/tier_a_api_token.txt)" \
  -H "Content-Type: application/json" -d '{"token":"not-a-real-token","name":"tier-a-noop","mac_eth":"<scan-mac>","ip":"<scan-ip>","port":9000}'
curl -sS -X POST http://127.0.0.1:8000/api/adopt -H "Authorization: Bearer $(cat /tmp/tier_a_api_token.txt)" \
  -H "Content-Type: application/json" -d '{"token":"<node-advertised-token>","name":"tier-a-noop","mac_eth":"<scan-mac>","ip":"<scan-ip>","port":9000}'
curl -sS -X POST http://127.0.0.1:8000/api/adopt -H "Authorization: Bearer $(cat /tmp/tier_a_api_token.txt)" \
  -H "Content-Type: application/json" -d '{"token":"<controller-token>","name":"<existing-name>","mac_eth":"<scan-mac>","ip":"<scan-ip>","port":9000}'
```

Artifacts:
- `/tmp/tier_a_phase3/scan.json`
- `/tmp/tier_a_phase3/issue_token.json`
- `/tmp/tier_a_phase3/adopt_reject_code.txt` (expected `403`)
- `/tmp/tier_a_phase3/adopt_adv_code.txt` (expected `403`)
- `/tmp/tier_a_phase3/adopt_accept_code.txt` (expected `200`)

### TS-8 (node-agent id mapping + health fields)

```bash
curl -sS http://127.0.0.1:8000/api/nodes
curl -sS http://127.0.0.1:8000/api/dashboard/state
```

Results (Pi nodes only):
- `Pi5 Node 1` and `Pi5 Node 2` include `nodes.config.agent_node_id`.
- `/api/dashboard/state` reports non-zero `uptime_seconds` / `cpu_percent` / `storage_used_bytes` for the Pi nodes after status publishes.

Artifacts:
- `/tmp/tier_a_phase3/nodes.json`
- `/tmp/tier_a_phase3/dashboard_state.json`

### CS-57 (latest values correctness)

```bash
curl -sS http://127.0.0.1:8000/api/sensors
```

Artifacts:
- `/tmp/tier_a_phase3/sensors.json`

