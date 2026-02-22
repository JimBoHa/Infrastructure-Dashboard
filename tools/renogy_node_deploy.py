#!/usr/bin/env python3
"""Generate provisioning bundles for Renogy BT-2 charge-controller nodes."""
from __future__ import annotations

import argparse
import json
import secrets
import hashlib
import textwrap
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Optional
from urllib import request


REPO_ROOT = Path(__file__).resolve().parent.parent
PRESETS_PATH = REPO_ROOT / "shared" / "presets" / "integrations.json"


def _load_integration_presets() -> dict[str, Any]:
    payload = json.loads(PRESETS_PATH.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("integrations.json must be an object")
    return payload


def load_renogy_sensor_defs() -> list[tuple[str, str, str, str]]:
    presets = _load_integration_presets().get("renogy_bt2") or {}
    sensors = presets.get("sensors") or []
    out: list[tuple[str, str, str, str]] = []
    for sensor in sensors:
        if not isinstance(sensor, dict):
            continue
        metric = str(sensor.get("metric") or "").strip()
        name = str(sensor.get("name") or "").strip()
        core_type = str(sensor.get("core_type") or "").strip()
        unit = str(sensor.get("unit") or "").strip()
        if not metric or not name or not core_type:
            continue
        out.append((metric, name, core_type, unit))
    if not out:
        raise ValueError("No Renogy BT-2 presets found in integrations.json")
    return out


def load_renogy_default_interval_seconds() -> int:
    presets = _load_integration_presets().get("renogy_bt2") or {}
    raw = presets.get("default_interval_seconds", 30)
    try:
        value = int(raw)
    except (TypeError, ValueError):
        value = 30
    return max(1, value)


RENOGY_DEFAULT_INTERVAL_SECONDS = load_renogy_default_interval_seconds()
RENOGY_SENSOR_DEFS = load_renogy_sensor_defs()
RENOGY_CORE_TYPES = {metric: core_type for metric, _, core_type, _ in RENOGY_SENSOR_DEFS}
RENOGY_BT_DEFAULT_URL = "http://127.0.0.1:9000/v1/renogy-bt"


def _normalize_mac_hex(mac: str | None) -> str:
    if not mac:
        return "000000000000"
    cleaned = "".join(ch for ch in mac.lower() if ch in "0123456789abcdef")
    if not cleaned:
        return "000000000000"
    return cleaned.zfill(12)[-12:]


def _ensure_aware(ts: datetime | None) -> datetime:
    if ts is None:
        return datetime.now(timezone.utc)
    if ts.tzinfo is None:
        return ts.replace(tzinfo=timezone.utc)
    return ts.astimezone(timezone.utc)


def _deterministic_hex_id(
    kind: str,
    mac_eth: str | None,
    mac_wifi: str | None,
    created_at: datetime,
    counter: int,
) -> str:
    timestamp = _ensure_aware(created_at).strftime("%Y%m%d%H%M%S%f")
    payload = "|".join(
        [
            kind,
            _normalize_mac_hex(mac_eth),
            _normalize_mac_hex(mac_wifi),
            timestamp,
            f"{counter:08x}",
        ]
    )
    return hashlib.sha256(payload.encode("ascii")).hexdigest()[:24]


def _slugify(value: str) -> str:
    cleaned = "".join(ch.lower() if ch.isalnum() else "-" for ch in value)
    while "--" in cleaned:
        cleaned = cleaned.replace("--", "-")
    return cleaned.strip("-")


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2))


def _render_env(
    *,
    node_id: str,
    node_name: str,
    mqtt_url: str,
    mqtt_username: Optional[str],
    mqtt_password: Optional[str],
) -> str:
    lines = [
        f'NODE_NODE_ID="{node_id}"',
        f'NODE_NODE_NAME="{node_name}"',
        f'NODE_MQTT_URL="{mqtt_url}"',
    ]
    if mqtt_username:
        lines.append(f'NODE_MQTT_USERNAME="{mqtt_username}"')
    if mqtt_password:
        lines.append(f'NODE_MQTT_PASSWORD="{mqtt_password}"')
    return "\n".join(lines) + "\n"


