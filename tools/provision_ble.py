#!/usr/bin/env python3
"""Command-line helper for provisioning nodes over BLE.

This is a headless fallback to the iOS onboarding UI. It scans for nodes
advertising the FarmDashboard provisioning GATT service, connects, reads
node metadata, and writes a JSON provisioning payload. You can also hit
the HTTP `/v1/provisioning/session` endpoint when BLE is unavailable.

Requires Python <3.14 and the `bleak` package.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from dataclasses import dataclass
from typing import Any, Dict, List, Optional


SERVICE_UUID = "9F0C9A30-8B1D-4E64-9A0A-0F2ED01F9F60"
INFO_UUID = "9F0C9A31-8B1D-4E64-9A0A-0F2ED01F9F60"
PROVISION_UUID = "9F0C9A32-8B1D-4E64-9A0A-0F2ED01F9F60"
STATUS_UUID = "9F0C9A33-8B1D-4E64-9A0A-0F2ED01F9F60"


@dataclass
class ScanResult:
    address: str
    name: str
    rssi: int
    uuids: List[str]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="BLE provisioning utility")
    sub = parser.add_subparsers(dest="command", required=True)

    scan = sub.add_parser("scan", help="Scan for BLE nodes")
    scan.add_argument("--timeout", type=float, default=8.0, help="Scan duration in seconds")
    scan.add_argument("--json", action="store_true", help="Emit JSON instead of text output")

    provision = sub.add_parser("provision", help="Provision a BLE node")
    provision.add_argument("--address", required=True, help="BLE address / UUID of the target node")
    provision.add_argument("--device-name", required=True, help="Friendly node name to set")
    provision.add_argument("--ssid", required=True, help="Wi-Fi SSID")
    provision.add_argument("--password", help="Wi-Fi password (omit for open networks)")
    provision.add_argument("--adoption-token", help="Adoption token to set on the node")
    provision.add_argument("--preferred-protocol", help="Mesh protocol hint (zigbee/thread)")
    provision.add_argument("--pin", help="Optional pairing PIN")
    provision.add_argument("--mesh-join-code", help="Optional mesh join code")
    provision.add_argument("--chunk-size", type=int, default=180, help="Write chunk size bytes")
    provision.add_argument("--wait-seconds", type=float, default=3.0, help="Seconds to wait for status events")
    provision.add_argument("--http-endpoint", help="Provision over HTTP instead of BLE (e.g., http://node:9000)")
    provision.add_argument("--session-only", action="store_true", help="Create a provisioning session without applying immediately")

    return parser


async def do_scan(timeout: float) -> List[ScanResult]:
    try:
        from bleak import BleakScanner  # type: ignore
    except Exception as exc:  # pragma: no cover
        raise RuntimeError("bleak not installed or unsupported on this Python") from exc

    devices = await BleakScanner.discover(timeout=timeout)
    results: List[ScanResult] = []
    for device in devices:
        uuids = [u.lower() for u in (device.metadata.get("uuids") or [])]
        if SERVICE_UUID.lower() not in uuids:
            continue
        results.append(
            ScanResult(
                address=str(device.address),
                name=device.name or "Unnamed Node",
                rssi=int(device.rssi or 0),
                uuids=uuids,
            )
        )
    return results


async def do_provision(args: argparse.Namespace) -> int:
    payload: Dict[str, Any] = {
        "device_name": args.device_name,
        "wifi_ssid": args.ssid,
    }
    if args.password:
        payload["wifi_password"] = args.password
    if args.adoption_token:
        payload["adoption_token"] = args.adoption_token
    if args.preferred_protocol:
        payload["preferred_protocol"] = args.preferred_protocol
    if args.pin:
        payload["pin"] = args.pin
    if args.mesh_join_code:
        payload["mesh_join_code"] = args.mesh_join_code

    if args.http_endpoint:
        import urllib.request

        target = args.http_endpoint.rstrip("/") + "/v1/provisioning/session"
        payload["start_only"] = bool(args.session_only)
        req = urllib.request.Request(
            target,
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:  # nosec: B310 - controlled URL
                body = resp.read().decode("utf-8")
                print("[http]", resp.status, body)
                return 0
        except Exception as exc:
            print("HTTP provisioning failed:", exc, file=sys.stderr)
            return 2

    try:
        from bleak import BleakClient  # type: ignore
    except Exception as exc:  # pragma: no cover
        print("bleak not installed or unsupported on this Python:", exc, file=sys.stderr)
        return 2

    data = json.dumps(payload).encode("utf-8")
    chunk_size = max(20, int(args.chunk_size))

    def on_status(_: int, blob: bytearray):
        try:
            print("[status]", blob.decode("utf-8"))
        except Exception:
            print("[status]", bytes(blob))

    async with BleakClient(args.address) as client:
        if not client.is_connected:
            print("Unable to connect to BLE node", file=sys.stderr)
            return 1

        try:
            info = await client.read_gatt_char(INFO_UUID)
            print("[info]", info.decode("utf-8"))
        except Exception as exc:
            print("Warning: unable to read info characteristic:", exc, file=sys.stderr)

        try:
            await client.start_notify(STATUS_UUID, on_status)
        except Exception:
            pass

        for offset in range(0, len(data), chunk_size):
            chunk = data[offset : offset + chunk_size]
            await client.write_gatt_char(PROVISION_UUID, chunk, response=False)
            await asyncio.sleep(0.02)

        await asyncio.sleep(float(args.wait_seconds))

    return 0


async def run(args: argparse.Namespace) -> int:
    if args.command == "scan":
        results = await do_scan(args.timeout)
        if args.json:
            print(json.dumps([r.__dict__ for r in results], indent=2))
        else:
            if not results:
                print("No provisioning nodes found.")
            for result in results:
                print(f"{result.name}  {result.address}  RSSI {result.rssi}")
        return 0
    if args.command == "provision":
        return await do_provision(args)
    return 1


def main(argv: Optional[List[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return asyncio.run(run(args))


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
