#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import secrets
import signal
import subprocess
import sys
from pathlib import Path
from typing import Any, Optional
from urllib import request
from urllib.error import HTTPError, URLError
import uuid

ROOT = Path(__file__).resolve().parents[1]
NODE_DIR = ROOT / "apps" / "node-agent"
DEFAULT_NODE_ID = "pi5-sim-01"
DEFAULT_NODE_NAME = "Pi 5 Simulator"

RENOGY_CORE_TYPES = {
    "pv_power_w": "power",
    "pv_voltage_v": "voltage",
    "pv_current_a": "current",
    "battery_soc_percent": "percentage",
    "battery_voltage_v": "voltage",
    "battery_current_a": "current",
    "battery_temp_c": "temperature",
    "controller_temp_c": "temperature",
    "load_power_w": "power",
    "load_voltage_v": "voltage",
    "load_current_a": "current",
    "runtime_hours": "runtime",
}


def _import_node_agent():
    sys.path.insert(0, str(NODE_DIR))
    try:
        from app.config import (  # type: ignore
            MeshDiagnosticsSummary,
            MeshRadioConfig,
            OutputConfig,
            RenogyBt2Config,
            SensorConfig,
            SimulationProfile,
        )
        from app.services.config_store import ConfigStore  # type: ignore
    finally:
        sys.path.pop(0)
    return (
        SensorConfig,
        OutputConfig,
        MeshRadioConfig,
        MeshDiagnosticsSummary,
        RenogyBt2Config,
        SimulationProfile,
        ConfigStore,
    )


(
    SensorConfig,
    OutputConfig,
    MeshRadioConfig,
    MeshDiagnosticsSummary,
    RenogyBt2Config,
    SimulationProfile,
    ConfigStore,
) = _import_node_agent()


def _mac_from_seed(seed: str) -> str:
    mac_int = uuid.uuid5(uuid.NAMESPACE_DNS, seed).int & 0xFFFFFFFFFFFF
    first = (mac_int >> 40) & 0xFF
    first = (first | 0x02) & 0xFE
    mac_int = (mac_int & 0x00FFFFFFFFFF) | (first << 40)
    return ":".join(f"{(mac_int >> (8 * idx)) & 0xFF:02X}" for idx in reversed(range(6)))


def _normalize_mac(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None
    cleaned = "".join(ch for ch in value.lower() if ch in "0123456789abcdef")
    if len(cleaned) != 12:
        return value.lower()
    return ":".join(cleaned[i : i + 2] for i in range(0, 12, 2))


def _dedupe_core_id(base_id: str, node_id: str, existing_ids: set[str]) -> str:
    if base_id not in existing_ids:
        return base_id
    seed = f"{node_id}:{base_id}"
    candidate = hashlib.sha1(seed.encode("utf-8")).hexdigest()[:24]
    if candidate not in existing_ids:
        return candidate
    counter = 1
    while True:
        candidate = hashlib.sha1(f"{seed}:{counter}".encode("utf-8")).hexdigest()[:24]
        if candidate not in existing_ids:
            return candidate
        counter += 1


def _http_request(
    method: str,
    url: str,
    *,
    token: Optional[str] = None,
    payload: Optional[dict[str, Any]] = None,
) -> Any:
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with request.urlopen(req, timeout=15) as resp:  # nosec: B310 - controlled URL
            body = resp.read().decode("utf-8")
    except HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="ignore")
        message = detail.strip() or exc.reason
        raise RuntimeError(f"Core request failed ({exc.code}) {method} {url}: {message}") from exc
    except URLError as exc:
        raise RuntimeError(f"Core request failed {method} {url}: {exc.reason}") from exc
    if not body:
        return {}
    return json.loads(body)


def _ensure_core_token(core_url: str, token: Optional[str]) -> str:
    if not token:
        raise RuntimeError(f"Missing --core-token for core server {core_url}")
    return token


