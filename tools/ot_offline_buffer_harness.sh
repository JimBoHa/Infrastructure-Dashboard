#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Offline telemetry buffering harness (OT-9 / TICKET-0049)

Runs an outage → buffer → reboot-mid-outage → reconnect → replay validation loop against a Pi node.

Prereqs:
  - Key-based SSH access to the node (no password prompts).
  - Node runs node-forwarder (HTTP on 127.0.0.1:9101) and node-agent under systemd.
  - The controller core-server is reachable for liveness checks (default: http://127.0.0.1:8000).

Usage:
  tools/ot_offline_buffer_harness.sh --ssh <user@host> [--core-url <url>] [--offline-seconds <n>] [--replay-timeout-seconds <n>]

Example:
  tools/ot_offline_buffer_harness.sh --ssh node1@10.255.8.170 --core-url http://127.0.0.1:8000
EOF
}

SSH_TARGET=""
CORE_URL="http://127.0.0.1:8000"
OFFLINE_SECONDS="60"
REPLAY_TIMEOUT_SECONDS="600"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ssh)
      SSH_TARGET="$2"
      shift 2
      ;;
    --core-url)
      CORE_URL="$2"
      shift 2
      ;;
    --offline-seconds)
      OFFLINE_SECONDS="$2"
      shift 2
      ;;
    --replay-timeout-seconds)
      REPLAY_TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$SSH_TARGET" ]]; then
  echo "Missing --ssh" >&2
  usage
  exit 2
fi

ssh_quiet() {
  ssh -o BatchMode=yes -o ConnectTimeout=5 "$SSH_TARGET" "$@"
}

node_forwarder_status_json() {
  ssh_quiet "curl -fsS http://127.0.0.1:9101/v1/status"
}

json_number() {
  local key="$1"
  python3 -c 'import json,sys; key=sys.argv[1]; payload=json.load(sys.stdin); value=payload.get(key); print(value if isinstance(value,(int,float)) and not isinstance(value,bool) else "")' "$key"
}

json_string() {
  local key="$1"
  python3 -c 'import json,sys; key=sys.argv[1]; payload=json.load(sys.stdin); value=payload.get(key); print(value if isinstance(value,str) else "")' "$key"
}

core_nodes_json() {
  curl -fsS "$CORE_URL/api/nodes"
}

core_node_status() {
  local host="$1"
  local mac_eth="$2"
  local agent_id="$3"
  python3 -c 'import json,sys; host=sys.argv[1].strip(); mac=sys.argv[2].strip().lower(); agent=sys.argv[3].strip(); nodes=json.load(sys.stdin); m=next((n for n in nodes if ((host and (n.get("ip_last") or "").strip()==host) or (mac and (((n.get("mac_eth") or "").strip().lower()==mac) or ((n.get("mac_wifi") or "").strip().lower()==mac))) or (isinstance((cfg:=(n.get("config") or {})),dict) and ((cfg.get("agent_node_id")==agent) or (cfg.get("node_id")==agent))))), None); print((m or {}).get("status") or "")' "$host" "$mac_eth" "$agent_id"
}

wait_for_ssh() {
  local deadline="$1"
  while true; do
    if ssh -o BatchMode=yes -o ConnectTimeout=3 "$SSH_TARGET" "echo ok" >/dev/null 2>&1; then
      return 0
    fi
    if [[ "$(date +%s)" -ge "$deadline" ]]; then
      return 1
    fi
    sleep 2
  done
}

echo "[ot-harness] Target: $SSH_TARGET"
echo "[ot-harness] Core URL: $CORE_URL"

NODE_HOST="${SSH_TARGET#*@}"
NODE_MAC_ETH="$(ssh_quiet "cat /sys/class/net/eth0/address 2>/dev/null | tr -d '\r' || true")"

NODE_AGENT_ENV="/etc/node-agent.env"
NODE_ID="$(ssh_quiet "bash -lc 'set -euo pipefail; source $NODE_AGENT_ENV >/dev/null 2>&1 || true; echo \"\${NODE_NODE_ID:-}\"'")"
if [[ -z "$NODE_ID" ]]; then
  # Fallback: parse NODE_NODE_ID directly.
  NODE_ID="$(ssh_quiet "bash -lc 'set -euo pipefail; grep -E \"^NODE_NODE_ID=\" -m 1 $NODE_AGENT_ENV | cut -d= -f2- | tr -d \"\\r\"'")"
fi
if [[ -z "$NODE_ID" ]]; then
  echo "[ot-harness] Failed to determine NODE_NODE_ID from $NODE_AGENT_ENV" >&2
  exit 1
fi

echo "[ot-harness] Node MQTT id: $NODE_ID"
echo "[ot-harness] Node host: $NODE_HOST"
echo "[ot-harness] Node MAC (eth0): ${NODE_MAC_ETH:-unknown}"

BASE_STATUS="$(node_forwarder_status_json)"
BASE_STREAM_ID="$(echo "$BASE_STATUS" | json_string stream_id)"
BASE_ACKED_SEQ="$(echo "$BASE_STATUS" | json_number acked_seq)"
BASE_NEXT_SEQ="$(echo "$BASE_STATUS" | json_number next_seq)"
BASE_BACKLOG="$(echo "$BASE_STATUS" | json_number backlog_samples)"
BASE_SPOOL_BYTES="$(echo "$BASE_STATUS" | json_number spool_bytes)"

echo "[ot-harness] Baseline spool: stream_id=${BASE_STREAM_ID:-?} acked_seq=${BASE_ACKED_SEQ:-?} next_seq=${BASE_NEXT_SEQ:-?} backlog=${BASE_BACKLOG:-?} spool_bytes=${BASE_SPOOL_BYTES:-?}"

