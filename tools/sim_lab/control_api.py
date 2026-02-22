from __future__ import annotations

import asyncio
import json
import logging
import os
import uuid
from copy import deepcopy
from contextlib import asynccontextmanager
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Literal, Optional

import httpx
from fastapi import Depends, FastAPI, HTTPException, status
from pydantic import BaseModel, Field

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_STORAGE_DIR = ROOT / "storage" / "sim_lab"
STORAGE_DIR = Path(os.getenv("SIM_LAB_STORAGE_DIR", DEFAULT_STORAGE_DIR))
NODE_HOST = os.getenv("SIM_LAB_NODE_HOST", "127.0.0.1")
ARM_TTL_SECONDS = int(os.getenv("SIM_LAB_ARM_TTL_SECONDS", "60"))
RESTART_QUEUE_PATH = Path(os.getenv("SIM_LAB_RESTART_QUEUE", STORAGE_DIR / "restart_queue.json"))

logger = logging.getLogger(__name__)


def _utcnow() -> datetime:
    return datetime.now(timezone.utc)


def _read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


@dataclass
class SimLabNode:
    node_id: str
    node_name: str
    api_base: str
    advertise_port: int
    config_path: Path
    sensors: list[dict]
    outputs: list[dict]
    simulation: dict


class FaultSpec(BaseModel):
    id: str
    kind: Literal[
        "node_offline",
        "node_offline_cycle",
        "sensor_jitter",
        "sensor_spike",
        "stuck_output",
        "base_override",
    ]
    node_id: Optional[str] = None
    sensor_id: Optional[str] = None
    output_id: Optional[str] = None
    config: Dict[str, object] = Field(default_factory=dict)


class FaultRequest(BaseModel):
    kind: FaultSpec.__annotations__["kind"]
    node_id: Optional[str] = None
    sensor_id: Optional[str] = None
    output_id: Optional[str] = None
    config: Dict[str, object] = Field(default_factory=dict)


class ArmRequest(BaseModel):
    armed: bool
    ttl_seconds: Optional[int] = None


class SeedRequest(BaseModel):
    seed: int


class TimeMultiplierRequest(BaseModel):
    multiplier: float = Field(gt=0)


class ScenarioInfo(BaseModel):
    id: str
    label: str
    description: str


class ControlStatus(BaseModel):
    run_state: Literal["running", "paused", "stopped"]
    armed: bool
    armed_until: Optional[str] = None
    active_scenario: Optional[str] = None
    seed: Optional[int] = None
    time_multiplier: float
    fault_count: int
    nodes: list[dict]
    updated_at: str


@dataclass(frozen=True)
class ScenarioDefinition:
    id: str
    label: str
    description: str
    apply: Callable[[list[SimLabNode]], Dict[str, dict]]


def _pick_sensor(node: SimLabNode, types: Iterable[str]) -> Optional[str]:
    type_set = {value.lower() for value in types}
    for sensor in node.sensors:
        sensor_type = str(sensor.get("type") or "").lower()
        if sensor_type in type_set:
            return sensor.get("sensor_id")
    if node.sensors:
        return node.sensors[0].get("sensor_id")
    return None


def _pick_output(node: SimLabNode, keywords: Iterable[str]) -> Optional[str]:
    keyword_set = {value.lower() for value in keywords}
    for output in node.outputs:
        name = str(output.get("name") or "").lower()
        output_type = str(output.get("type") or "").lower()
        if keyword_set.intersection(name.split()) or output_type in keyword_set:
            return output.get("output_id") or output.get("id")
    if node.outputs:
        return node.outputs[0].get("output_id") or node.outputs[0].get("id")
    return None


def _scenario_baseline(nodes: list[SimLabNode]) -> Dict[str, dict]:
    return {node.node_id: {} for node in nodes}


def _scenario_storm_surge(nodes: list[SimLabNode]) -> Dict[str, dict]:
    overrides: Dict[str, dict] = {}
    for node in nodes:
        sensor_id = _pick_sensor(node, ["wind", "wind_speed", "pressure", "flow", "flow_meter"])
        if sensor_id:
            overrides[node.node_id] = {
                "jitter": {sensor_id: 0.6},
                "spikes": {sensor_id: {"every_seconds": 55, "magnitude": 4.5, "jitter": 0.4}},
            }
        else:
            overrides[node.node_id] = {}
    return overrides


