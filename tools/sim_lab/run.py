from __future__ import annotations

import argparse
import asyncio
import json
import os
import socket
import signal
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Dict, Iterable, List, Optional
from urllib.parse import urlparse
import uuid

ROOT = Path(__file__).resolve().parents[2]

CORE_RS_DIR = ROOT / "apps" / "core-server-rs"
NODE_DIR = ROOT / "apps" / "node-agent"
WEB_DIR = ROOT / "apps" / "dashboard-web"
SIDECAR_DIR = ROOT / "apps" / "telemetry-sidecar"
STORAGE_DIR = ROOT / "storage" / "sim_lab"
RESTART_QUEUE_PATH = STORAGE_DIR / "restart_queue.json"


def _default_advertise_ip() -> str:
    try:
        hostname = socket.gethostname()
        ip = socket.gethostbyname(hostname)
        if ip and not ip.startswith("127."):
            return ip
        candidates = socket.gethostbyname_ex(hostname)[2]
        for candidate in candidates:
            if candidate and not candidate.startswith("127."):
                return candidate
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.connect(("8.8.8.8", 80))
            return sock.getsockname()[0]
    except OSError:
        return "127.0.0.1"


def sim_lab_node_auth_token() -> str:
    token = os.environ.get("SIM_LAB_NODE_AUTH_TOKEN", "sim-lab-token").strip()
    return token or "sim-lab-token"


sys.path.insert(0, str(NODE_DIR))
from app.config import (  # type: ignore  # noqa: E402
    OutputConfig as AgentOutputConfig,
    SensorConfig as AgentSensorConfig,
    Settings as AgentSettings,
    SimulationProfile,
)
from app.services.config_store import ConfigStore  # type: ignore  # noqa: E402

sys.path.pop(0)


def run_cmd(cmd: List[str], cwd: Path) -> None:
    print(f"[sim-lab] running {' '.join(cmd)} (cwd={cwd})")
    proc = subprocess.run(cmd, cwd=cwd)
    if proc.returncode != 0:
        raise RuntimeError(f"Command {' '.join(cmd)} failed with exit code {proc.returncode}")


@lru_cache(maxsize=None)
def poetry_python(cwd: Path) -> str:
    """Return the Poetry-managed python executable for the given project directory."""
    env = os.environ.copy()
    for key in ("POETRY_ACTIVE", "VIRTUAL_ENV", "PYTHONHOME"):
        env.pop(key, None)
    proc = subprocess.run(
        ["poetry", "run", "python", "-c", "import sys; print(sys.executable)"],
        cwd=str(cwd),
        env=env,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"poetry python lookup failed in {cwd}: {proc.stderr.strip() or proc.stdout.strip()}"
        )
    candidates = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    if not candidates:
        raise RuntimeError(f"poetry python lookup failed in {cwd}: empty output")
    return candidates[-1]


def resolve_database_url() -> str:
    for key in ("CORE_DATABASE_URL", "DATABASE_URL"):
        value = os.environ.get(key)
        if value and value.strip():
            return value.strip()
    raise RuntimeError(
        "Database URL not configured. Set CORE_DATABASE_URL (preferred) or DATABASE_URL."
    )


def run_farmctl_db_migrate(database_url: str) -> None:
    run_cmd(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(ROOT / "apps" / "farmctl" / "Cargo.toml"),
            "--",
            "db",
            "migrate",
            "--database-url",
            database_url,
            "--migrations-root",
            str(ROOT / "infra" / "migrations"),
        ],
        cwd=ROOT,
    )


def run_farmctl_db_seed_demo(database_url: str) -> None:
    run_cmd(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(ROOT / "apps" / "farmctl" / "Cargo.toml"),
            "--",
            "db",
            "seed-demo",
            "--database-url",
            database_url,
        ],
        cwd=ROOT,
    )


def wait_for_database(database_url: str, timeout_seconds: int = 60) -> None:
    normalized = database_url.replace("postgresql+psycopg", "postgresql")
    parsed = urlparse(normalized)
    host = parsed.hostname
    port = parsed.port or 5432
    if not host:
        # Best-effort: some Postgres URLs may omit the hostname (ex: unix sockets). In those
        # cases, rely on downstream tooling to surface connection errors.
        return
    deadline = time.time() + timeout_seconds
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=1):
                return
        except OSError as exc:
            last_error = exc
            time.sleep(1)
    raise RuntimeError(
        "Timed out waiting for database. "
        "Ensure native Postgres is running and CORE_DATABASE_URL is reachable. "
        f"Last error: {last_error}"
    )


