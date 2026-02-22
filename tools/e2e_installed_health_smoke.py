#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
REPORTS_DIR = REPO_ROOT / "reports" / "e2e-installed-health-smoke"
LAST_SETUP_STATE = REPO_ROOT / "reports" / "e2e-setup-smoke" / "last_state.json"

DEFAULT_SETUP_CONFIG = Path("/Users/Shared/FarmDashboard/setup/config.json")


def timestamp_slug() -> str:
    return time.strftime("%Y-%m-%dT%H-%M-%SZ", time.gmtime())


def resolve_setup_config_path() -> Path:
    env_path = os.environ.get("FARM_SETUP_CONFIG")
    if env_path:
        return Path(env_path)
    state_dir = os.environ.get("FARM_SETUP_STATE_DIR")
    if state_dir:
        candidate = Path(state_dir) / "config.json"
        if candidate.exists():
            return candidate
    last_state = resolve_last_state_config()
    if last_state:
        return last_state
    return DEFAULT_SETUP_CONFIG


def resolve_last_state_config() -> Path | None:
    if not LAST_SETUP_STATE.exists():
        return None
    try:
        payload = json.loads(LAST_SETUP_STATE.read_text())
    except json.JSONDecodeError:
        return None
    if not payload.get("preserved"):
        return None
    config_path = payload.get("config_path")
    if not isinstance(config_path, str) or not config_path:
        return None
    candidate = Path(config_path)
    if candidate.exists():
        return candidate
    return None


def load_setup_config(config_path: Path) -> dict:
    if not config_path.exists():
        return {}
    try:
        return json.loads(config_path.read_text())
    except json.JSONDecodeError:
        return {}


def resolve_install_root(config: dict) -> Path | None:
    value = config.get("install_root")
    if isinstance(value, str) and value.strip():
        return Path(value)
    return None


def resolve_farmctl_binary(install_root: Path | None) -> Path | None:
    if install_root is not None:
        candidate = install_root / "bin" / "farmctl"
        if candidate.exists():
            return candidate
    repo_candidate = REPO_ROOT / "apps" / "farmctl" / "target" / "release" / "farmctl"
    if repo_candidate.exists():
        return repo_candidate
    return None


def main() -> int:
    artifacts_dir = REPORTS_DIR / timestamp_slug()
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    log_path = artifacts_dir / "health.log"

    config_path = resolve_setup_config_path()
    config = load_setup_config(config_path)
    install_root = resolve_install_root(config)
    farmctl = resolve_farmctl_binary(install_root)
    profile = str(config.get("profile", "")).strip().lower()
    if profile not in {"prod", "e2e"}:
        profile = ""

    if farmctl is None:
        log_path.write_text(
            "Missing farmctl binary.\n"
            f"- Config: {config_path}\n"
            f"- install_root: {install_root}\n"
            "Run `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke` first.\n"
        )
        print("e2e-installed-health-smoke: FAIL (farmctl binary missing)")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    cmd = [str(farmctl)]
    if profile:
        cmd.extend(["--profile", profile])
    cmd.extend(["health", "--config", str(config_path), "--json"])

    env = os.environ.copy()
    timeout_seconds = int(os.environ.get("FARM_E2E_HEALTH_TIMEOUT_SECONDS", "60"))
    deadline = time.time() + max(1, timeout_seconds)
    attempts: list[str] = []
    while True:
        result = subprocess.run(cmd, capture_output=True, text=True, env=env)
        attempts.append(
            f"$ {' '.join(cmd)}\n\nSTDOUT:\n{result.stdout}\n\nSTDERR:\n{result.stderr}\n"
        )
        if result.returncode == 0:
            break
        if time.time() >= deadline:
            log_path.write_text("\n\n---\n\n".join(attempts))
            print(f"e2e-installed-health-smoke: FAIL (exit {result.returncode})")
            print(f"Artifacts: {artifacts_dir}")
            return 1
        time.sleep(1.0)

    log_path.write_text("\n\n---\n\n".join(attempts))

    print("e2e-installed-health-smoke: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
