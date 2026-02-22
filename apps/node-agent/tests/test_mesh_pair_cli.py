from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


def test_mesh_pair_cli_writes_artifacts(tmp_path: Path) -> None:
    repo_root = Path(__file__).resolve().parents[3]
    tool_path = repo_root / "tools" / "mesh_pair.py"
    assert tool_path.exists()

    config_path = tmp_path / "node_config.json"

    base_env = os.environ.copy()
    base_env.update(
        {
            "NODE_ADVERTISE_IP": "127.0.0.1",
            "NODE_MESH": json.dumps({"enabled": True}),
            "NODE_NODE_ID": "mesh-pair-cli-test",
        }
    )

    join_artifact = tmp_path / "join_artifact.json"
    join = subprocess.run(
        [
            sys.executable,
            str(tool_path),
            "--config-path",
            str(config_path),
            "--artifact-path",
            str(join_artifact),
            "join",
            "--timeout",
            "1",
        ],
        env=base_env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert join.returncode == 0, join.stderr
    payload = json.loads(join_artifact.read_text())
    assert payload["command"] == "join"
    assert payload["node_id"] == "mesh-pair-cli-test"
    assert payload["mesh"]["config"]["network_key"] == "0011…EEFF"

    leave_artifact = tmp_path / "leave_artifact.json"
    leave = subprocess.run(
        [
            sys.executable,
            str(tool_path),
            "--config-path",
            str(config_path),
            "--artifact-path",
            str(leave_artifact),
            "leave",
            "--ieee",
            "0011223344556677",
        ],
        env=base_env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert leave.returncode == 0, leave.stderr
    payload = json.loads(leave_artifact.read_text())
    assert payload["command"] == "leave"
    assert payload["requested"]["ieee"] == "0011223344556677"