def _split_fixture_url(url: str) -> tuple[str, str]:
    parsed = urlparse(url)
    if not parsed.scheme or not parsed.netloc:
        raise ValueError(f"Fixture URL must include scheme and host: {url}")
    base_url = f"{parsed.scheme}://{parsed.netloc}"
    path = parsed.path or "/"
    return base_url, path


def _split_mqtt_url(url: str) -> tuple[str, int]:
    parsed = urlparse(url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 1883
    return host, port


def _normalize_sidecar_db_url(db_url: str) -> str:
    return db_url.replace("postgresql+psycopg", "postgresql")


@dataclass
class ManagedProcess:
    name: str
    command: List[str]
    cwd: Path
    env: Dict[str, str]
    process: asyncio.subprocess.Process | None = None

    async def start(self) -> None:
        print(f"[sim-lab] starting {self.name}: {' '.join(self.command)}")
        self.process = await asyncio.create_subprocess_exec(
            *self.command,
            cwd=str(self.cwd),
            env=self.env,
        )

    async def stop(self) -> None:
        if not self.process:
            return
        if self.process.returncode is None:
            self.process.terminate()
            try:
                await asyncio.wait_for(self.process.wait(), timeout=10)
            except asyncio.TimeoutError:
                self.process.kill()
        print(f"[sim-lab] stopped {self.name}")


@dataclass
class DbNode:
    id: str
    name: str
    mac_eth: Optional[str]
    mac_wifi: Optional[str]
    config: dict
    sensors: list
    outputs: list


DEMO_NODE_SPECS = [
    {
        "id": "11111111-1111-1111-1111-111111111111",
        "name": "North Field Controller",
        "mac_eth": "40:16:7E:AA:01:01",
        "mac_wifi": "40:16:7E:AA:01:02",
        "config": {
            "hardware": "Pi 5",
            "firmware": "1.4.2",
            "mesh_role": "coordinator",
            "tags": ["controller", "north"],
            "buffer": {"pending_messages": 2},
            "retention_days": 21,
        },
    },
    {
        "id": "22222222-2222-2222-2222-222222222222",
        "name": "Irrigation Pump House",
        "mac_eth": "40:16:7E:AA:02:01",
        "mac_wifi": "40:16:7E:AA:02:02",
        "config": {
            "hardware": "Pi 5",
            "firmware": "1.4.2",
            "mesh_role": "router",
            "tags": ["pump", "irrigation"],
            "buffer": {"pending_messages": 1},
        },
    },
    {
        "id": "33333333-3333-3333-3333-333333333333",
        "name": "Greenhouse South",
        "mac_eth": "40:16:7E:AA:03:01",
        "mac_wifi": "40:16:7E:AA:03:02",
        "config": {
            "hardware": "Pi Zero 2 W",
            "firmware": "1.3.9",
            "mesh_role": "router",
            "tags": ["greenhouse"],
            "retention_days": 7,
        },
    },
]

DEMO_SENSOR_SPECS = [
    {
        "node_name": "North Field Controller",
        "sensor_id": "soil-moisture-north",
        "name": "Soil Moisture - North",
        "type": "moisture",
        "unit": "%",
        "interval_seconds": 1800,
        "rolling_avg_seconds": 600,
        "config": {"default_interval_seconds": 1800, "category": "moisture", "rolling_enabled": True},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "soil-temp-north",
        "name": "Soil Temperature",
        "type": "temperature",
        "unit": "°C",
        "interval_seconds": 1800,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 1800, "category": "temperature", "rolling_enabled": False},
    },
    {
        "node_name": "Irrigation Pump House",
        "sensor_id": "pump-load",
        "name": "Pump Load",
        "type": "power",
        "unit": "kW",
        "interval_seconds": 1,
        "rolling_avg_seconds": 60,
        "config": {"default_interval_seconds": 1, "category": "power", "rolling_enabled": True},
    },
    {
        "node_name": "Irrigation Pump House",
        "sensor_id": "water-pressure",
        "name": "Water Pressure",
        "type": "pressure",
        "unit": "psi",
        "interval_seconds": 30,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 30, "category": "pressure", "rolling_enabled": False},
    },
    {
        "node_name": "Irrigation Pump House",
        "sensor_id": "flow-meter-domestic",
        "name": "Domestic Flow",
        "type": "flow",
        "unit": "gpm",
        "interval_seconds": 0,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 0, "category": "flow", "rolling_enabled": False},
    },
    {
        "node_name": "Irrigation Pump House",
        "sensor_id": "reservoir-level",
        "name": "Reservoir Level",
        "type": "water_level",
        "unit": "in",
        "interval_seconds": 1800,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 1800, "category": "water_level", "rolling_enabled": False},
    },
    {
        "node_name": "Greenhouse South",
        "sensor_id": "greenhouse-humidity",
        "name": "Greenhouse Humidity",
        "type": "humidity",
        "unit": "%",
        "interval_seconds": 600,
        "rolling_avg_seconds": 300,
        "config": {"default_interval_seconds": 600, "category": "humidity", "rolling_enabled": True},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "wind-speed-north",
        "name": "Wind Speed",
        "type": "wind",
        "unit": "mph",
        "interval_seconds": 30,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 30, "category": "wind", "rolling_enabled": False},
    },
    {
        "node_name": "Greenhouse South",
        "sensor_id": "lux-greenhouse",
        "name": "Greenhouse Lux",
        "type": "lux",
        "unit": "lux",
        "interval_seconds": 300,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 300, "category": "light", "rolling_enabled": False},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "rain-gauge",
        "name": "Rain Gauge",
        "type": "rain",
        "unit": "mm",
        "interval_seconds": 0,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 0, "category": "rain", "rolling_enabled": False},
    },
    {
        "node_name": "Irrigation Pump House",
        "sensor_id": "fert-level",
        "name": "Fertilizer Level",
        "type": "chemical_level",
        "unit": "%",
        "interval_seconds": 1800,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 1800, "category": "level", "rolling_enabled": False},
    },
    {
        "node_name": "Irrigation Pump House",
        "sensor_id": "solar-irradiance",
        "name": "Solar Irradiance",
        "type": "solar",
        "unit": "W/m²",
        "interval_seconds": 300,
        "rolling_avg_seconds": 0,
        "config": {"default_interval_seconds": 300, "category": "solar", "rolling_enabled": False},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "renogy-pv-power",
        "name": "Renogy PV Power",
        "type": "renogy_bt2",
        "unit": "W",
        "interval_seconds": 5,
        "rolling_avg_seconds": 0,
        "config": {"metric": "pv_power_w", "default_interval_seconds": 5, "category": "power"},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "renogy-battery-soc",
        "name": "Renogy Battery SOC",
        "type": "renogy_bt2",
        "unit": "%",
        "interval_seconds": 5,
        "rolling_avg_seconds": 0,
        "config": {"metric": "battery_soc_percent", "default_interval_seconds": 5, "category": "battery"},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "renogy-load-power",
        "name": "Renogy Load Power",
        "type": "renogy_bt2",
        "unit": "W",
        "interval_seconds": 5,
        "rolling_avg_seconds": 0,
        "config": {"metric": "load_power_w", "default_interval_seconds": 5, "category": "power"},
    },
    {
        "node_name": "North Field Controller",
        "sensor_id": "renogy-runtime",
        "name": "Renogy Runtime",
        "type": "renogy_bt2",
        "unit": "hrs",
        "interval_seconds": 5,
        "rolling_avg_seconds": 0,
        "config": {"metric": "runtime_hours", "default_interval_seconds": 5, "category": "battery"},
    },
]

