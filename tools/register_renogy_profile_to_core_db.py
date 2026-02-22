#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

REPO_ROOT = Path(__file__).resolve().parent.parent
PRESETS_PATH = REPO_ROOT / "shared" / "presets" / "integrations.json"
DEFAULT_SETUP_CONFIG_PATH = Path("/Users/Shared/FarmDashboard/setup/config.json")
DEFAULT_PSQL_PATH = Path("/usr/local/farm-dashboard/native/postgres/bin/psql")

MAC_RE = re.compile(r"^[0-9a-fA-F]{2}(:[0-9a-fA-F]{2}){5}$")
SENSOR_ID_RE = re.compile(r"^[0-9a-fA-F]{24}$")
IP_RE = re.compile(r"^[0-9.]+$")


def _parse_db_url(raw: str) -> dict[str, str]:
    parsed = urlparse(raw)
    if not parsed.hostname or not parsed.path:
        raise ValueError("Invalid database_url in setup config")
    user = parsed.username or "postgres"
    password = parsed.password or ""
    host = parsed.hostname
    port = str(parsed.port or 5432)
    dbname = parsed.path.lstrip("/")
    if not dbname:
        raise ValueError("database_url missing database name")
    if not password:
        raise ValueError("database_url missing password")
    return {"user": user, "password": password, "host": host, "port": port, "dbname": dbname}


def _sanitize_psql_value(value: str) -> str:
    # `psql -v` variables are substituted into the script; keep them single-line to avoid
    # surprising statement termination.
    return value.replace("\x00", "").replace("\r", " ").replace("\n", " ").strip()


def _run_psql(psql_path: Path, db: dict[str, str], sql: str, variables: dict[str, str] | None = None) -> str:
    env = os.environ.copy()
    env["PGPASSWORD"] = db["password"]
    cmd = [
        str(psql_path),
        "-X",
        "-q",
        "-v",
        "ON_ERROR_STOP=1",
        "-h",
        db["host"],
        "-p",
        db["port"],
        "-U",
        db["user"],
        "-d",
        db["dbname"],
        "-At",
    ]
    if variables:
        for key, value in variables.items():
            cmd.extend(["-v", f"{key}={_sanitize_psql_value(value)}"])
    cmd.extend(["-c", sql])
    proc = subprocess.run(cmd, env=env, capture_output=True, text=True)
    if proc.returncode != 0:
        tail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(tail or f"psql failed ({proc.returncode})")
    return (proc.stdout or "").strip()


def _load_renogy_presets() -> dict[str, dict[str, str]]:
    payload = json.loads(PRESETS_PATH.read_text(encoding="utf-8"))
    renogy = payload.get("renogy_bt2") or {}
    sensors = renogy.get("sensors") or []
    out: dict[str, dict[str, str]] = {}
    for item in sensors:
        if not isinstance(item, dict):
            continue
        metric = str(item.get("metric") or "").strip()
        if not metric:
            continue
        out[metric] = {
            "core_type": str(item.get("core_type") or "").strip(),
            "unit": str(item.get("unit") or "").strip(),
            "name": str(item.get("name") or "").strip(),
        }
    return out


def _validate_mac(mac: str | None, label: str) -> str | None:
    if not mac:
        return None
    value = mac.strip()
    if not value:
        return None
    if not MAC_RE.match(value):
        raise ValueError(f"Invalid {label}: {value}")
    return value.lower()


def _validate_sensor_id(sensor_id: str) -> str:
    value = sensor_id.strip()
    if not SENSOR_ID_RE.match(value):
        raise ValueError(f"Invalid sensor_id: {value}")
    return value.lower()


