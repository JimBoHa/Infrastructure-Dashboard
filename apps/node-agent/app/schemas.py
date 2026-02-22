from __future__ import annotations

from typing import Dict, List, Optional

from pydantic import BaseModel, Field, field_validator

from app.config import (
    Ads1263HatSettings,
    DisplayConfig,
    OutputConfig,
    RenogyBt2Config,
    ScheduleConfig,
    SensorConfig,
)


class SensorUpdatePayload(BaseModel):
    name: Optional[str] = None
    interval_seconds: Optional[float] = None
    rolling_average_seconds: Optional[float] = None
    unit: Optional[str] = None
    location: Optional[str] = None


class OutputUpdatePayload(BaseModel):
    name: Optional[str] = None
    default_state: Optional[str] = None
    command_topic: Optional[str] = None
    supported_states: Optional[List[str]] = None


class NodeUpdatePayload(BaseModel):
    node_name: Optional[str] = None
    heartbeat_interval_seconds: Optional[float] = None
    telemetry_interval_seconds: Optional[float] = None
    capabilities: Optional[List[str]] = None


class SimulationUpdatePayload(BaseModel):
    enabled: Optional[bool] = None
    seed: Optional[int] = None
    time_multiplier: Optional[float] = None
    offline: Optional[bool] = None
    offline_cycle: Optional[Dict[str, object]] = None
    jitter: Optional[Dict[str, float]] = None
    spikes: Optional[Dict[str, Dict[str, object]]] = None
    stuck_outputs: Optional[List[str]] = None
    base_overrides: Optional[Dict[str, float]] = None
    label: Optional[str] = None
    mesh_nodes: Optional[List[Dict[str, object]]] = None


class ConfigEnvelope(BaseModel):
    node: Optional[Dict[str, object]] = None
    sensors: Optional[List[SensorConfig]] = None
    outputs: Optional[List[OutputConfig]] = None
    schedules: Optional[List[ScheduleConfig]] = None
    renogy_bt2: Optional[RenogyBt2Config] = None
    ads1263: Optional[Ads1263HatSettings] = None
    display: Optional[DisplayConfig] = None


class BluetoothProvisionRequest(BaseModel):
    device_name: str
    pin: Optional[str] = None
    wifi_ssid: Optional[str] = None
    wifi_password: Optional[str] = None
    mesh_join_code: Optional[str] = None
    preferred_protocol: Optional[str] = None  # e.g., "zigbee" or "thread"
    adoption_token: Optional[str] = None
    session_id: Optional[str] = None


class ProvisioningSessionRequest(BluetoothProvisionRequest):
    start_only: bool = False


class MeshJoinRequest(BaseModel):
    duration_seconds: int = 120


class MeshRemoveRequest(BaseModel):
    ieee: str


class RenogyIngestResponse(BaseModel):
    status: str
    metrics: List[str] = []


class RenogyRegisterWrite(BaseModel):
    address: int = Field(ge=0, le=0xFFFF)
    values: List[int] = Field(default_factory=list, min_length=1, max_length=32)
    description: Optional[str] = None

    @field_validator("values")
    @classmethod
    def _validate_values(cls, values: List[int]) -> List[int]:
        normalized: List[int] = []
        for value in values:
            if value < 0 or value > 0xFFFF:
                raise ValueError("Register values must be between 0 and 65535")
            normalized.append(int(value))
        return normalized


class RenogyApplyRequest(BaseModel):
    writes: List[RenogyRegisterWrite]
    verify: bool = True


class RenogyErrorDetail(BaseModel):
    type: str
    message: str
    code: Optional[int] = None


class RenogyApplyResult(BaseModel):
    address: int
    values: List[int]
    read_back: Optional[List[int]] = None


class RenogyApplyResponse(BaseModel):
    status: str
    applied: List[RenogyApplyResult] = Field(default_factory=list)
    error: Optional[RenogyErrorDetail] = None


class RenogySettingsReadResponse(BaseModel):
    status: str
    start_address: int
    count: int
    registers: List[int] = Field(default_factory=list)
    error: Optional[RenogyErrorDetail] = None