DEMO_OUTPUT_SPECS = [
    {
        "node_name": "Irrigation Pump House",
        "id": "out-pump-1",
        "name": "Pump 1",
        "type": "relay",
        "state": "off",
        "supported_states": ["off", "on", "auto"],
        "config": {"command_topic": "iot/pump-house/pump1/command"},
    },
    {
        "node_name": "Greenhouse South",
        "id": "out-greenhouse-fan",
        "name": "Greenhouse Fan",
        "type": "relay",
        "state": "auto",
        "supported_states": ["off", "on", "auto"],
        "config": {"command_topic": "iot/greenhouse/fan/command"},
    },
]


def build_demo_snapshot() -> List[DbNode]:
    node_map: Dict[str, DbNode] = {}
    for spec in DEMO_NODE_SPECS:
        node_map[spec["name"]] = DbNode(
            id=spec["id"],
            name=spec["name"],
            mac_eth=spec.get("mac_eth"),
            mac_wifi=spec.get("mac_wifi"),
            config=dict(spec.get("config") or {}),
            sensors=[],
            outputs=[],
        )

    for sensor in DEMO_SENSOR_SPECS:
        node = node_map.get(sensor["node_name"])
        if node:
            node.sensors.append(sensor)

    for output in DEMO_OUTPUT_SPECS:
        node = node_map.get(output["node_name"])
        if node:
            node.outputs.append(output)

    return [node_map[spec["name"]] for spec in DEMO_NODE_SPECS if spec["name"] in node_map]