def _render_renogy_bt_config(
    *,
    bt2_address: str,
    bt2_alias: str,
    adapter: str,
    device_id: int,
    poll_interval: float,
    temperature_unit: str,
    ingest_url: str,
    ingest_token: str,
) -> str:
    raise RuntimeError(
        "renogy-bt-config.ini is deprecated: renogy-bt is now shipped as an in-repo systemd service "
        "(apps/node-agent/systemd/renogy-bt.service) and uses node-agent config/env."
    )


def _render_renogy_bt_service() -> str:
    raise RuntimeError(
        "renogy-bt.service rendering is deprecated: renogy-bt is now shipped as part of the node-agent kit."
    )


def _build_sensor_configs(
    *,
    created_at: datetime,
    mac_eth: Optional[str],
    mac_wifi: Optional[str],
    interval_seconds: float,
) -> list[dict[str, Any]]:
    sensors = []
    for idx, (metric, name, _core_type, unit) in enumerate(RENOGY_SENSOR_DEFS):
        sensor_id = _deterministic_hex_id("sensor", mac_eth, mac_wifi, created_at, idx)
        sensors.append(
            {
                "sensor_id": sensor_id,
                "name": name,
                "type": "renogy_bt2",
                "unit": unit,
                "interval_seconds": interval_seconds,
                "rolling_average_seconds": 0.0,
                "metric": metric,
                "channel": 0,
            }
        )
    return sensors


def _build_node_config(
    *,
    node_id: str,
    node_name: str,
    adoption_token: str,
    capabilities: Iterable[str],
    sensors: list[dict[str, Any]],
    renogy_bt2: dict[str, Any],
    wifi_hints: Optional[dict[str, Any]] = None,
    mac_eth: Optional[str] = None,
    mac_wifi: Optional[str] = None,
) -> dict[str, Any]:
    return {
        "node": {
            "node_id": node_id,
            "node_name": node_name,
            "mac_eth": mac_eth,
            "mac_wifi": mac_wifi,
            "adoption_token": adoption_token,
            "heartbeat_interval_seconds": 5.0,
            "telemetry_interval_seconds": renogy_bt2.get("poll_interval_seconds", float(RENOGY_DEFAULT_INTERVAL_SECONDS)),
            "capabilities": list(capabilities),
        },
        "wifi_hints": wifi_hints or {},
        "sensors": sensors,
        "outputs": [],
        "schedules": [],
        "mesh": {
            "enabled": False,
            "driver": "zigpy",
            "protocol": "zigbee",
            "channel": 15,
            "pan_id": "0x1A2B",
            "extended_pan_id": "00:12:4B:00:01:A2:B3:C4",
            "network_key": "00112233445566778899AABBCCDDEEFF",
            "tc_link_key": None,
            "serial_device": None,
            "baudrate": 115200,
            "polling_interval_seconds": 5.0,
            "diagnostics_interval_seconds": 60.0,
            "max_backfill_seconds": 180.0,
        },
        "mesh_summary": {
            "health": "unknown",
            "node_count": 0,
            "coordinator_ieee": mac_eth,
            "average_link_quality": None,
            "last_rssi": None,
            "last_battery_percent": None,
            "last_parent": None,
            "last_updated": None,
            "health_details": {},
        },
        "renogy_bt2": renogy_bt2,
        "saved_at": None,
    }


def _http_request(
    method: str,
    url: str,
    *,
    token: str,
    payload: Optional[dict[str, Any]] = None,
) -> dict[str, Any]:
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", f"Bearer {token}")
    with request.urlopen(req, timeout=15) as resp:  # nosec: B310 - controlled URL
        body = resp.read().decode("utf-8")
        return json.loads(body) if body else {}


def _ensure_core_token(core_url: str, token: Optional[str]) -> str:
    if not token:
        raise RuntimeError(f"Missing --core-token for core server {core_url}")
    return token


