#!/usr/bin/env python3
"""Command-line helper to pair mesh endpoints with the node agent."""
from __future__ import annotations

import argparse
import asyncio
from datetime import datetime, timezone
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict

PROJECT_ROOT = Path(__file__).resolve().parents[1] / "apps" / "node-agent"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from app.config import Settings  # noqa: E402
from app.hardware import MeshAdapter  # noqa: E402


DEFAULT_CONFIG_PATH = PROJECT_ROOT / "storage" / "node_config.json"


def _redact_mesh_config(mesh_config: Dict[str, Any], *, include_secrets: bool) -> Dict[str, Any]:
    if include_secrets:
        return dict(mesh_config)
    redacted = dict(mesh_config)
    for key in ("network_key", "tc_link_key"):
        if key in redacted and redacted[key]:
            value = str(redacted[key])
            if len(value) > 8:
                redacted[key] = f"{value[:4]}…{value[-4:]}"
            else:
                redacted[key] = "REDACTED"
    return redacted


def _write_artifact(path: Path, payload: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    tmp_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    tmp_path.replace(path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Mesh pairing utility")
    parser.add_argument(
        "--config-path",
        help="Optional override for NODE_CONFIG_PATH when running out of band",
    )
    parser.add_argument(
        "--artifact-path",
        help="Optional override for the pairing artifact output path (JSON)",
    )
    parser.add_argument(
        "--include-secrets",
        action="store_true",
        help="Include mesh network keys in the artifact output (default: redacted)",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    scan = sub.add_parser("scan", help="List mesh diagnostics and known children")
    scan.add_argument("--json", action="store_true", help="Emit JSON instead of text output")

    join = sub.add_parser("join", help="Open a join window for a mesh endpoint")
    join.add_argument("--ieee", help="IEEE address of the device if already known")
    join.add_argument("--timeout", type=int, default=120, help="How long to keep join open (seconds)")

    adopt = sub.add_parser("adopt", help="Alias for join (backwards compatible)")
    adopt.add_argument("--ieee", help="IEEE address of the device if already known")
    adopt.add_argument("--timeout", type=int, default=120, help="How long to keep join open (seconds)")

    leave = sub.add_parser("leave", help="Remove a paired mesh endpoint")
    leave.add_argument("--ieee", required=True, help="IEEE address to remove")

    remove = sub.add_parser("remove", help="Alias for leave (backwards compatible)")
    remove.add_argument("--ieee", required=True, help="IEEE address to remove")

    return parser


async def run_command(args: argparse.Namespace) -> int:
    config_path = (
        Path(args.config_path).expanduser()
        if args.config_path
        else Path(os.environ.get("NODE_CONFIG_PATH", DEFAULT_CONFIG_PATH)).expanduser()
    )
    config_path.parent.mkdir(parents=True, exist_ok=True)
    os.environ["NODE_CONFIG_PATH"] = str(config_path)

    settings = Settings()
    adapter = MeshAdapter(settings)
    if adapter.enabled:
        await adapter.start()

    artifact_path = (
        Path(args.artifact_path).expanduser()
        if args.artifact_path
        else config_path.parent / "mesh_pairing.json"
    )

    try:
        if args.command == "scan":
            payload = {
                "summary": adapter.snapshot_summary(),
                "topology": adapter.topology_snapshot(),
            }
            if args.json:
                print(json.dumps(payload, indent=2, sort_keys=True))
            else:
                print("Mesh diagnostics:")
                for key, value in payload["summary"].items():
                    print(f"  {key}: {value}")
                print("Known devices:")
                if not payload["topology"]:
                    print("  (none discovered)")
                for device in payload["topology"]:
                    diag = device.get("diagnostics") or {}
                    print(f"  {device.get('ieee')}: LQI={diag.get('lqi')} RSSI={diag.get('rssi')} Battery={diag.get('battery_percent')}")
            _write_artifact(
                artifact_path,
                {
                    "version": 1,
                    "created_at": datetime.now(timezone.utc).isoformat(),
                    "command": "scan",
                    "node_id": settings.node_id,
                    "mesh": {
                        "enabled": adapter.enabled,
                        "driver": settings.mesh.driver,
                        "protocol": settings.mesh.protocol,
                        "config": _redact_mesh_config(
                            settings.mesh.model_dump(), include_secrets=bool(args.include_secrets)
                        ),
                    },
                    "summary": payload["summary"],
                    "topology": payload["topology"],
                },
            )
            return 0
        if args.command in {"join", "adopt"}:
            success = await adapter.start_join(args.timeout)
            if success:
                print(f"Join permitted for {args.timeout}s. Power on the device to pair.")
                if args.ieee:
                    print(f"(Hint: watch for endpoint reports from {args.ieee})")
                _write_artifact(
                    artifact_path,
                    {
                        "version": 1,
                        "created_at": datetime.now(timezone.utc).isoformat(),
                        "command": "join",
                        "node_id": settings.node_id,
                        "requested": {"ieee": args.ieee, "timeout_seconds": args.timeout},
                        "mesh": {
                            "enabled": adapter.enabled,
                            "driver": settings.mesh.driver,
                            "protocol": settings.mesh.protocol,
                            "config": _redact_mesh_config(
                                settings.mesh.model_dump(), include_secrets=bool(args.include_secrets)
                            ),
                        },
                        "summary": adapter.snapshot_summary(),
                        "topology": adapter.topology_snapshot(),
                    },
                )
                return 0
            print("Failed to open join window; see logs for details", file=sys.stderr)
            return 1
        if args.command in {"leave", "remove"}:
            success = await adapter.remove_device(args.ieee)
            if success:
                print(f"Requested removal for {args.ieee}")
                _write_artifact(
                    artifact_path,
                    {
                        "version": 1,
                        "created_at": datetime.now(timezone.utc).isoformat(),
                        "command": "leave",
                        "node_id": settings.node_id,
                        "requested": {"ieee": args.ieee},
                        "mesh": {
                            "enabled": adapter.enabled,
                            "driver": settings.mesh.driver,
                            "protocol": settings.mesh.protocol,
                            "config": _redact_mesh_config(
                                settings.mesh.model_dump(), include_secrets=bool(args.include_secrets)
                            ),
                        },
                        "summary": adapter.snapshot_summary(),
                        "topology": adapter.topology_snapshot(),
                    },
                )
                return 0
            print("Failed to remove device; see logs for details", file=sys.stderr)
            return 1
    finally:
        if adapter.enabled:
            await adapter.stop()
    return 1


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return asyncio.run(run_command(args))


if __name__ == "__main__":  # pragma: no cover - manual invocation
    raise SystemExit(main())