def profile(label: str, seed: int, **overrides) -> dict:
    payload = {"enabled": True, "label": label, "seed": seed}
    payload.update(overrides)
    return SimulationProfile(**payload).model_dump()


def _mesh_ieee(node_id: str, index: int) -> str:
    seed = uuid.uuid5(uuid.NAMESPACE_DNS, f"sim-mesh-{node_id}-{index}").hex[:16].upper()
    return ":".join(seed[i : i + 2] for i in range(0, 16, 2))


def build_mesh_nodes(node: DbNode, *, seed: int) -> list[dict]:
    parent = _mesh_ieee(node.id, 0)
    return [
        {
            "ieee": _mesh_ieee(node.id, 1),
            "cluster": 0x0402,
            "attribute": 0x0000,
            "unit": "C",
            "base_value": 21.0 + (seed % 3),
            "amplitude": 1.8,
            "jitter": 0.15,
            "battery_percent": 72.0,
            "parent": parent,
            "depth": 1,
            "lqi": 210,
            "rssi": -43,
        },
        {
            "ieee": _mesh_ieee(node.id, 2),
            "cluster": 0x0405,
            "attribute": 0x0000,
            "unit": "%",
            "base_value": 38.0 + (seed % 5),
            "amplitude": 3.6,
            "jitter": 0.25,
            "battery_percent": 64.0,
            "parent": parent,
            "depth": 2,
            "lqi": 188,
            "rssi": -55,
        },
    ]


def build_sensor_payloads(
    agent_settings: AgentSettings,
    sensors: Iterable,
    *,
    allow_empty: bool = False,
) -> List[dict]:
    items: List[dict] = []
    for idx, sensor in enumerate(sensors):
        config = (
            sensor.get("config") if isinstance(sensor, dict) else getattr(sensor, "config", None)
        ) or {}
        sensor_id = sensor.get("sensor_id") if isinstance(sensor, dict) else getattr(sensor, "sensor_id", None)
        name = sensor.get("name") if isinstance(sensor, dict) else getattr(sensor, "name", None)
        sensor_type = sensor.get("type") if isinstance(sensor, dict) else getattr(sensor, "type", None)
        unit = sensor.get("unit") if isinstance(sensor, dict) else getattr(sensor, "unit", None)
        interval_seconds = (
            sensor.get("interval_seconds") if isinstance(sensor, dict) else getattr(sensor, "interval_seconds", 0)
        )
        rolling_avg_seconds = (
            sensor.get("rolling_avg_seconds")
            if isinstance(sensor, dict)
            else getattr(sensor, "rolling_avg_seconds", None)
        )
        items.append(
            AgentSensorConfig(
                sensor_id=sensor_id,
                name=name,
                type=sensor_type or "analog",
                channel=idx,
                unit=unit or config.get("unit") or "",
                metric=config.get("metric"),
                interval_seconds=interval_seconds,
                rolling_average_seconds=rolling_avg_seconds or 0,
                offset=config.get("offset", 0.0),
                scale=config.get("scale", 1.0),
                location=config.get("location"),
            ).model_dump()
        )
    if not items and not allow_empty:
        if agent_settings.sensors:
            stub = agent_settings.sensors[0].model_copy(
                update={"sensor_id": "sim-sensor", "name": "Sim Sensor"}
            )
            items.append(stub.model_dump())
        else:
            items.append(
                AgentSensorConfig(sensor_id="sim-sensor", name="Sim Sensor").model_dump()
            )
    return items


def build_output_payloads(agent_settings: AgentSettings, outputs: Iterable) -> List[dict]:
    items: List[dict] = []
    for idx, output in enumerate(outputs):
        config = (
            output.get("config") if isinstance(output, dict) else getattr(output, "config", None)
        ) or {}
        output_id = output.get("id") if isinstance(output, dict) else getattr(output, "id", None)
        name = output.get("name") if isinstance(output, dict) else getattr(output, "name", None)
        output_type = output.get("type") if isinstance(output, dict) else getattr(output, "type", None)
        state = output.get("state") if isinstance(output, dict) else getattr(output, "state", None)
        default_state = output.get("default_state") if isinstance(output, dict) else getattr(output, "default_state", None)
        supported_states = (
            output.get("supported_states")
            if isinstance(output, dict)
            else getattr(output, "supported_states", None)
        )
        items.append(
            AgentOutputConfig(
                output_id=output_id,
                name=name,
                type=output_type or "relay",
                channel=idx,
                state=state or default_state or "unknown",
                default_state=state or default_state or "off",
                supported_states=supported_states or [],
                command_topic=config.get("command_topic"),
            ).model_dump()
        )
    if not items:
        if agent_settings.outputs:
            stub = agent_settings.outputs[0].model_copy(
                update={"output_id": "sim-output", "name": "Sim Output"}
            )
            items.append(stub.model_dump())
        else:
            items.append(
                AgentOutputConfig(
                    output_id="sim-output",
                    name="Sim Output",
                    type="relay",
                    channel=0,
                    state="off",
                    default_state="off",
                    supported_states=["off", "on"],
                ).model_dump()
            )
    return items


