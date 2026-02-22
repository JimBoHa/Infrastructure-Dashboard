#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent


CHECKS: list[dict[str, object]] = [
    {
        "label": "preconfigured-media firstboot script template",
        "path": REPO_ROOT / "tools" / "build_image.py",
        "forbid": [
            "apt-get update",
            "apt-get install",
            "/opt/node-agent/.venv",
            "python3 -m venv",
            ".venv/bin/pip",
            "pip install -r",
        ],
        "require": [
            "dpkg -i /opt/node-agent/debs/*.deb",
            "dtparam=spi=on",
        ],
    },
    {
        "label": "SSH deploy job (core-server-rs)",
        "path": REPO_ROOT
        / "apps"
        / "core-server-rs"
        / "src"
        / "services"
        / "deployments",
        "forbid": [
            "apt-get update",
            "apt-get install",
            "/opt/node-agent/.venv",
            "python3 -m venv",
            ".venv/bin/pip",
            "pip install -r",
        ],
        "require": [
            "dpkg -i /opt/node-agent/debs/*.deb",
            "dtparam=spi=on",
        ],
    },
    {
        "label": "systemd node-agent.service",
        "path": REPO_ROOT / "apps" / "node-agent" / "systemd" / "node-agent.service",
        "forbid": [
            "/opt/node-agent/.venv",
            ".venv/bin/python",
        ],
        "require": [
            "EnvironmentFile=-/etc/node-agent.env",
            "WorkingDirectory=/opt/node-agent",
            "ExecStart=/usr/local/bin/node-agent-python",
        ],
    },
    {
        "label": "systemd node-forwarder.service",
        "path": REPO_ROOT / "apps" / "node-agent" / "systemd" / "node-forwarder.service",
        "forbid": [],
        "require": [
            "EnvironmentFile=-/etc/node-agent.env",
            "WorkingDirectory=/opt/node-agent",
            "ExecStart=/usr/local/bin/node-forwarder",
        ],
    },
    {
        "label": "systemd renogy-bt.service",
        "path": REPO_ROOT / "apps" / "node-agent" / "systemd" / "renogy-bt.service",
        "forbid": [
            "/opt/node-agent/.venv",
            ".venv/bin/python",
        ],
        "require": [
            "EnvironmentFile=-/etc/node-agent.env",
            "WorkingDirectory=/opt/node-agent",
            "ExecStart=/usr/local/bin/node-agent-python",
        ],
    },
    {
        "label": "farmctl bundle overlay stages offline deps",
        "path": REPO_ROOT / "apps" / "farmctl" / "src" / "bundle_node_overlay.rs",
        "require": [
            "stage_offline_node_deps(",
            "opt/node-agent/vendor",
            "opt/node-agent/debs",
        ],
        "forbid": [],
    },
]


def main() -> int:
    failures: list[str] = []
    for check in CHECKS:
        label = str(check["label"])
        path = check["path"]
        if not isinstance(path, Path):
            failures.append(f"{label}: invalid path entry")
            continue
        if not path.exists():
            failures.append(f"{label}: missing file {path}")
            continue

        if path.is_dir():
            contents = "\n".join(
                p.read_text(encoding="utf-8", errors="replace")
                for p in sorted(path.rglob("*.rs"))
                if p.is_file()
            )
        else:
            contents = path.read_text(encoding="utf-8", errors="replace")
        for needle in check.get("forbid", []) or []:
            if str(needle) in contents:
                failures.append(f"{label}: found forbidden '{needle}' in {path}")

        for needle in check.get("require", []) or []:
            if str(needle) not in contents:
                failures.append(f"{label}: expected '{needle}' in {path}")

    if failures:
        for failure in failures:
            print(f"offline-node-smoke: {failure}", file=sys.stderr)
        return 1
    print("offline-node-smoke: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
