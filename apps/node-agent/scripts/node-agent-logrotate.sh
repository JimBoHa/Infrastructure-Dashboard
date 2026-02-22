#!/bin/bash
set -euo pipefail

# Default locations can be overridden from /etc/node-agent.env
CONFIG_FILE="${NODE_AGENT_LOGROTATE_CONFIG:-/etc/logrotate.d/node-agent}"
STATE_FILE="${NODE_AGENT_LOGROTATE_STATE:-/var/lib/logrotate/status-node-agent}"
JOURNAL_UNIT="${NODE_AGENT_JOURNAL_UNIT:-node-agent.service}"
VACUUM_DAYS="${NODE_AGENT_JOURNAL_VACUUM_DAYS:-14}"

if command -v logrotate >/dev/null 2>&1 && [[ -f "${CONFIG_FILE}" ]]; then
  install -d -m 0755 "$(dirname "${STATE_FILE}")"
  logrotate -s "${STATE_FILE}" "${CONFIG_FILE}"
fi

if command -v journalctl >/dev/null 2>&1; then
  journalctl --unit="${JOURNAL_UNIT}" --rotate || true
  journalctl --unit="${JOURNAL_UNIT}" --vacuum-time="${VACUUM_DAYS}d" || true
fi