def build_config_payload(
    agent_settings: AgentSettings,
    node: DbNode,
    *,
    advertise_port: int,
    telemetry_interval: float,
    heartbeat_interval: float,
    simulation_profile: dict,
    adoption_token: Optional[str] = None,
    allow_empty_sensors: bool = False,
) -> dict:
    capabilities = list(agent_settings.capabilities)
    if "simulation" not in capabilities:
        capabilities.append("simulation")
    return {
        "node": {
            "node_id": node.id,
            "node_name": node.name,
            "hardware_model": (node.config or {}).get("hardware", "Pi 5"),
            "firmware_version": (node.config or {}).get("firmware", "1.4.2"),
            "mac_eth": node.mac_eth,
            "mac_wifi": node.mac_wifi,
            "adoption_token": adoption_token,
            "heartbeat_interval_seconds": heartbeat_interval,
            "telemetry_interval_seconds": telemetry_interval,
            "capabilities": capabilities,
        },
        "wifi_hints": {},
        "sensors": build_sensor_payloads(agent_settings, node.sensors, allow_empty=allow_empty_sensors),
        "outputs": build_output_payloads(agent_settings, node.outputs),
        "schedules": [],
        "mesh": agent_settings.mesh.model_dump(),
        "mesh_summary": agent_settings.mesh_summary.model_dump(),
        "simulation": simulation_profile,
        "saved_at": None,
        "advertise_port": advertise_port,
    }


async def launch_node_agent(
    payload: dict,
    *,
    config_path: Path,
    advertise_port: int,
    advertise_ip: str,
    mqtt_url: str,
    name: str,
) -> ManagedProcess:
    config_path.parent.mkdir(parents=True, exist_ok=True)
    store = ConfigStore(config_path)
    store.save(payload)
    env = os.environ.copy()
    for key in ("POETRY_ACTIVE", "VIRTUAL_ENV", "PYTHONHOME"):
        env.pop(key, None)
    env["NODE_PROVISIONING_SECRET"] = sim_lab_node_auth_token()
    env.update(
        {
            "NODE_CONFIG_PATH": str(config_path),
            "NODE_NODE_ID": payload["node"]["node_id"],
            "NODE_NODE_NAME": payload["node"]["node_name"],
            "NODE_ADVERTISE_PORT": str(advertise_port),
            "NODE_ADVERTISE_IP": advertise_ip,
            "NODE_MQTT_URL": mqtt_url,
        }
    )
    python = poetry_python(NODE_DIR)
    proc = ManagedProcess(
        name=name,
        command=[
            python,
            "-m",
            "uvicorn",
            "app.main:app",
            "--host",
            "0.0.0.0",
            "--port",
            str(advertise_port),
        ],
        cwd=NODE_DIR,
        env=env,
    )
    await proc.start()
    return proc