def _core_sensor_payload(sensor: SensorConfig, node_id: str) -> dict[str, Any]:
    config: dict[str, Any] = {"simulated": True}
    core_type = sensor.type
    if sensor.metric:
        core_type = RENOGY_CORE_TYPES.get(sensor.metric, "power")
        config.update({"source": "renogy_bt2", "metric": sensor.metric})
    else:
        config.update({"source": "pi5_simulator", "channel": sensor.channel, "driver": sensor.type})
    return {
        "node_id": node_id,
        "sensor_id": sensor.sensor_id,
        "name": sensor.name,
        "type": core_type,
        "unit": sensor.unit,
        "interval_seconds": int(sensor.interval_seconds),
        "rolling_avg_seconds": int(sensor.rolling_average_seconds),
        "config": config,
    }


def _core_output_payload(output: OutputConfig, node_id: str) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "node_id": node_id,
        "id": output.output_id,
        "name": output.name,
        "type": output.type,
        "state": output.default_state or output.state,
        "supported_states": output.supported_states,
        "config": {"source": "pi5_simulator", "channel": output.channel, "simulated": True},
    }
    if output.command_topic:
        payload["command_topic"] = output.command_topic
    return payload


def _sync_core_sensors(
    *,
    core_url: str,
    token: str,
    node_id: str,
    sensors: list[SensorConfig],
) -> None:
    existing = _http_request("GET", f"{core_url}/api/sensors", token=token)
    existing_ids = {item.get("sensor_id") for item in existing if item.get("sensor_id")}
    existing_by_id = {item["sensor_id"]: item for item in existing if item.get("sensor_id")}
    for sensor in sensors:
        record = existing_by_id.get(sensor.sensor_id)
        if record and str(record.get("node_id")) == str(node_id):
            continue
        if sensor.sensor_id in existing_ids:
            sensor.sensor_id = _dedupe_core_id(sensor.sensor_id, node_id, existing_ids)
        payload = _core_sensor_payload(sensor, node_id)
        _http_request("POST", f"{core_url}/api/sensors", token=token, payload=payload)
        existing_ids.add(sensor.sensor_id)


def _sync_core_outputs(
    *,
    core_url: str,
    token: str,
    node_id: str,
    outputs: list[OutputConfig],
) -> None:
    existing = _http_request("GET", f"{core_url}/api/outputs", token=token)
    existing_ids = {item.get("id") for item in existing if item.get("id")}
    existing_by_id = {item["id"]: item for item in existing if item.get("id")}
    for output in outputs:
        record = existing_by_id.get(output.output_id)
        if record and str(record.get("node_id")) == str(node_id):
            continue
        if output.output_id in existing_ids:
            output.output_id = _dedupe_core_id(output.output_id, node_id, existing_ids)
        payload = _core_output_payload(output, node_id)
        _http_request("POST", f"{core_url}/api/outputs", token=token, payload=payload)
        existing_ids.add(output.output_id)


def _register_core_assets(
    *,
    core_url: str,
    token: str,
    node_name: str,
    mac_eth: Optional[str],
    mac_wifi: Optional[str],
    sensors: list[SensorConfig],
    outputs: list[OutputConfig],
) -> str:
    mac_eth = _normalize_mac(mac_eth)
    mac_wifi = _normalize_mac(mac_wifi)
    if not mac_eth and not mac_wifi:
        raise RuntimeError("mac_eth or mac_wifi required to register core node")
    core_url = core_url.rstrip("/")
    nodes = _http_request("GET", f"{core_url}/api/nodes", token=token)
    node_record = None
    for node in nodes:
        if mac_eth and node.get("mac_eth") == mac_eth:
            node_record = node
            break
        if mac_wifi and node.get("mac_wifi") == mac_wifi:
            node_record = node
            break

    node_id = None
    if node_record:
        node_id = node_record.get("id")
        if node_id and node_name and node_record.get("name") != node_name:
            _http_request(
                "PUT",
                f"{core_url}/api/nodes/{node_id}",
                token=token,
                payload={"name": node_name},
            )
    else:
        payload = {"name": node_name, "mac_eth": mac_eth, "mac_wifi": mac_wifi}
        created = _http_request("POST", f"{core_url}/api/nodes", token=token, payload=payload)
        node_id = created.get("id")

    if not node_id:
        raise RuntimeError("Unable to create/find core node record")

    _sync_core_sensors(core_url=core_url, token=token, node_id=node_id, sensors=sensors)
    _sync_core_outputs(core_url=core_url, token=token, node_id=node_id, outputs=outputs)
    return str(node_id)