def _validate_ip(ip: str | None) -> str | None:
    if not ip:
        return None
    value = ip.strip()
    if not value:
        return None
    if not IP_RE.match(value):
        raise ValueError(f"Invalid ip: {value}")
    return value


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Register a Renogy bundle profile into the local core Postgres DB.")
    parser.add_argument(
        "--profile",
        type=Path,
        default=Path("build/renogy-pi5-node1/renogy-node-profile.json"),
        help="Path to renogy-node-profile.json",
    )
    parser.add_argument("--ip-last", default=None, help="Optional node ip_last (e.g., 10.255.8.170)")
    parser.add_argument(
        "--create-node",
        action="store_true",
        help="Create the node row if it does not already exist (dev-only; bypasses adoption).",
    )
    parser.add_argument(
        "--update-node",
        action="store_true",
        help="Update existing node name/ip_last (use sparingly; avoids overwriting node attributes from stale profiles).",
    )
    parser.add_argument(
        "--setup-config",
        type=Path,
        default=DEFAULT_SETUP_CONFIG_PATH,
        help="Path to controller setup config.json (contains database_url).",
    )
    parser.add_argument(
        "--psql",
        type=Path,
        default=DEFAULT_PSQL_PATH,
        help="Path to psql binary for the embedded Postgres.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    if not args.profile.exists():
        raise SystemExit(f"Profile not found: {args.profile}")
    if not args.setup_config.exists():
        raise SystemExit(f"Setup config not found: {args.setup_config}")
    if not args.psql.exists():
        raise SystemExit(f"psql not found: {args.psql}")

    db_cfg = json.loads(args.setup_config.read_text(encoding="utf-8"))
    db_url = db_cfg.get("database_url")
    if not isinstance(db_url, str) or not db_url.strip():
        raise SystemExit("setup config missing database_url")
    db = _parse_db_url(db_url.strip())

    profile = json.loads(args.profile.read_text(encoding="utf-8"))
    if not isinstance(profile, dict):
        raise SystemExit("Profile must be a JSON object")
    node = profile.get("node") or {}
    if not isinstance(node, dict):
        raise SystemExit("Profile node must be an object")

    node_name = str(node.get("node_name") or "").strip() or "Renogy Node"
    mac_eth = _validate_mac(node.get("mac_eth"), "mac_eth")
    mac_wifi = _validate_mac(node.get("mac_wifi"), "mac_wifi")
    if not mac_eth and not mac_wifi:
        raise SystemExit("Profile must include mac_eth or mac_wifi")

    ip_last = _validate_ip(args.ip_last)

    presets = _load_renogy_presets()
    sensors = profile.get("sensors") or []
    if not isinstance(sensors, list) or not sensors:
        raise SystemExit("Profile missing sensors list")

    lookup_sql = (
        "SELECT id::text\n"
        "FROM nodes\n"
        "WHERE (\n"
        "    NULLIF(:'mac_eth', '') IS NOT NULL\n"
        "    AND mac_eth = NULLIF(:'mac_eth', '')::macaddr\n"
        ") OR (\n"
        "    NULLIF(:'mac_wifi', '') IS NOT NULL\n"
        "    AND mac_wifi = NULLIF(:'mac_wifi', '')::macaddr\n"
        ")\n"
        "LIMIT 1"
    )
    existing_id = _run_psql(
        args.psql,
        db,
        lookup_sql,
        {
            "mac_eth": mac_eth or "",
            "mac_wifi": mac_wifi or "",
        },
    ) or None

    if existing_id:
        if args.update_node:
            update_sql = (
                "UPDATE nodes\n"
                "SET name = :'node_name',\n"
                "    ip_last = COALESCE(NULLIF(:'ip_last', '')::inet, ip_last)\n"
                "WHERE id = :'node_id'::uuid"
            )
            _run_psql(
                args.psql,
                db,
                update_sql,
                {
                    "node_id": existing_id,
                    "node_name": node_name,
                    "ip_last": ip_last or "",
                },
            )
        node_id = existing_id
    else:
        if not args.create_node:
            raise SystemExit(
                "Node not found in DB for provided MAC(s). "
                "Adopt the node via the dashboard (recommended), or re-run with --create-node for dev-only usage."
            )

        insert_sql = (
            "INSERT INTO nodes (name, mac_eth, mac_wifi, ip_last)\n"
            "VALUES (\n"
            "    :'node_name',\n"
            "    NULLIF(:'mac_eth', '')::macaddr,\n"
            "    NULLIF(:'mac_wifi', '')::macaddr,\n"
            "    NULLIF(:'ip_last', '')::inet\n"
            ")\n"
            "RETURNING id::text"
        )
        node_id = _run_psql(
            args.psql,
            db,
            insert_sql,
            {
                "node_name": node_name,
                "mac_eth": mac_eth or "",
                "mac_wifi": mac_wifi or "",
                "ip_last": ip_last or "",
            },
        ).strip()

    sensor_rows: list[dict[str, object]] = []
    for sensor in sensors:
        if not isinstance(sensor, dict):
            continue
        sensor_id = _validate_sensor_id(str(sensor.get("sensor_id") or ""))
        metric = str(sensor.get("metric") or "").strip()
        if metric not in presets:
            continue
        name = str(sensor.get("name") or "").strip() or presets.get(metric, {}).get("name") or metric or sensor_id
        unit = str(sensor.get("unit") or "").strip() or presets.get(metric, {}).get("unit") or "-"
        core_type = presets.get(metric, {}).get("core_type") or "power"

        try:
            interval_seconds = int(float(sensor.get("interval_seconds", 30)))
        except (TypeError, ValueError):
            interval_seconds = 30
        interval_seconds = max(1, interval_seconds)

        try:
            rolling_avg_seconds = int(float(sensor.get("rolling_average_seconds", 0)))
        except (TypeError, ValueError):
            rolling_avg_seconds = 0
        rolling_avg_seconds = max(0, rolling_avg_seconds)

        sensor_rows.append(
            {
                "sensor_id": sensor_id,
                "metric": metric,
                "name": name,
                "unit": unit,
                "core_type": core_type,
                "interval_seconds": interval_seconds,
                "rolling_avg_seconds": rolling_avg_seconds,
            }
        )

    if not sensor_rows:
        raise SystemExit("No allowlisted Renogy sensors found in profile; nothing to register")

    sensors_sql = (
        "BEGIN;\n"
        "WITH incoming AS (\n"
        "    SELECT * FROM jsonb_to_recordset(:'sensors_json'::jsonb) AS x(\n"
        "        sensor_id text,\n"
        "        metric text,\n"
        "        name text,\n"
        "        unit text,\n"
        "        core_type text,\n"
        "        interval_seconds int,\n"
        "        rolling_avg_seconds int\n"
        "    )\n"
        ")\n"
        "INSERT INTO sensors (sensor_id, node_id, name, type, unit, interval_seconds, rolling_avg_seconds, config)\n"
        "SELECT\n"
        "    lower(trim(sensor_id)),\n"
        "    :'node_id'::uuid,\n"
        "    trim(name),\n"
        "    trim(core_type),\n"
        "    trim(unit),\n"
        "    GREATEST(interval_seconds, 1),\n"
        "    GREATEST(rolling_avg_seconds, 0),\n"
        "    jsonb_build_object('source', 'renogy_bt2', 'metric', trim(metric))\n"
        "FROM incoming\n"
        "ON CONFLICT (sensor_id) DO UPDATE\n"
        "SET node_id = EXCLUDED.node_id,\n"
        "    name = EXCLUDED.name,\n"
        "    type = EXCLUDED.type,\n"
        "    unit = EXCLUDED.unit,\n"
        "    interval_seconds = EXCLUDED.interval_seconds,\n"
        "    rolling_avg_seconds = EXCLUDED.rolling_avg_seconds,\n"
        "    config = EXCLUDED.config,\n"
        "    deleted_at = NULL;\n"
        "COMMIT;\n"
        "SELECT count(*) FROM sensors WHERE node_id = :'node_id'::uuid AND deleted_at IS NULL"
    )
    sensor_count = _run_psql(
        args.psql,
        db,
        sensors_sql,
        {
            "node_id": node_id,
            "sensors_json": json.dumps(sensor_rows, separators=(",", ":")),
        },
    ).strip()

    print(f"node_id={node_id}")
    print(f"registered_sensors={sensor_count} (attempted={len(sensor_rows)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