async def launch_core(
    core_port: int,
    *,
    database_url: str | None = None,
    forecast_fixture_url: Optional[str] = None,
    rates_fixture_url: Optional[str] = None,
    predictive_enabled: bool = False,
    predictive_ingest_token: Optional[str] = None,
    node_agent_port: int = 9000,
) -> ManagedProcess:
    env = os.environ.copy()
    env.setdefault("CORE_DEBUG", "false")
    if database_url:
        env["CORE_DATABASE_URL"] = database_url
    env.update(
        {
            "CORE_DEMO_MODE": "false",
            "CORE_ENABLE_ANALYTICS_FEEDS": "true",
            "CORE_ENABLE_FORECAST_INGESTION": "true",
            "CORE_ENABLE_INDICATOR_GENERATION": "true",
            "CORE_MQTT_INGEST_ENABLED": "false",
            "CORE_ANALYTICS_FEED_POLL_INTERVAL_SECONDS": "60",
            "CORE_INDICATOR_POLL_INTERVAL_SECONDS": "60",
            "CORE_FORECAST_POLL_INTERVAL_SECONDS": "30",
            "CORE_SCHEDULE_POLL_INTERVAL_SECONDS": "15",
            "CORE_NODE_AGENT_PORT": str(node_agent_port),
            "CORE_FORECAST_PROVIDER": "sim",
            "CORE_FORECAST_LATITUDE": "37.7749",
            "CORE_FORECAST_LONGITUDE": "-122.4194",
            "CORE_ANALYTICS_RATES__ENABLED": "true",
            "CORE_ANALYTICS_RATES__PROVIDER": "fixed",
            "CORE_ANALYTICS_RATES__FIXED_RATE": "0.24",
        }
    )
    if predictive_enabled:
        env["CORE_PREDICTIVE_ALARMS__ENABLED"] = "true"
        env["CORE_PREDICTIVE_ALARMS__INGEST_MODE"] = "db"
        if predictive_ingest_token:
            env["CORE_PREDICTIVE_ALARMS__INGEST_TOKEN"] = predictive_ingest_token
    else:
        env["CORE_PREDICTIVE_ALARMS__ENABLED"] = "false"
    if forecast_fixture_url:
        base_url, path = _split_fixture_url(forecast_fixture_url)
        env.update(
            {
                "CORE_FORECAST_PROVIDER": "http",
                "CORE_FORECAST_API_BASE_URL": base_url,
                "CORE_FORECAST_API_PATH": path,
            }
        )
    if rates_fixture_url:
        base_url, path = _split_fixture_url(rates_fixture_url)
        env.update(
            {
                "CORE_ANALYTICS_RATES__PROVIDER": "http",
                "CORE_ANALYTICS_RATES__API_BASE_URL": base_url,
                "CORE_ANALYTICS_RATES__API_PATH": path,
            }
        )
    proc = ManagedProcess(
        name="core-server",
        command=[
            "cargo",
            "run",
            "--quiet",
            "--",
            "--host",
            "0.0.0.0",
            "--port",
            str(core_port),
        ],
        cwd=CORE_RS_DIR,
        env=env,
    )
    await proc.start()
    return proc


async def launch_sidecar(
    *,
    mqtt_url: str,
    database_url: str,
    predictive_feed_url: str | None,
    predictive_feed_token: str | None,
) -> ManagedProcess:
    mqtt_host, mqtt_port = _split_mqtt_url(mqtt_url)
    env = os.environ.copy()
    env.update(
        {
            "SIDECAR_DATABASE_URL": database_url,
            "SIDECAR_MQTT_HOST": mqtt_host,
            "SIDECAR_MQTT_PORT": str(mqtt_port),
        }
    )
    if predictive_feed_url:
        env["SIDECAR_PREDICTIVE_FEED_URL"] = predictive_feed_url
    if predictive_feed_token:
        env["SIDECAR_PREDICTIVE_FEED_TOKEN"] = predictive_feed_token

    proc = ManagedProcess(
        name="telemetry-sidecar",
        command=["cargo", "run"],
        cwd=SIDECAR_DIR,
        env=env,
    )
    await proc.start()
    return proc


async def launch_dashboard(web_port: int, api_base: str, sim_lab_api_base: str) -> ManagedProcess:
    env = os.environ.copy()
    env.update(
        {
            "FARM_CORE_API_BASE": api_base,
            "NEXT_PUBLIC_SIM_LAB_API_BASE": sim_lab_api_base,
        }
    )
    proc = ManagedProcess(
        name="dashboard-web",
        command=["npm", "run", "dev", "--", "--hostname", "0.0.0.0", "--port", str(web_port)],
        cwd=WEB_DIR,
        env=env,
    )
    await proc.start()
    return proc


async def launch_control_api(control_port: int, *, node_host: str) -> ManagedProcess:
    env = os.environ.copy()
    existing_path = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = f"{ROOT}{os.pathsep}{existing_path}" if existing_path else str(ROOT)
    env.update(
        {
            "SIM_LAB_STORAGE_DIR": str(STORAGE_DIR),
            "SIM_LAB_NODE_HOST": node_host,
            "SIM_LAB_RESTART_QUEUE": str(RESTART_QUEUE_PATH),
        }
    )
    proc = ManagedProcess(
        name="sim-lab-control",
        command=[
            sys.executable,
            "-m",
            "uvicorn",
            "tools.sim_lab.control_api:app",
            "--host",
            "0.0.0.0",
            "--port",
            str(control_port),
        ],
        cwd=ROOT,
        env=env,
    )
    await proc.start()
    return proc

def _load_restart_queue() -> list[dict]:
    if not RESTART_QUEUE_PATH.exists():
        return []
    try:
        payload = json.loads(RESTART_QUEUE_PATH.read_text())
    except json.JSONDecodeError:
        return []
    return list(payload.get("requests") or [])


def _clear_restart_queue() -> None:
    if RESTART_QUEUE_PATH.exists():
        try:
            RESTART_QUEUE_PATH.unlink()
        except OSError:
            pass