def _issue_adoption_token(
    *,
    core_url: str,
    token: str,
    mac_eth: Optional[str],
    mac_wifi: Optional[str],
    service_name: Optional[str],
) -> str:
    if not mac_eth and not mac_wifi:
        raise RuntimeError("mac_eth or mac_wifi required to issue adoption token")
    payload: dict[str, Any] = {
        "mac_eth": mac_eth,
        "mac_wifi": mac_wifi,
        "ttl_seconds": 900,
    }
    if service_name:
        payload["service_name"] = service_name
    response = _http_request("POST", f"{core_url}/api/adoption/tokens", token=token, payload=payload)
    token_value = response.get("token")
    if not token_value:
        raise RuntimeError("Core server did not return an adoption token")
    return str(token_value)


def _base_sensors() -> list:
    return [
        SensorConfig(
            sensor_id="pi5-temp-1",
            name="Ambient Temp",
            type="temperature",
            channel=0,
            unit="C",
            interval_seconds=15.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-humidity-1",
            name="Humidity",
            type="humidity",
            channel=1,
            unit="%",
            interval_seconds=15.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-moisture-1",
            name="Soil Moisture",
            type="moisture",
            channel=2,
            unit="%",
            interval_seconds=20.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-pressure-1",
            name="Water Pressure",
            type="pressure",
            channel=3,
            unit="kPa",
            interval_seconds=10.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-flow-1",
            name="Flow Rate",
            type="flow",
            channel=4,
            unit="gpm",
            interval_seconds=5.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-irradiance-1",
            name="Solar Irradiance",
            type="irradiance",
            channel=5,
            unit="W/m2",
            interval_seconds=10.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-wind-1",
            name="Wind Speed",
            type="wind_speed",
            channel=6,
            unit="m/s",
            interval_seconds=5.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-water-level-1",
            name="Reservoir Level",
            type="water_level",
            channel=7,
            unit="cm",
            interval_seconds=20.0,
            rolling_average_seconds=0.0,
        ),
        SensorConfig(
            sensor_id="pi5-power-1",
            name="Pump Power",
            type="power",
            channel=8,
            unit="kW",
            interval_seconds=5.0,
            rolling_average_seconds=30.0,
        ),
        SensorConfig(
            sensor_id="pi5-voltage-1",
            name="Valve Voltage",
            type="voltage",
            channel=9,
            unit="V",
            interval_seconds=5.0,
            rolling_average_seconds=0.0,
        ),
    ]


def _renogy_sensors() -> list:
    metrics = [
        ("renogy-pv-power", "pv_power_w", "PV Power", "W"),
        ("renogy-pv-v", "pv_voltage_v", "PV Voltage", "V"),
        ("renogy-pv-a", "pv_current_a", "PV Current", "A"),
        ("renogy-batt-soc", "battery_soc_percent", "Battery SOC", "%"),
        ("renogy-batt-v", "battery_voltage_v", "Battery Voltage", "V"),
        ("renogy-batt-a", "battery_current_a", "Battery Current", "A"),
        ("renogy-batt-temp", "battery_temp_c", "Battery Temp", "C"),
        ("renogy-ctrl-temp", "controller_temp_c", "Controller Temp", "C"),
        ("renogy-load-power", "load_power_w", "Load Power", "W"),
        ("renogy-load-v", "load_voltage_v", "Load Voltage", "V"),
        ("renogy-load-a", "load_current_a", "Load Current", "A"),
        ("renogy-runtime", "runtime_hours", "Estimated Runtime", "h"),
    ]
    sensors = []
    for idx, (sensor_id, metric, name, unit) in enumerate(metrics):
        sensors.append(
            SensorConfig(
                sensor_id=sensor_id,
                name=name,
                type="renogy_bt2",
                metric=metric,
                channel=idx,
                unit=unit,
                interval_seconds=10.0,
                rolling_average_seconds=0.0,
            )
        )
    return sensors


