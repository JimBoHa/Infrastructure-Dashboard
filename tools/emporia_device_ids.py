#!/usr/bin/env python3
"""Fetch Emporia deviceGid values using a Cognito authtoken."""
from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any

import httpx


DEFAULT_API_BASE = "https://api.emporiaenergy.com"
ENV_TOKEN_KEYS = (
    "EMPORIA_AUTHTOKEN",
    "CORE_ANALYTICS_EMPORIA__AUTH_TOKEN",
    "CORE_ANALYTICS_EMPORIA__API_KEY",
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Fetch Emporia deviceGid values")
    parser.add_argument(
        "--authtoken",
        help="Cognito id_token for Emporia (sent as authtoken header)",
    )
    parser.add_argument(
        "--api-base",
        default=DEFAULT_API_BASE,
        help=f"Emporia API base URL (default: {DEFAULT_API_BASE})",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit JSON payload instead of text",
    )
    return parser


def _resolve_authtoken(args: argparse.Namespace) -> str | None:
    if args.authtoken:
        return args.authtoken.strip()
    for key in ENV_TOKEN_KEYS:
        value = os.getenv(key)
        if value:
            return value.strip()
    return None


def _flatten_devices(device: dict[str, Any]) -> list[dict[str, Any]]:
    flattened = [device]
    for child in device.get("devices", []) or []:
        if isinstance(child, dict):
            flattened.extend(_flatten_devices(child))
    return flattened


def _device_name(device: dict[str, Any]) -> str | None:
    props = device.get("locationProperties") or {}
    for key in ("deviceName", "displayName"):
        if props.get(key):
            return str(props[key]).strip()
    for key in ("deviceName", "name"):
        if device.get(key):
            return str(device[key]).strip()
    return None


def fetch_devices(api_base: str, authtoken: str) -> list[dict[str, Any]]:
    url = f"{api_base.rstrip('/')}/customers/devices"
    headers = {"authtoken": authtoken}
    with httpx.Client(timeout=10) as client:
        resp = client.get(url, headers=headers)
        resp.raise_for_status()
        payload = resp.json()
    devices: list[dict[str, Any]] = []
    for device in payload.get("devices", []) or []:
        if isinstance(device, dict):
            devices.extend(_flatten_devices(device))
    return devices


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    authtoken = _resolve_authtoken(args)
    if not authtoken:
        print(
            "Missing authtoken. Pass --authtoken or set EMPORIA_AUTHTOKEN / "
            "CORE_ANALYTICS_EMPORIA__AUTH_TOKEN.",
            file=sys.stderr,
        )
        return 1

    devices = fetch_devices(args.api_base, authtoken)
    entries = []
    device_ids: list[str] = []
    for device in devices:
        device_gid = device.get("deviceGid")
        if device_gid is None:
            continue
        device_id = str(device_gid)
        device_ids.append(device_id)
        entries.append(
            {
                "device_gid": device_id,
                "name": _device_name(device),
                "model": device.get("model"),
                "firmware": device.get("firmware"),
            }
        )

    device_ids = sorted(set(device_ids))
    if args.json:
        print(json.dumps({"device_gids": device_ids, "devices": entries}, indent=2))
        return 0

    print("device_gids:", ",".join(device_ids))
    for entry in entries:
        name = entry["name"] or "unknown"
        model = entry["model"] or "unknown"
        print(f"- {entry['device_gid']}: {name} ({model})")
    return 0


if __name__ == "__main__":  # pragma: no cover - manual invocation
    raise SystemExit(main())