async def watch_restart_queue(
    node_registry: Dict[str, dict],
    *,
    advertise_ip: str,
    mqtt_url: str,
) -> None:
    while True:
        requests = _load_restart_queue()
        if requests:
            _clear_restart_queue()
        for request in requests:
            node_id = request.get("node_id")
            entry = node_registry.get(node_id)
            if not entry:
                continue
            store = ConfigStore(entry["config_path"])
            payload = store.load()
            if not payload:
                continue
            simulation = request.get("simulation")
            if simulation:
                payload["simulation"] = simulation
                store.save(payload)
            await entry["process"].stop()
            proc = await launch_node_agent(
                payload,
                config_path=entry["config_path"],
                advertise_port=entry["advertise_port"],
                advertise_ip=advertise_ip,
                mqtt_url=mqtt_url,
                name=entry["name"],
            )
            entry["process"] = proc
            print(f"[sim-lab] restarted {entry['name']} via control API fallback")
        await asyncio.sleep(2)


async def main() -> None:
    parser = argparse.ArgumentParser(description="Run the Sim Lab orchestration stack.")
    parser.add_argument("--core-port", type=int, default=8000)
    parser.add_argument("--web-port", type=int, default=3001)
    parser.add_argument("--control-port", type=int, default=8100)
    parser.add_argument("--mqtt-url", type=str, default="mqtt://127.0.0.1:1883")
    parser.add_argument("--advertise-ip", type=str, default=None)
    parser.add_argument("--node-port-base", type=int, default=9100)
    parser.add_argument("--heartbeat-interval", type=float, default=5.0)
    parser.add_argument("--telemetry-interval", type=float, default=5.0)
    parser.add_argument(
        "--forecast-fixture-url",
        type=str,
        default=None,
        help="Optional HTTP fixture URL for forecast ingestion (ex: http://127.0.0.1:9103/forecast.json)",
    )
    parser.add_argument(
        "--rates-fixture-url",
        type=str,
        default=None,
        help="Optional HTTP fixture URL for utility rates (ex: http://127.0.0.1:9104/rates.json)",
    )
    parser.add_argument(
        "--predictive-enabled",
        action="store_true",
        help="Enable predictive alarms during Sim Lab runs (default disabled).",
    )
    parser.add_argument(
        "--no-core",
        action="store_true",
        help="Skip launching the core server (assume it is already running).",
    )
    parser.add_argument(
        "--no-sidecar",
        action="store_true",
        help="Skip launching the telemetry sidecar (assume it is already running).",
    )
    parser.add_argument(
        "--no-web",
        action="store_true",
        help="Skip launching the dashboard web server (assume it is already running).",
    )
    parser.add_argument(
        "--no-migrations",
        action="store_true",
        help="Skip running migrations before seeding.",
    )
    parser.add_argument(
        "--no-seed",
        action="store_true",
        help="Skip seeding demo data before launching Sim Lab.",
    )
    args = parser.parse_args()

    advertise_ip = args.advertise_ip or _default_advertise_ip()

    core_database_url = None
    if not (args.no_core and args.no_sidecar and args.no_migrations and args.no_seed):
        core_database_url = resolve_database_url()
        wait_for_database(core_database_url)
        if not args.no_migrations:
            run_farmctl_db_migrate(core_database_url)
        if not args.no_seed:
            run_farmctl_db_seed_demo(core_database_url)
    sidecar_database_url = (
        _normalize_sidecar_db_url(core_database_url) if core_database_url else None
    )
    predictive_enabled_env = os.getenv("CORE_PREDICTIVE_ALARMS__ENABLED")
    if args.predictive_enabled:
        predictive_enabled = True
    elif predictive_enabled_env is not None:
        predictive_enabled = predictive_enabled_env.strip().lower() in {"1", "true", "yes"}
    else:
        predictive_enabled = False
    predictive_token = None
    if predictive_enabled:
        predictive_token = os.getenv("CORE_PREDICTIVE_ALARMS__INGEST_TOKEN") or uuid.uuid4().hex
    predictive_feed_url = (
        f"http://127.0.0.1:{args.core_port}/api/predictive/ingest"
        if predictive_enabled
        else None
    )

    base_agent_settings = AgentSettings()
    base_agent_settings.mesh.enabled = True
    snapshot = build_demo_snapshot()
    STORAGE_DIR.mkdir(parents=True, exist_ok=True)

    control_api_base = f"http://127.0.0.1:{args.control_port}"

    core_proc = None
    if not args.no_core:
        if not core_database_url:
            raise RuntimeError(
                "CORE_DATABASE_URL is required to launch core-server (set CORE_DATABASE_URL or pass --no-core)."
            )
        core_proc = await launch_core(
            args.core_port,
            database_url=core_database_url,
            forecast_fixture_url=args.forecast_fixture_url,
            rates_fixture_url=args.rates_fixture_url,
            predictive_enabled=predictive_enabled,
            predictive_ingest_token=predictive_token,
            node_agent_port=args.node_port_base,
        )
    control_proc = await launch_control_api(args.control_port, node_host=advertise_ip)
    sidecar_proc = None
    if not args.no_sidecar:
        if not sidecar_database_url:
            raise RuntimeError(
                "CORE_DATABASE_URL is required to launch telemetry-sidecar (set CORE_DATABASE_URL or pass --no-sidecar)."
            )
        sidecar_proc = await launch_sidecar(
            mqtt_url=args.mqtt_url,
            database_url=sidecar_database_url,
            predictive_feed_url=predictive_feed_url,
            predictive_feed_token=predictive_token,
        )
    web_proc = None
    if not args.no_web:
        web_proc = await launch_dashboard(
            args.web_port,
            f"http://127.0.0.1:{args.core_port}",
            control_api_base,
        )

    sim_profiles: Dict[str, dict] = {}
    if snapshot:
        sim_profiles[snapshot[0].id] = profile(
            label="north-field",
            seed=101,
            jitter={"soil-moisture-north": 0.4, "wind-speed-north": 0.8},
            spikes={
                "wind-speed-north": {"every_seconds": 70, "magnitude": 5.0, "jitter": 0.5}
            },
            mesh_nodes=build_mesh_nodes(snapshot[0], seed=101),
        )
    if len(snapshot) > 1:
        sim_profiles[snapshot[1].id] = profile(
            label="pump-house",
            seed=202,
            spikes={"pump-load": {"every_seconds": 55, "magnitude": 6.5}},
            stuck_outputs=["out-pump-1"],
            mesh_nodes=build_mesh_nodes(snapshot[1], seed=202),
        )
    if len(snapshot) > 2:
        sim_profiles[snapshot[2].id] = profile(
            label="greenhouse",
            seed=303,
            offline_cycle={
                "period_seconds": 90,
                "offline_seconds": 12,
                "initial_offset_seconds": 8,
            },
            jitter={"greenhouse-humidity": 1.2},
            mesh_nodes=build_mesh_nodes(snapshot[2], seed=303),
        )

    node_processes: List[ManagedProcess] = []
    node_registry: Dict[str, dict] = {}
    port_counter = args.node_port_base
    for node in snapshot:
        sim_profile = sim_profiles.get(
            node.id,
            profile(label="sim-default", seed=404, mesh_nodes=build_mesh_nodes(node, seed=404)),
        )
        cfg_path = STORAGE_DIR / f"{node.id}.json"
        payload = build_config_payload(
            base_agent_settings,
            node,
            advertise_port=port_counter,
            telemetry_interval=args.telemetry_interval,
            heartbeat_interval=args.heartbeat_interval,
            simulation_profile=sim_profile,
        )
        name = f"node-{node.name}"
        proc = await launch_node_agent(
            payload,
            config_path=cfg_path,
            advertise_port=port_counter,
            advertise_ip=advertise_ip,
            mqtt_url=args.mqtt_url,
            name=name,
        )
        node_processes.append(proc)
        node_registry[node.id] = {
            "node_id": node.id,
            "name": name,
            "process": proc,
            "config_path": cfg_path,
            "advertise_port": port_counter,
        }
        port_counter += 1

    stop_event = asyncio.Event()

    def _signal_handler():
        stop_event.set()

    for sig in (signal.SIGINT, signal.SIGTERM):
        try:
            asyncio.get_running_loop().add_signal_handler(sig, _signal_handler)
        except NotImplementedError:
            pass

    restart_task = asyncio.create_task(
        watch_restart_queue(
            node_registry,
            advertise_ip=advertise_ip,
            mqtt_url=args.mqtt_url,
        )
    )

    print("[sim-lab] stack is running. Press Ctrl-C to stop.")
    await stop_event.wait()
    restart_task.cancel()
    await asyncio.gather(restart_task, return_exceptions=True)

    for proc in node_processes:
        await proc.stop()
    if web_proc:
        await web_proc.stop()
    await control_proc.stop()
    if sidecar_proc:
        await sidecar_proc.stop()
    if core_proc:
        await core_proc.stop()
    print("[sim-lab] shutdown complete.")


if __name__ == "__main__":
    asyncio.run(main())