BACKUP_PATH=""

cleanup_restore_env() {
  if [[ -z "$BACKUP_PATH" ]]; then
    return 0
  fi
  echo "[ot-harness] Restoring $NODE_AGENT_ENV from $BACKUP_PATH"
  ssh_quiet "sudo cp '$BACKUP_PATH' '$NODE_AGENT_ENV' && sudo systemctl restart node-forwarder.service node-agent.service" || true
}

trap cleanup_restore_env EXIT

OFFLINE_URL="mqtt://192.0.2.1:1883"
echo "[ot-harness] Forcing offline by swapping NODE_MQTT_URL -> $OFFLINE_URL"

backup_out="$(ssh_quiet "sudo bash -s" <<EOF
set -euo pipefail
ts=\$(date +%Y%m%d%H%M%S)
backup="${NODE_AGENT_ENV}.ot49-bak-\${ts}"
cp "${NODE_AGENT_ENV}" "\${backup}"
python3 - <<'PY'
import pathlib

path = pathlib.Path("${NODE_AGENT_ENV}")
new_url = "${OFFLINE_URL}"
lines = path.read_text().splitlines()
out = []
seen = False
for line in lines:
    if line.startswith("NODE_MQTT_URL="):
        out.append("NODE_MQTT_URL=" + new_url)
        seen = True
    else:
        out.append(line)
if not seen:
    out.append("NODE_MQTT_URL=" + new_url)
path.write_text("\\n".join(out) + "\\n")
PY
systemctl restart node-forwarder.service node-agent.service
echo "\${backup}"
EOF
)"
BACKUP_PATH="$(echo "$backup_out" | tail -n 1 | tr -d '\r')"

echo "[ot-harness] Env backup: $BACKUP_PATH"

echo "[ot-harness] Waiting $OFFLINE_SECONDS seconds while offline…"
sleep "$OFFLINE_SECONDS"

OFFLINE_STATUS="$(node_forwarder_status_json)"
OFFLINE_BACKLOG="$(echo "$OFFLINE_STATUS" | json_number backlog_samples)"
OFFLINE_SPOOL_BYTES="$(echo "$OFFLINE_STATUS" | json_number spool_bytes)"
OFFLINE_NEXT_SEQ="$(echo "$OFFLINE_STATUS" | json_number next_seq)"
OFFLINE_ACKED_SEQ="$(echo "$OFFLINE_STATUS" | json_number acked_seq)"

echo "[ot-harness] Offline spool: acked_seq=${OFFLINE_ACKED_SEQ:-?} next_seq=${OFFLINE_NEXT_SEQ:-?} backlog=${OFFLINE_BACKLOG:-?} spool_bytes=${OFFLINE_SPOOL_BYTES:-?}"

echo "[ot-harness] Rebooting node mid-outage…"
ssh_quiet "sudo reboot" || true

echo "[ot-harness] Waiting for SSH to return…"
if ! wait_for_ssh "$(( $(date +%s) + 180 ))"; then
  echo "[ot-harness] Node did not come back within 180s" >&2
  exit 1
fi

POST_REBOOT_STATUS="$(node_forwarder_status_json)"
POST_REBOOT_STREAM_ID="$(echo "$POST_REBOOT_STATUS" | json_string stream_id)"
POST_REBOOT_BACKLOG="$(echo "$POST_REBOOT_STATUS" | json_number backlog_samples)"
POST_REBOOT_ACKED_SEQ="$(echo "$POST_REBOOT_STATUS" | json_number acked_seq)"
POST_REBOOT_NEXT_SEQ="$(echo "$POST_REBOOT_STATUS" | json_number next_seq)"

echo "[ot-harness] Post-reboot spool: stream_id=${POST_REBOOT_STREAM_ID:-?} acked_seq=${POST_REBOOT_ACKED_SEQ:-?} next_seq=${POST_REBOOT_NEXT_SEQ:-?} backlog=${POST_REBOOT_BACKLOG:-?}"

echo "[ot-harness] Restoring MQTT config (reconnect + replay)…"
cleanup_restore_env
BACKUP_PATH="" # prevent double-restore in trap

echo "[ot-harness] Monitoring replay drain (timeout ${REPLAY_TIMEOUT_SECONDS}s)…"
deadline="$(( $(date +%s) + REPLAY_TIMEOUT_SECONDS ))"
saw_online=0
while true; do
  status_json="$(node_forwarder_status_json)"
  backlog="$(echo "$status_json" | json_number backlog_samples)"
  acked="$(echo "$status_json" | json_number acked_seq)"
  next_seq="$(echo "$status_json" | json_number next_seq)"

  node_status="$(core_nodes_json | core_node_status "$NODE_HOST" "$NODE_MAC_ETH" "$NODE_ID")"

  if [[ "$node_status" == "online" ]]; then
    saw_online=1
  elif [[ "$saw_online" -eq 1 && -n "$node_status" ]]; then
    echo "[ot-harness] FAIL: controller node status flipped away from online during replay: $node_status" >&2
    exit 1
  fi

  echo "[ot-harness] replay: backlog=${backlog:-?} acked_seq=${acked:-?} next_seq=${next_seq:-?} controller_status=${node_status:-?}"

  if [[ "${backlog:-}" == "0" ]]; then
    break
  fi

  if [[ "$(date +%s)" -ge "$deadline" ]]; then
    echo "[ot-harness] FAIL: replay did not drain before timeout" >&2
    exit 1
  fi
  sleep 2
done

echo "[ot-harness] PASS: backlog drained and controller stayed stable during replay."