def _default_outputs() -> list:
    return [
        OutputConfig(
            output_id="out-irrigation-1",
            name="Irrigation Valve",
            type="relay",
            channel=0,
            supported_states=["off", "on"],
            default_state="off",
        ),
        OutputConfig(
            output_id="out-pump-1",
            name="Pump Starter",
            type="relay",
            channel=1,
            supported_states=["off", "on"],
            default_state="off",
        ),
        OutputConfig(
            output_id="out-fan-1",
            name="Ventilation Fan",
            type="relay",
            channel=2,
            supported_states=["off", "on"],
            default_state="off",
        ),
    ]


def _build_sim_profile(seed: int | None) -> SimulationProfile:
    return SimulationProfile(
        enabled=True,
        seed=seed,
        time_multiplier=1.0,
        label="pi5-sim",
        jitter={
            "pi5-temp-1": 0.2,
            "pi5-humidity-1": 0.8,
            "pi5-moisture-1": 0.5,
            "pi5-pressure-1": 0.4,
            "pi5-flow-1": 0.3,
        },
        spikes={
            "pi5-pressure-1": {"every_seconds": 45.0, "magnitude": 5.0},
            "pi5-flow-1": {"every_seconds": 30.0, "magnitude": 2.5},
        },
    )


def _load_payload(config_path: Path) -> dict[str, Any]:
    if not config_path.exists():
        raise RuntimeError(f"Config not found: {config_path}")
    return json.loads(config_path.read_text())


def _parse_sensors(payload: dict[str, Any]) -> list[SensorConfig]:
    sensors = payload.get("sensors") or []
    return [SensorConfig.model_validate(item) for item in sensors]


def _parse_outputs(payload: dict[str, Any]) -> list[OutputConfig]:
    outputs = payload.get("outputs") or []
    return [OutputConfig.model_validate(item) for item in outputs]


def _apply_renogy_overrides(
    renogy: RenogyBt2Config,
    *,
    mode: Optional[str],
    ingest_token: Optional[str],
) -> RenogyBt2Config:
    if mode:
        if mode == "disabled":
            renogy.enabled = False
        else:
            renogy.enabled = True
            renogy.mode = mode
    if ingest_token:
        renogy.ingest_token = ingest_token
        renogy.enabled = True
    if renogy.mode == "external" and renogy.enabled and not renogy.ingest_token:
        renogy.ingest_token = secrets.token_hex(8)
    return renogy


def _build_payload(
    args: argparse.Namespace,
    *,
    node_id: str,
    adoption_token: str,
    sensors: list[SensorConfig],
    outputs: list[OutputConfig],
    simulation: SimulationProfile,
    renogy_config: RenogyBt2Config,
) -> dict:
    capabilities = []
    if simulation.enabled:
        capabilities.append("simulation")
    if renogy_config.enabled or any(sensor.metric for sensor in sensors):
        capabilities.append("renogy-bt2")

    mesh_config = MeshRadioConfig(enabled=args.mesh)
    mesh_summary = MeshDiagnosticsSummary(health="unknown", coordinator_ieee=args.mac_eth)

    return {
        "node": {
            "node_id": node_id,
            "node_name": args.node_name,
            "hardware_model": "Raspberry Pi 5",
            "firmware_version": "sim-1.0",
            "mac_eth": args.mac_eth,
            "mac_wifi": args.mac_wifi,
            "adoption_token": adoption_token,
            "heartbeat_interval_seconds": 5.0,
            "telemetry_interval_seconds": 10.0,
            "capabilities": capabilities,
        },
        "wifi_hints": {},
        "sensors": [sensor.model_dump() for sensor in sensors],
        "outputs": [output.model_dump() for output in outputs],
        "schedules": [],
        "mesh": mesh_config.model_dump(),
        "mesh_summary": mesh_summary.model_dump(),
        "renogy_bt2": renogy_config.model_dump(),
        "simulation": simulation.model_dump(),
        "saved_at": None,
    }