def _scenario_pump_failure(nodes: list[SimLabNode]) -> Dict[str, dict]:
    overrides: Dict[str, dict] = {}
    for node in nodes:
        output_id = _pick_output(node, ["pump", "relay", "valve"])
        overrides[node.node_id] = {"stuck_outputs": [output_id]} if output_id else {}
    return overrides


def _scenario_node_dropout(nodes: list[SimLabNode]) -> Dict[str, dict]:
    overrides = {node.node_id: {} for node in nodes}
    if not nodes:
        return overrides
    target = sorted(nodes, key=lambda item: item.node_id)[-1]
    overrides[target.node_id] = {
        "offline_cycle": {"period_seconds": 90, "offline_seconds": 12, "initial_offset_seconds": 8}
    }
    return overrides


SCENARIOS: list[ScenarioDefinition] = [
    ScenarioDefinition(
        id="baseline",
        label="Baseline",
        description="Nominal telemetry and outputs with no injected anomalies.",
        apply=_scenario_baseline,
    ),
    ScenarioDefinition(
        id="storm-surge",
        label="Storm Surge",
        description="Wind/pressure jitter with periodic spikes for surge demos.",
        apply=_scenario_storm_surge,
    ),
    ScenarioDefinition(
        id="pump-failure",
        label="Pump Failure",
        description="Stuck actuator profile for irrigation pump outputs.",
        apply=_scenario_pump_failure,
    ),
    ScenarioDefinition(
        id="node-dropout",
        label="Node Dropout",
        description="One node cycles offline/online to exercise alarms.",
        apply=_scenario_node_dropout,
    ),
]


class SimLabControlState:
    def __init__(self) -> None:
        self.run_state: Literal["running", "paused", "stopped"] = "running"
        self.armed_until: Optional[datetime] = None
        self.active_scenario: Optional[str] = "baseline"
        self.seed: Optional[int] = None
        self.time_multiplier: float = 1.0
        self.faults: list[FaultSpec] = []
        self.updated_at: datetime = _utcnow()

    def is_armed(self) -> bool:
        if not self.armed_until:
            return False
        return self.armed_until > _utcnow()

    def touch(self) -> None:
        self.updated_at = _utcnow()