def _register_core_assets(
    *,
    core_url: str,
    token: str,
    node_name: str,
    mac_eth: Optional[str],
    mac_wifi: Optional[str],
    sensors: list[dict[str, Any]],
) -> str:
    nodes = _http_request("GET", f"{core_url}/api/nodes", token=token)
    node_id = None
    for node in nodes:
        if mac_eth and node.get("mac_eth") == mac_eth:
            node_id = node.get("id")
            break
        if mac_wifi and node.get("mac_wifi") == mac_wifi:
            node_id = node.get("id")
            break

    if not node_id:
        payload = {"name": node_name, "mac_eth": mac_eth, "mac_wifi": mac_wifi}
        created = _http_request("POST", f"{core_url}/api/nodes", token=token, payload=payload)
        node_id = created.get("id")

    if not node_id:
        raise RuntimeError("Unable to create/find core node record")

    existing_sensors = _http_request("GET", f"{core_url}/api/sensors", token=token)
    existing_ids = {item.get("sensor_id") for item in existing_sensors if item.get("node_id") == node_id}

    for sensor in sensors:
        if sensor["sensor_id"] in existing_ids:
            continue
        payload = {
            "node_id": node_id,
            "sensor_id": sensor["sensor_id"],
            "name": sensor["name"],
            "type": RENOGY_CORE_TYPES.get(sensor["metric"], "power"),
            "unit": sensor["unit"],
            "interval_seconds": int(sensor["interval_seconds"]),
            "rolling_avg_seconds": int(sensor.get("rolling_average_seconds", 0)),
            "config": {"source": "renogy_bt2", "metric": sensor["metric"]},
        }
        _http_request("POST", f"{core_url}/api/sensors", token=token, payload=payload)

    return node_id


def _issue_adoption_token(
    *,
    core_url: str,
    token: str,
    mac_eth: Optional[str],
    mac_wifi: Optional[str],
) -> str:
    if not mac_eth and not mac_wifi:
        raise RuntimeError("mac_eth or mac_wifi required to issue adoption token")
    payload = {"mac_eth": mac_eth, "mac_wifi": mac_wifi, "ttl_seconds": 900}
    response = _http_request("POST", f"{core_url}/api/adoption/tokens", token=token, payload=payload)
    token_value = response.get("token")
    if not token_value:
        raise RuntimeError("Core server did not return an adoption token")
    return str(token_value)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Renogy BT-2 node provisioning helper")
    sub = parser.add_subparsers(dest="command", required=True)

    bundle = sub.add_parser("bundle", help="Generate a Renogy provisioning bundle")
    bundle.add_argument("--node-name", required=True)
    bundle.add_argument("--node-id")
    bundle.add_argument("--mac-eth")
    bundle.add_argument("--mac-wifi")
    bundle.add_argument("--collector", choices=["ble", "renogy-bt"], default="renogy-bt")
    bundle.add_argument("--bt2-address")
    bundle.add_argument("--bt2-name")
    bundle.add_argument("--bt2-alias")
    bundle.add_argument("--bt2-adapter", default="hci0")
    bundle.add_argument("--unit-id", type=int, default=255)
    bundle.add_argument("--poll-interval", type=float, default=float(RENOGY_DEFAULT_INTERVAL_SECONDS))
    bundle.add_argument("--battery-capacity-ah", type=int)
    bundle.add_argument("--temperature-unit", choices=["C", "F"], default="C")
    bundle.add_argument("--service-uuid")
    bundle.add_argument("--write-uuid")
    bundle.add_argument("--notify-uuid")
    bundle.add_argument("--mqtt-url", default="mqtt://core.local:1883")
    bundle.add_argument("--mqtt-username")
    bundle.add_argument("--mqtt-password")
    bundle.add_argument("--adoption-token")
    bundle.add_argument("--ingest-token")
    bundle.add_argument(
        "--renogy-bt-url",
        default=RENOGY_BT_DEFAULT_URL,
        help="Deprecated: renogy-bt is shipped as a local systemd service and always POSTs to the local node-agent ingest endpoint.",
    )
    bundle.add_argument("--wifi-ssid")
    bundle.add_argument("--wifi-password")
    bundle.add_argument("--output", type=Path, default=Path("build/renogy-node"))
    bundle.add_argument("--core-url")
    bundle.add_argument("--core-token")
    bundle.add_argument("--register-core", action="store_true")

    return parser