def _write_config(path: Path, payload: dict) -> None:
    store = ConfigStore(path)
    store.save(payload)


def _run_node_agent(
    *,
    config_path: Path,
    node_id: str,
    node_name: str,
    mqtt_url: str,
    port: int,
) -> None:
    env = os.environ.copy()
    env.update(
        {
            "NODE_CONFIG_PATH": str(config_path),
            "NODE_NODE_ID": node_id,
            "NODE_NODE_NAME": node_name,
            "NODE_MQTT_URL": mqtt_url,
            "NODE_ADVERTISE_PORT": str(port),
        }
    )
    cmd = [
        "poetry",
        "run",
        "uvicorn",
        "app.main:app",
        "--host",
        "0.0.0.0",
        "--port",
        str(port),
    ]
    print(f"[pi5-sim] starting node-agent: {' '.join(cmd)}")
    proc = subprocess.Popen(cmd, cwd=str(NODE_DIR), env=env)
    try:
        proc.wait()
    except KeyboardInterrupt:
        proc.send_signal(signal.SIGINT)
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description="Run a Raspberry Pi 5 node-agent simulator.")
    parser.add_argument("--node-id", default=DEFAULT_NODE_ID, help="Node identifier to advertise")
    parser.add_argument("--node-name", default=DEFAULT_NODE_NAME, help="Display name for the node")
    parser.add_argument("--mqtt-url", default="mqtt://127.0.0.1:1883", help="MQTT broker URL")
    parser.add_argument("--port", type=int, default=9300, help="HTTP port for the node-agent")
    parser.add_argument(
        "--adoption-token",
        default=None,
        help="Adoption token for discovery (defaults to pi5-sim-token unless --register-core issues one)",
    )
    parser.add_argument("--mac-eth", default=None, help="Override Ethernet MAC")
    parser.add_argument("--mac-wifi", default=None, help="Override Wi-Fi MAC")
    parser.add_argument("--config-dir", default=None, help="Directory to write node_config.json")
    parser.add_argument("--config-path", default=None, help="Existing node_config.json to run instead of generating")
    parser.add_argument("--seed", type=int, default=None, help="Override simulation seed")
    parser.add_argument("--no-simulation", action="store_true", help="Disable simulation profile")
    parser.add_argument("--mesh", action="store_true", help="Enable mesh adapter in config")
    parser.add_argument("--no-renogy", action="store_true", help="Disable Renogy sensor set")
    parser.add_argument(
        "--renogy-mode",
        choices=["ble", "external", "disabled"],
        help="Override Renogy collector mode (external for renogy-bt)",
    )
    parser.add_argument("--renogy-ingest-token", help="Bearer token for renogy-bt ingest")
    parser.add_argument("--core-url", help="Core server base URL (for full-stack registration)")
    parser.add_argument("--core-token", help="Core API token with config.write")
    parser.add_argument(
        "--register-core",
        action="store_true",
        help="Register node/sensors/outputs in core and use the core node UUID for MQTT topics",
    )
    parser.add_argument(
        "--write-config-only",
        action="store_true",
        help="Generate config and exit without starting node-agent",
    )
    args = parser.parse_args()

    payload: dict[str, Any] | None = None
    sensors: list[SensorConfig] = []
    outputs: list[OutputConfig] = []
    renogy_config = RenogyBt2Config()

    if args.config_path:
        payload = _load_payload(Path(args.config_path))
        node_payload = payload.get("node") or {}
        node_id = args.node_id if args.node_id != DEFAULT_NODE_ID else node_payload.get("node_id") or args.node_id
        node_name = args.node_name if args.node_name != DEFAULT_NODE_NAME else node_payload.get("node_name") or args.node_name
        adoption_token = args.adoption_token or node_payload.get("adoption_token")
        args.mac_eth = args.mac_eth or node_payload.get("mac_eth")
        args.mac_wifi = args.mac_wifi or node_payload.get("mac_wifi")
        sensors = _parse_sensors(payload)
        outputs = _parse_outputs(payload)
        renogy_config = RenogyBt2Config.model_validate(payload.get("renogy_bt2") or {})
    else:
        node_id = args.node_id
        node_name = args.node_name
        adoption_token = args.adoption_token
        sensors = _base_sensors()
        if not args.no_renogy:
            sensors.extend(_renogy_sensors())
        outputs = _default_outputs()

    if args.no_renogy:
        sensors = [sensor for sensor in sensors if sensor.type != "renogy_bt2"]
        renogy_config.enabled = False

    renogy_config = _apply_renogy_overrides(
        renogy_config,
        mode=args.renogy_mode,
        ingest_token=args.renogy_ingest_token,
    )
    if renogy_config.mode == "external" and renogy_config.enabled and renogy_config.ingest_token:
        print(f"[pi5-sim] renogy ingest token: {renogy_config.ingest_token}")

    if not args.mac_eth:
        args.mac_eth = _mac_from_seed(f"{node_id}-eth")
    if not args.mac_wifi:
        args.mac_wifi = _mac_from_seed(f"{node_id}-wifi")
    args.mac_eth = _normalize_mac(args.mac_eth)
    args.mac_wifi = _normalize_mac(args.mac_wifi)

    if args.register_core:
        if not args.core_url:
            raise RuntimeError("Provide --core-url when using --register-core")
        token = _ensure_core_token(args.core_url, args.core_token)
        node_id = _register_core_assets(
            core_url=args.core_url,
            token=token,
            node_name=node_name,
            mac_eth=args.mac_eth,
            mac_wifi=args.mac_wifi,
            sensors=sensors,
            outputs=outputs,
        )
        if not adoption_token:
            adoption_token = _issue_adoption_token(
                core_url=args.core_url.rstrip("/"),
                token=token,
                mac_eth=args.mac_eth,
                mac_wifi=args.mac_wifi,
                service_name=node_id,
            )
        print(f"[pi5-sim] core node id: {node_id}")
        print(f"[pi5-sim] adoption token: {adoption_token}")
    if not adoption_token:
        adoption_token = "pi5-sim-token"

    if payload is None:
        sim_profile = (
            SimulationProfile(enabled=False)
            if args.no_simulation
            else _build_sim_profile(args.seed)
        )
        payload = _build_payload(
            args,
            node_id=node_id,
            adoption_token=adoption_token,
            sensors=sensors,
            outputs=outputs,
            simulation=sim_profile,
            renogy_config=renogy_config,
        )
    else:
        node_payload = payload.setdefault("node", {})
        node_payload.update(
            {
                "node_id": node_id,
                "node_name": node_name,
                "mac_eth": args.mac_eth,
                "mac_wifi": args.mac_wifi,
                "adoption_token": adoption_token,
            }
        )
        payload["sensors"] = [sensor.model_dump() for sensor in sensors]
        payload["outputs"] = [output.model_dump() for output in outputs]
        payload["renogy_bt2"] = renogy_config.model_dump()
        if args.no_simulation:
            payload["simulation"] = SimulationProfile(enabled=False).model_dump()

        capabilities = list(node_payload.get("capabilities") or [])
        if renogy_config.enabled and "renogy-bt2" not in capabilities:
            capabilities.append("renogy-bt2")
        if not renogy_config.enabled and "renogy-bt2" in capabilities:
            capabilities.remove("renogy-bt2")
        if args.no_simulation and "simulation" in capabilities:
            capabilities.remove("simulation")
        node_payload["capabilities"] = capabilities

    if args.config_dir:
        config_dir = Path(args.config_dir)
    else:
        config_dir = ROOT / "storage" / "pi5_sim" / node_id
    config_dir.mkdir(parents=True, exist_ok=True)
    config_path = config_dir / "node_config.json"
    _write_config(config_path, payload)
    print(f"[pi5-sim] wrote config: {config_path}")

    if args.write_config_only:
        return 0

    _run_node_agent(
        config_path=config_path,
        node_id=node_id,
        node_name=node_name,
        mqtt_url=args.mqtt_url,
        port=args.port,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