class SimLabControlPlane:
    def __init__(self, storage_dir: Path, node_host: str) -> None:
        self.storage_dir = storage_dir
        self.node_host = node_host
        self.state = SimLabControlState()
        self._baseline_profiles: Dict[str, dict] = {}
        self._client = httpx.AsyncClient(timeout=5.0)
        self._queue_lock = asyncio.Lock()

    async def close(self) -> None:
        await self._client.aclose()

    def _load_nodes(self) -> list[SimLabNode]:
        nodes: list[SimLabNode] = []
        if not self.storage_dir.exists():
            return nodes
        for path in sorted(self.storage_dir.glob("*.json")):
            if path.name == RESTART_QUEUE_PATH.name:
                continue
            try:
                payload = _read_json(path)
            except Exception:
                logger.warning("Skipping invalid Sim Lab config: %s", path)
                continue
            node = payload.get("node") or {}
            node_id = node.get("node_id")
            node_name = node.get("node_name") or node_id
            advertise_port = payload.get("advertise_port")
            if not node_id or not advertise_port:
                continue
            api_base = f"http://{self.node_host}:{advertise_port}"
            nodes.append(
                SimLabNode(
                    node_id=str(node_id),
                    node_name=str(node_name),
                    api_base=api_base,
                    advertise_port=int(advertise_port),
                    config_path=path,
                    sensors=list(payload.get("sensors") or []),
                    outputs=list(payload.get("outputs") or []),
                    simulation=dict(payload.get("simulation") or {}),
                )
            )
        return nodes

    def _ensure_baselines(self, nodes: list[SimLabNode]) -> None:
        for node in nodes:
            if node.node_id not in self._baseline_profiles:
                self._baseline_profiles[node.node_id] = deepcopy(node.simulation or {})

    def _scenario_overrides(self, nodes: list[SimLabNode]) -> Dict[str, dict]:
        scenario_id = self.state.active_scenario or "baseline"
        scenario = next((item for item in SCENARIOS if item.id == scenario_id), SCENARIOS[0])
        return scenario.apply(nodes)

    def _apply_faults(self, profile: dict, node: SimLabNode) -> dict:
        result = deepcopy(profile)
        for fault in self.state.faults:
            if fault.node_id and fault.node_id != node.node_id:
                continue
            if fault.sensor_id and not any(
                sensor.get("sensor_id") == fault.sensor_id for sensor in node.sensors
            ):
                continue
            if fault.output_id and not any(
                (output.get("output_id") or output.get("id")) == fault.output_id
                for output in node.outputs
            ):
                continue
            if fault.kind == "node_offline":
                result["offline"] = True
                result["offline_cycle"] = None
            elif fault.kind == "node_offline_cycle":
                result["offline_cycle"] = fault.config
            elif fault.kind == "sensor_jitter" and fault.sensor_id:
                jitter = dict(result.get("jitter") or {})
                jitter[fault.sensor_id] = float(fault.config.get("sigma", 0.5))
                result["jitter"] = jitter
            elif fault.kind == "sensor_spike" and fault.sensor_id:
                spikes = dict(result.get("spikes") or {})
                spikes[fault.sensor_id] = {
                    "every_seconds": float(fault.config.get("every_seconds", 45)),
                    "magnitude": float(fault.config.get("magnitude", 4.0)),
                    "jitter": float(fault.config.get("jitter", 0.2)),
                }
                result["spikes"] = spikes
            elif fault.kind == "stuck_output":
                stuck = set(result.get("stuck_outputs") or [])
                if fault.output_id:
                    stuck.add(fault.output_id)
                result["stuck_outputs"] = list(stuck)
            elif fault.kind == "base_override" and fault.sensor_id:
                base = dict(result.get("base_overrides") or {})
                base[fault.sensor_id] = float(fault.config.get("value", 0.0))
                result["base_overrides"] = base
        return result

    def _compose_profile(self, node: SimLabNode, scenario_overrides: dict) -> dict:
        base = deepcopy(self._baseline_profiles.get(node.node_id, node.simulation or {}))
        overrides = scenario_overrides.get(node.node_id) or {}
        merged = deepcopy(base)
        merged.update(overrides)
        merged = self._apply_faults(merged, node)
        if self.state.seed is not None:
            merged["seed"] = self.state.seed
        merged["time_multiplier"] = self.state.time_multiplier
        merged["enabled"] = True
        if self.state.run_state in {"paused", "stopped"}:
            merged["offline"] = True
            merged["offline_cycle"] = None
        return merged

    async def _queue_restart(self, node_id: str, simulation: dict, reason: str) -> None:
        entry = {
            "node_id": node_id,
            "simulation": simulation,
            "reason": reason,
            "requested_at": _utcnow().isoformat(),
        }
        async with self._queue_lock:
            self.storage_dir.mkdir(parents=True, exist_ok=True)
            if RESTART_QUEUE_PATH.exists():
                payload = _read_json(RESTART_QUEUE_PATH)
            else:
                payload = {}
            requests = list(payload.get("requests") or [])
            requests.append(entry)
            payload["requests"] = requests
            RESTART_QUEUE_PATH.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    async def _update_node(self, node: SimLabNode, profile: dict) -> dict:
        url = f"{node.api_base}/v1/simulation"
        try:
            response = await self._client.put(url, json=profile)
            if response.status_code >= 400:
                await self._queue_restart(node.node_id, profile, f"http {response.status_code}")
                return {"node_id": node.node_id, "status": "queued", "detail": response.text}
            return {"node_id": node.node_id, "status": "ok"}
        except Exception as exc:
            await self._queue_restart(node.node_id, profile, f"error: {exc}")
            return {"node_id": node.node_id, "status": "queued", "detail": str(exc)}

    async def apply_profiles(self) -> list[dict]:
        nodes = self._load_nodes()
        self._ensure_baselines(nodes)
        scenario_overrides = self._scenario_overrides(nodes)
        tasks = [
            self._update_node(node, self._compose_profile(node, scenario_overrides))
            for node in nodes
        ]
        if not tasks:
            return []
        return await asyncio.gather(*tasks)

    def status(self) -> ControlStatus:
        nodes = self._load_nodes()
        self._ensure_baselines(nodes)
        return ControlStatus(
            run_state=self.state.run_state,
            armed=self.state.is_armed(),
            armed_until=self.state.armed_until.isoformat() if self.state.armed_until else None,
            active_scenario=self.state.active_scenario,
            seed=self.state.seed,
            time_multiplier=self.state.time_multiplier,
            fault_count=len(self.state.faults),
            nodes=[
                {
                    "node_id": node.node_id,
                    "node_name": node.node_name,
                    "api_base": node.api_base,
                }
                for node in nodes
            ],
            updated_at=self.state.updated_at.isoformat(),
        )

    def list_faults(self) -> list[FaultSpec]:
        return list(self.state.faults)

    def set_armed(self, armed: bool, ttl_seconds: Optional[int]) -> None:
        if armed:
            ttl = ttl_seconds or ARM_TTL_SECONDS
            self.state.armed_until = _utcnow() + timedelta(seconds=ttl)
        else:
            self.state.armed_until = None
        self.state.touch()

    def set_run_state(self, value: Literal["running", "paused", "stopped"]) -> None:
        self.state.run_state = value
        self.state.touch()

    def set_seed(self, seed: int) -> None:
        self.state.seed = seed
        self.state.touch()

    def set_time_multiplier(self, multiplier: float) -> None:
        self.state.time_multiplier = multiplier
        self.state.touch()

    def set_scenario(self, scenario_id: str) -> None:
        if not any(item.id == scenario_id for item in SCENARIOS):
            raise HTTPException(status_code=404, detail="Scenario not found")
        self.state.active_scenario = scenario_id
        self.state.touch()

    def add_fault(self, request: FaultRequest, nodes: list[SimLabNode]) -> FaultSpec:
        node_id = request.node_id
        sensor_id = request.sensor_id
        output_id = request.output_id
        if sensor_id and not node_id:
            node_id = next(
                (
                    node.node_id
                    for node in nodes
                    if any(sensor.get("sensor_id") == sensor_id for sensor in node.sensors)
                ),
                None,
            )
        if output_id and not node_id:
            node_id = next(
                (
                    node.node_id
                    for node in nodes
                    if any(
                        (output.get("output_id") or output.get("id")) == output_id
                        for output in node.outputs
                    )
                ),
                None,
            )
        fault = FaultSpec(
            id=f"flt-{uuid.uuid4().hex[:8]}",
            kind=request.kind,
            node_id=node_id,
            sensor_id=sensor_id,
            output_id=output_id,
            config=request.config or {},
        )
        self.state.faults.append(fault)
        self.state.touch()
        return fault

    def clear_fault(self, fault_id: str) -> None:
        next_faults = [fault for fault in self.state.faults if fault.id != fault_id]
        if len(next_faults) == len(self.state.faults):
            raise HTTPException(status_code=404, detail="Fault not found")
        self.state.faults = next_faults
        self.state.touch()

    def clear_faults(self) -> None:
        self.state.faults = []
        self.state.touch()