def run_bundle(args: argparse.Namespace) -> int:
    output_dir: Path = args.output
    output_dir.mkdir(parents=True, exist_ok=True)

    node_name = args.node_name.strip()
    node_id = args.node_id or f"renogy-{_slugify(node_name)}"
    created_at = datetime.now(timezone.utc)

    collector = args.collector
    ingest_token = args.ingest_token
    if collector == "renogy-bt" and not ingest_token:
        ingest_token = secrets.token_hex(8)

    renogy_bt2 = {
        "enabled": True,
        "mode": "external" if collector == "renogy-bt" else "ble",
        "address": args.bt2_address,
        "device_name": args.bt2_name,
        "unit_id": args.unit_id,
        "poll_interval_seconds": args.poll_interval,
        "request_timeout_seconds": 4.0,
        "connect_timeout_seconds": 10.0,
        "adapter": args.bt2_adapter,
        "service_uuid": args.service_uuid,
        "write_uuid": args.write_uuid,
        "notify_uuid": args.notify_uuid,
        "ingest_token": ingest_token,
        "battery_capacity_ah": args.battery_capacity_ah,
    }

    if not renogy_bt2["address"] and not renogy_bt2["device_name"]:
        raise RuntimeError("Provide --bt2-address or --bt2-name for the Renogy BT-2 module")
    if collector == "renogy-bt" and not renogy_bt2["address"]:
        raise RuntimeError("renogy-bt requires --bt2-address for BLE matching")

    sensors = _build_sensor_configs(
        created_at=created_at,
        mac_eth=args.mac_eth,
        mac_wifi=args.mac_wifi,
        interval_seconds=args.poll_interval,
    )
    capabilities = ["sensors", "backups", "bluetooth-provisioning", "renogy-bt2"]

    wifi_hints = None
    if args.wifi_ssid:
        wifi_hints = {"ssid": args.wifi_ssid, "password": args.wifi_password}

    adoption_token = args.adoption_token or secrets.token_hex(4)
    core_node_id = None
    if args.core_url and args.register_core:
        token = _ensure_core_token(args.core_url, args.core_token)
        if not args.adoption_token:
            adoption_token = _issue_adoption_token(
                core_url=args.core_url,
                token=token,
                mac_eth=args.mac_eth,
                mac_wifi=args.mac_wifi,
            )
        core_node_id = _register_core_assets(
            core_url=args.core_url,
            token=token,
            node_name=node_name,
            mac_eth=args.mac_eth,
            mac_wifi=args.mac_wifi,
            sensors=sensors,
        )

    node_config = _build_node_config(
        node_id=node_id,
        node_name=node_name,
        adoption_token=adoption_token,
        capabilities=capabilities,
        sensors=sensors,
        renogy_bt2=renogy_bt2,
        wifi_hints=wifi_hints,
        mac_eth=args.mac_eth,
        mac_wifi=args.mac_wifi,
    )
    _write_json(output_dir / "node_config.json", node_config)

    # Generic node stack: renogy-bt is shipped as a baseline systemd unit and is enabled
    # on first boot when `renogy_bt2.enabled=true` and `renogy_bt2.mode=external` in node_config.json.

    firstboot = {
        "node": {
            "node_id": node_id,
            "node_name": node_name,
            "adoption_token": adoption_token,
        },
        "wifi": {
            "ssid": args.wifi_ssid,
            "password": args.wifi_password,
        },
    }
    _write_json(output_dir / "node-agent-firstboot.json", firstboot)

    env_text = _render_env(
        node_id=node_id,
        node_name=node_name,
        mqtt_url=args.mqtt_url,
        mqtt_username=args.mqtt_username,
        mqtt_password=args.mqtt_password,
    )
    (output_dir / "node-agent.env").write_text(env_text)

    profile = {
        "generated_at": created_at.isoformat(),
        "node": {
            "node_id": node_id,
            "node_name": node_name,
            "mac_eth": args.mac_eth,
            "mac_wifi": args.mac_wifi,
            "adoption_token": adoption_token,
        },
        "renogy_bt2": renogy_bt2,
        "renogy_bt": {
            "collector": collector,
            "ingest_url": RENOGY_BT_DEFAULT_URL,
            "ingest_token": ingest_token,
            "bt2_alias": args.bt2_alias,
            "bt2_adapter": args.bt2_adapter,
            "temperature_unit": args.temperature_unit,
            "battery_capacity_ah": args.battery_capacity_ah,
        },
        "mqtt": {
            "url": args.mqtt_url,
            "username": args.mqtt_username,
        },
        "sensors": sensors,
    }

    if core_node_id:
        profile["core"] = {"url": args.core_url, "node_id": core_node_id}

    _write_json(output_dir / "renogy-node-profile.json", profile)

    print(f"Wrote Renogy bundle to {output_dir}")
    return 0


def main(argv: Optional[list[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command == "bundle":
        return run_bundle(args)
    return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
