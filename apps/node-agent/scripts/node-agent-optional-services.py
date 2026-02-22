#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


CONFIG_PATH = Path(os.environ.get("NODE_AGENT_CONFIG_FILE", "/opt/node-agent/storage/node_config.json"))


def _load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _systemctl(*args: str) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        ["systemctl", *args],
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        if detail:
            print(f"node-agent-optional-services: systemctl {' '.join(args)} failed: {detail}")
    return proc


def _apply_renogy_bt(config: dict[str, Any]) -> None:
    renogy = config.get("renogy_bt2") or {}
    enabled = bool(renogy.get("enabled"))
    mode = str(renogy.get("mode") or "")
    address = str(renogy.get("address") or "").strip()
    device_name = str(renogy.get("device_name") or "").strip()
    ingest_token = renogy.get("ingest_token")
    has_token = isinstance(ingest_token, str) and bool(ingest_token.strip())

    should_enable = bool(enabled and mode == "external" and has_token and (address or device_name))

    if should_enable:
        print("node-agent-optional-services: enabling renogy-bt.service")
        _systemctl("enable", "--now", "renogy-bt.service")
        return

    print("node-agent-optional-services: disabling renogy-bt.service")
    _systemctl("disable", "--now", "renogy-bt.service")


def main() -> None:
    config: dict[str, Any] = _load_json(CONFIG_PATH) if CONFIG_PATH.exists() else {}

    _systemctl("daemon-reload")
    _apply_renogy_bt(config)


if __name__ == "__main__":
    main()