@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.control_plane = SimLabControlPlane(STORAGE_DIR, NODE_HOST)
    try:
        yield
    finally:
        control: SimLabControlPlane = app.state.control_plane
        await control.close()


def get_control_plane() -> SimLabControlPlane:
    return app.state.control_plane


def require_armed(control: SimLabControlPlane = Depends(get_control_plane)) -> SimLabControlPlane:
    if not control.state.is_armed():
        raise HTTPException(status_code=status.HTTP_409_CONFLICT, detail="Sim Lab is not armed")
    return control


app = FastAPI(title="Sim Lab Control API", lifespan=lifespan)


@app.get("/healthz")
async def healthz() -> dict:
    return {"status": "ok"}


@app.get("/sim-lab/status", response_model=ControlStatus)
async def sim_lab_status(control: SimLabControlPlane = Depends(get_control_plane)) -> ControlStatus:
    return control.status()


@app.get("/sim-lab/scenarios", response_model=list[ScenarioInfo])
async def sim_lab_scenarios() -> list[ScenarioInfo]:
    return [ScenarioInfo(id=item.id, label=item.label, description=item.description) for item in SCENARIOS]


@app.get("/sim-lab/faults", response_model=list[FaultSpec])
async def sim_lab_faults(control: SimLabControlPlane = Depends(get_control_plane)) -> list[FaultSpec]:
    return control.list_faults()


@app.post("/sim-lab/arm")
async def sim_lab_arm(request: ArmRequest, control: SimLabControlPlane = Depends(get_control_plane)) -> dict:
    control.set_armed(request.armed, request.ttl_seconds)
    return {"armed": control.state.is_armed(), "armed_until": control.state.armed_until}


@app.post("/sim-lab/actions/start")
async def sim_lab_start(control: SimLabControlPlane = Depends(require_armed)) -> dict:
    control.set_run_state("running")
    results = await control.apply_profiles()
    return {"status": "running", "results": results}


@app.post("/sim-lab/actions/pause")
async def sim_lab_pause(control: SimLabControlPlane = Depends(require_armed)) -> dict:
    control.set_run_state("paused")
    results = await control.apply_profiles()
    return {"status": "paused", "results": results}


@app.post("/sim-lab/actions/stop")
async def sim_lab_stop(control: SimLabControlPlane = Depends(require_armed)) -> dict:
    control.set_run_state("stopped")
    results = await control.apply_profiles()
    return {"status": "stopped", "results": results}


@app.post("/sim-lab/actions/seed")
async def sim_lab_seed(request: SeedRequest, control: SimLabControlPlane = Depends(require_armed)) -> dict:
    control.set_seed(request.seed)
    results = await control.apply_profiles()
    return {"status": "ok", "results": results}


@app.post("/sim-lab/actions/time-multiplier")
async def sim_lab_time_multiplier(
    request: TimeMultiplierRequest,
    control: SimLabControlPlane = Depends(require_armed),
) -> dict:
    control.set_time_multiplier(request.multiplier)
    results = await control.apply_profiles()
    return {"status": "ok", "results": results}


@app.post("/sim-lab/actions/reset")
async def sim_lab_reset(control: SimLabControlPlane = Depends(require_armed)) -> dict:
    control.set_run_state("running")
    control.set_scenario("baseline")
    control.clear_faults()
    results = await control.apply_profiles()
    return {"status": "ok", "results": results}


@app.post("/sim-lab/scenarios/{scenario_id}/apply")
async def sim_lab_apply_scenario(
    scenario_id: str,
    control: SimLabControlPlane = Depends(require_armed),
) -> dict:
    control.set_scenario(scenario_id)
    results = await control.apply_profiles()
    return {"status": "ok", "scenario": scenario_id, "results": results}


@app.post("/sim-lab/faults/apply", response_model=FaultSpec)
async def sim_lab_apply_fault(
    request: FaultRequest,
    control: SimLabControlPlane = Depends(require_armed),
) -> FaultSpec:
    nodes = control._load_nodes()
    fault = control.add_fault(request, nodes)
    await control.apply_profiles()
    return fault


@app.post("/sim-lab/faults/{fault_id}/clear")
async def sim_lab_clear_fault(
    fault_id: str,
    control: SimLabControlPlane = Depends(require_armed),
) -> dict:
    control.clear_fault(fault_id)
    results = await control.apply_profiles()
    return {"status": "ok", "results": results}


@app.post("/sim-lab/faults/clear")
async def sim_lab_clear_faults(control: SimLabControlPlane = Depends(require_armed)) -> dict:
    control.clear_faults()
    results = await control.apply_profiles()
    return {"status": "ok", "results": results}
