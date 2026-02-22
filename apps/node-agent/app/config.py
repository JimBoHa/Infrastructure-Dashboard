"""Runtime configuration for the node agent."""
from __future__ import annotations

import json
import logging
from datetime import datetime, timezone
from functools import lru_cache
from pathlib import Path
import uuid
from typing import Any, Dict, List, Optional, Literal
from urllib.parse import urlparse

from pydantic import BaseModel, Field, SecretStr, field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

from app import build_info

MIN_INTERVAL_SECONDS = 1.0
MAX_INTERVAL_SECONDS = 3600.0


def _clamp_interval_seconds(value: float, *, field: str) -> float:
    try:
        parsed = float(value)
    except Exception as exc:
        raise ValueError(f"{field} must be a number") from exc
    if parsed != parsed:  # NaN
        raise ValueError(f"{field} must be a real number")
    return max(MIN_INTERVAL_SECONDS, min(parsed, MAX_INTERVAL_SECONDS))


def _format_mac(value: int | str | None) -> Optional[str]:
    if value is None:
        return None
    if isinstance(value, int):
        mac = f"{value:012x}"
        return ":".join(mac[i : i + 2] for i in range(0, 12, 2)).upper()
    cleaned = str(value).replace("-", ":").upper()
    if ":" in cleaned:
        parts = cleaned.split(":")
        if len(parts) == 6:
            return ":".join(part.zfill(2) for part in parts)
    if len(cleaned) == 12:
        return ":".join(cleaned[i : i + 2] for i in range(0, 12, 2))
    return cleaned


class SensorConfig(BaseModel):
    sensor_id: str
    name: str = Field(default="Sensor", description="Display name")
    type: str = Field(default="analog", description="Driver type, e.g. analog or pulse")
    channel: int = 0
    negative_channel: Optional[int] = Field(
        default=None,
        description="Optional negative input channel for differential analog sensors",
    )
    unit: str = "V"
    location: Optional[str] = None
    metric: Optional[str] = Field(default=None, description="Metric key for composite sources (e.g. Renogy BT-2)")
    interval_seconds: float = 30.0
    rolling_average_seconds: float = 0.0
    input_min: Optional[float] = Field(default=None, description="Input minimum (e.g. 0V or 4mA)")
    input_max: Optional[float] = Field(default=None, description="Input maximum (e.g. 10V or 20mA)")
    output_min: Optional[float] = Field(default=None, description="Engineering units minimum (e.g. 0 psi)")
    output_max: Optional[float] = Field(default=None, description="Engineering units maximum (e.g. 300 psi)")
    offset: float = Field(default=0.0, description="Additive offset applied after scaling")
    scale: float = Field(default=1.0, description="Multiplicative factor applied after offset")
    pulses_per_unit: Optional[float] = Field(
        default=None,
        description="For pulse sensors, pulses required to equal one engineering unit",
    )
    current_loop_shunt_ohms: Optional[float] = Field(
        default=None,
        description="4-20mA current loop: shunt resistor value in ohms (enables current-loop conversion)",
    )
    current_loop_range_m: Optional[float] = Field(
        default=None,
        description="4-20mA current loop: engineering range in meters (sensor span, e.g. 5.0 for a 0-5m transducer)",
    )
    current_loop_zero_ma: float = Field(default=4.0, description="4-20mA current loop: zero current (mA)")
    current_loop_span_ma: float = Field(default=16.0, description="4-20mA current loop: span current (mA)")
    current_loop_fault_low_ma: float = Field(
        default=3.5, description="4-20mA current loop: low fault threshold (mA)"
    )
    current_loop_fault_high_ma: float = Field(
        default=21.0, description="4-20mA current loop: high fault threshold (mA)"
    )


class OutputConfig(BaseModel):
    output_id: str
    name: str = "Output"
    type: str = "relay"
    channel: int = 0
    state: str = "unknown"
    supported_states: List[str] = Field(default_factory=lambda: ["off", "on"])
    default_state: str = "off"
    command_topic: Optional[str] = None


class ScheduleConfig(BaseModel):
    schedule_id: str
    name: str
    rrule: str
    blocks: List[Dict[str, str]] = Field(default_factory=list)
    conditions: List[Dict[str, str]] = Field(default_factory=list)
    actions: List[Dict[str, str]] = Field(default_factory=list)


class MeshRadioConfig(BaseModel):
    enabled: bool = Field(default=False, description="Enable the mesh radio adapter")
    driver: str = Field(default="zigpy", description="Radio stack implementation (zigpy/thread)")
    protocol: str = Field(default="zigbee", description="Mesh protocol identifier")
    channel: int = Field(default=15, ge=0, description="Mesh radio channel number")
    pan_id: str = Field(default="0x1A2B", description="Personal Area Network identifier")
    extended_pan_id: str = Field(
        default="00:12:4B:00:01:A2:B3:C4",
        description="64-bit extended PAN identifier",
    )
    network_key: str = Field(
        default="00112233445566778899AABBCCDDEEFF",
        description="Network encryption key as 32 hex characters",
    )
    tc_link_key: Optional[str] = Field(
        default=None,
        description="Optional trust-center link key for pre-shared installs",
    )
    serial_device: Optional[str] = Field(default=None, description="Serial device path for the coordinator")
    baudrate: int = Field(default=115200, description="Coordinator serial baud rate")
    polling_interval_seconds: float = Field(default=5.0, ge=0.5, description="How often to poll mesh state")
    diagnostics_interval_seconds: float = Field(
        default=60.0,
        ge=10.0,
        description="How often to snapshot diagnostics for discovery metadata",
    )
    max_backfill_seconds: float = Field(
        default=180.0,
        ge=0.0,
        description="Maximum age (seconds) for mesh samples to be backfilled through telemetry",
    )

    @model_validator(mode="after")
    def _normalize_identifiers(self):
        self.pan_id = self._norm_short_identifier(self.pan_id, 4)
        self.extended_pan_id = self._norm_long_identifier(self.extended_pan_id)
        self.network_key = self._norm_network_key(self.network_key)
        if self.tc_link_key:
            self.tc_link_key = self._norm_network_key(self.tc_link_key)
        return self

    @staticmethod
    def _norm_short_identifier(value: str, expected_len: int) -> str:
        cleaned = value.lower().replace("0x", "")
        if len(cleaned) != expected_len or any(c not in "0123456789abcdef" for c in cleaned):
            raise ValueError(f"Identifier must be {expected_len} hex characters, got {value!r}")
        return "0x" + cleaned.upper()

    @staticmethod
    def _norm_long_identifier(value: str) -> str:
        cleaned = value.replace("-", ":").upper()
        parts = cleaned.split(":")
        if len(parts) not in {1, 8}:
            raise ValueError(f"Extended PAN ID must be 16 hex characters (optionally colon separated), got {value!r}")
        if len(parts) == 1:
            clean = parts[0]
            if len(clean) != 16:
                raise ValueError(f"Extended PAN ID must be 16 hex characters, got {value!r}")
            return ":".join(clean[i : i + 2].upper() for i in range(0, 16, 2))
        for part in parts:
            if len(part) != 2 or any(c not in "0123456789ABCDEF" for c in part):
                raise ValueError(f"Extended PAN ID must be 16 hex characters, got {value!r}")
        return ":".join(part.upper() for part in parts)

    @staticmethod
    def _norm_network_key(value: str) -> str:
        cleaned = value.replace(" ", "").replace("0x", "").upper()
        if len(cleaned) != 32 or any(c not in "0123456789ABCDEF" for c in cleaned):
            raise ValueError("Network key must be 16 bytes (32 hex characters)")
        return cleaned


class RenogyBt2Config(BaseModel):
    """Configuration for Renogy BT-2 BLE polling."""

    enabled: bool = Field(default=False, description="Enable Renogy BT-2 telemetry polling")
    mode: Literal["ble", "external"] = Field(
        default="ble",
        description="Collector mode: BLE polling or external ingest (renogy-bt)",
    )
    address: Optional[str] = Field(default=None, description="BLE MAC/address for BT-2 module")
    device_name: Optional[str] = Field(default=None, description="Optional BLE name to scan for")
    unit_id: int = Field(default=1, ge=1, le=255, description="Modbus unit id")
    poll_interval_seconds: float = Field(default=10.0, ge=1.0, description="Poll cadence in seconds")
    request_timeout_seconds: float = Field(default=4.0, ge=1.0, description="Modbus request timeout")
    connect_timeout_seconds: float = Field(default=10.0, ge=1.0, description="BLE connect/scan timeout")
    adapter: Optional[str] = Field(default=None, description="BlueZ adapter name (e.g. hci0)")
    service_uuid: Optional[str] = Field(default=None, description="Override BLE service UUID")
    write_uuid: Optional[str] = Field(default=None, description="Override BLE write characteristic UUID")
    notify_uuid: Optional[str] = Field(default=None, description="Override BLE notify characteristic UUID")
    ingest_token: Optional[str] = Field(
        default=None,
        description="Bearer token required for external Renogy ingest",
    )
    battery_capacity_ah: Optional[int] = Field(
        default=None,
        ge=0,
        description="Battery capacity for runtime estimates when using external ingest",
    )


class Ads1263HatSettings(BaseModel):
    """Configuration for the Waveshare High-Precision AD HAT (ADS1263)."""

    enabled: bool = Field(default=False, description="Enable ADS1263 HAT support")
    spi_bus: int = Field(default=0, ge=0, description="SPI bus number (usually 0)")
    spi_device: int = Field(default=0, ge=0, description="SPI device (usually 0)")
    spi_mode: int = Field(default=0b01, description="SPI mode (ADS1263 uses mode 1)")
    spi_speed_hz: int = Field(default=2_000_000, ge=100_000, description="SPI clock speed in Hz")
    rst_bcm: int = Field(default=18, ge=0, description="BCM GPIO for RESET")
    cs_bcm: int = Field(default=22, ge=0, description="BCM GPIO for chip select")
    drdy_bcm: int = Field(default=17, ge=0, description="BCM GPIO for DRDY (active-low)")
    vref_volts: float = Field(default=5.0, gt=0, description="Reference voltage in volts")
    gain: int = Field(default=1, ge=1, le=64, description="ADC gain")
    data_rate: str = Field(default="ADS1263_100SPS", description="ADS1263 data rate key (e.g. ADS1263_100SPS)")
    scan_interval_seconds: float = Field(default=0.25, gt=0, description="Background scan cadence")


class DisplayTile(BaseModel):
    """A single tile rendered by the optional local display UI."""

    type: Literal["core_status", "latency", "sensor", "sensors", "trends", "outputs"]
    sensor_id: Optional[str] = None


class DisplayTrendConfig(BaseModel):
    sensor_id: str
    default_range: Literal["1h", "6h", "24h"] = "6h"


class DisplayConfig(BaseModel):
    """Local display configuration (optional, Pi 5 only)."""

    schema_version: int = Field(
        default=1,
        ge=1,
        description="Schema version for this config payload (used for forwards-compatible syncing).",
    )
    enabled: bool = Field(default=False, description="Enable the local kiosk display UI")
    kiosk_autostart: bool = Field(
        default=False,
        description="If true, the node image may launch a kiosk browser at boot (implementation is optional).",
    )
    ui_refresh_seconds: int = Field(default=2, ge=1, le=60, description="UI refresh cadence")
    latency_sample_seconds: int = Field(default=10, ge=1, le=300, description="Latency probe cadence")
    latency_window_samples: int = Field(default=12, ge=3, le=120, description="Latency/jitter window size")
    tiles: List[DisplayTile] = Field(default_factory=list, description="Ordered tile layout")

    outputs_enabled: bool = Field(default=False, description="Enable output control page (advanced)")
    local_pin_hash: Optional[str] = Field(
        default=None,
        description="Optional SHA-256 hex digest for a local display PIN used to confirm output commands.",
    )
    trend_ranges: List[Literal["1h", "6h", "24h"]] = Field(
        default_factory=lambda: ["1h", "6h", "24h"],
        description="Allowed trend ranges",
    )
    trends: List[DisplayTrendConfig] = Field(default_factory=list, description="Trend sensors to show")
    core_api_base_url: Optional[str] = Field(
        default=None,
        description="Optional core-server base URL override (defaults to http://<mqtt_host>:8000).",
    )


class MeshDiagnosticsSummary(BaseModel):
    coordinator_ieee: Optional[str] = None
    node_count: int = 0
    last_parent: Optional[str] = None
    last_battery_percent: Optional[float] = None
    last_rssi: Optional[int] = None
    average_link_quality: Optional[float] = None
    health_details: Dict[str, Any] = Field(default_factory=dict)
    last_updated: Optional[datetime] = None
    health: str = "unknown"

    def as_properties(self) -> Dict[str, str]:
        props: Dict[str, str] = {"mesh_health": self.health}
        if self.node_count:
            props["mesh_nodes"] = str(self.node_count)
        if self.last_parent:
            props["mesh_parent"] = self.last_parent
        if self.last_battery_percent is not None:
            props["mesh_battery_percent"] = f"{self.last_battery_percent:.1f}"
        if self.average_link_quality is not None:
            props["mesh_lqi"] = f"{self.average_link_quality:.1f}"
        if self.last_rssi is not None:
            props["mesh_rssi"] = str(self.last_rssi)
        if self.coordinator_ieee:
            props["mesh_coordinator"] = self.coordinator_ieee
        if self.last_updated:
            props["mesh_updated"] = self.last_updated.replace(tzinfo=timezone.utc).isoformat()
        for key, value in self.health_details.items():
            props[f"mesh_{key}"] = str(value)
        return props


class OfflineCycleConfig(BaseModel):
    period_seconds: float = Field(default=60.0, gt=0)
    offline_seconds: float = Field(default=8.0, ge=0)
    initial_offset_seconds: float = Field(default=0.0, ge=0)


class SpikeConfig(BaseModel):
    every_seconds: float = Field(default=45.0, gt=0)
    magnitude: float = Field(default=5.0)
    jitter: float = Field(default=0.2)


class MeshSimNode(BaseModel):
    """Deterministic mesh node payload used in Sim Lab."""

    ieee: str
    endpoint: int = 1
    cluster: int = 0x0402
    attribute: int = 0x0000
    unit: Optional[str] = None
    base_value: float = 20.0
    amplitude: float = 2.0
    jitter: float = 0.2
    battery_percent: Optional[float] = None
    parent: Optional[str] = None
    depth: Optional[int] = None
    lqi: Optional[int] = None
    rssi: Optional[int] = None
    sensor_id: Optional[str] = None


class SimulationProfile(BaseModel):
    """Simulation profile used by the Sim Lab runner."""

    enabled: bool = False
    seed: Optional[int] = None
    time_multiplier: float = Field(default=1.0, gt=0)
    offline: bool = False
    offline_cycle: Optional[OfflineCycleConfig] = None
    jitter: Dict[str, float] = Field(default_factory=dict)
    spikes: Dict[str, SpikeConfig] = Field(default_factory=dict)
    stuck_outputs: List[str] = Field(default_factory=list)
    base_overrides: Dict[str, float] = Field(default_factory=dict)
    label: Optional[str] = None
    mesh_nodes: List[MeshSimNode] = Field(default_factory=list)


class Settings(BaseSettings):
    """Environment driven settings with sensible defaults for Pi nodes."""

    node_id: str = Field(default="pi-node", description="Unique identifier for MQTT topics")
    node_name: str = Field(default="Field Node", description="Human readable name")
    service_version: str = "0.1.0"
    log_level: str = "INFO"
    otel_enabled: bool = False
    otel_service_name: str = "node-agent"
    otel_exporter_otlp_endpoint: str = "http://127.0.0.1:4317"
    otel_exporter_otlp_headers: Optional[str] = None
    otel_sample_ratio: float = 1.0
    mqtt_url: str = Field(default="mqtt://127.0.0.1:1883", description="MQTT broker URL")
    mqtt_username: Optional[str] = None
    mqtt_password: Optional[str] = None
    node_forwarder_url: str = Field(
        default="http://127.0.0.1:9101",
        description="Local node-forwarder HTTP base URL (samples -> spool -> replay)",
    )
    heartbeat_interval_seconds: float = 5.0
    telemetry_interval_seconds: float = 30.0
    rolling_sample_rate_hz: int = 10
    mac_eth: Optional[str] = None
    mac_wifi: Optional[str] = None
    hardware_model: str = "Raspberry Pi 5"
    firmware_version: str = "1.4.2"
    wifi_hints: Optional[dict] = None
    provision_queue_path: Optional[str] = None
    provisioning_secret: SecretStr | None = None
    firstboot_path: Optional[str] = None
    capabilities: List[str] = Field(
        default_factory=lambda: [
            "sensors",
            "outputs",
            "mesh-ready",
            "backups",
            "bluetooth-provisioning",
        ]
    )
    adoption_token: str = Field(default_factory=lambda: uuid.uuid4().hex[:8])
    advertise_ip: Optional[str] = None
    advertise_port: int = 9000
    config_path: str = "storage/node_config.json"
    sensors: List[SensorConfig] = Field(default_factory=list)
    outputs: List[OutputConfig] = Field(default_factory=list)
    schedules: List[ScheduleConfig] = Field(default_factory=list)
    mesh: MeshRadioConfig = Field(default_factory=MeshRadioConfig)
    mesh_summary: MeshDiagnosticsSummary = Field(default_factory=MeshDiagnosticsSummary)
    renogy_bt2: RenogyBt2Config = Field(default_factory=RenogyBt2Config)
    sim_profile_path: Optional[str] = None
    simulation: SimulationProfile = Field(default_factory=SimulationProfile)
    started_at_monotonic: float | None = None
    ads1263: Ads1263HatSettings = Field(default_factory=Ads1263HatSettings)
    display: DisplayConfig = Field(default_factory=DisplayConfig)

    model_config = SettingsConfigDict(
        env_prefix="NODE_",
        env_file=".env",
        extra="ignore",
    )

    @field_validator("heartbeat_interval_seconds")
    @classmethod
    def _clamp_heartbeat(cls, value: float) -> float:
        return _clamp_interval_seconds(value, field="heartbeat_interval_seconds")

    @field_validator("telemetry_interval_seconds")
    @classmethod
    def _clamp_telemetry(cls, value: float) -> float:
        return _clamp_interval_seconds(value, field="telemetry_interval_seconds")

    @model_validator(mode="after")
    def _populate_defaults(self):
        if not self.mac_eth:
            self.mac_eth = _format_mac(uuid.getnode())
        else:
            self.mac_eth = _format_mac(self.mac_eth)
        if self.node_id == "pi-node":
            mac_suffix = self.mac_eth.replace(":", "")[-6:]
            self.node_id = f"pi-{mac_suffix}"
        if self.node_name == "Field Node":
            mac_suffix = self.mac_eth.replace(":", "")[-6:]
            self.node_name = f"Field Node {mac_suffix}"
        if self.mac_wifi:
            self.mac_wifi = _format_mac(self.mac_wifi)
        default_service_version = type(self).model_fields["service_version"].default
        if self.service_version == default_service_version:
            version_path = Path("/opt/node-agent/VERSION")
            if version_path.exists():
                try:
                    first_line = version_path.read_text(encoding="utf-8").splitlines()[0].strip()
                    if first_line:
                        self.service_version = first_line
                except Exception as exc:
                    logging.getLogger(__name__).debug("Unable to read %s: %s", version_path, exc)
        if not self.advertise_ip:
            self.advertise_ip = _default_ip()
        if not self.mesh_summary.coordinator_ieee:
            self.mesh_summary.coordinator_ieee = self.mac_eth
        if self.mesh.enabled:
            self.mesh_summary.health = "starting"
        if self.sim_profile_path:
            try:
                path = Path(self.sim_profile_path)
                if path.exists():
                    data = json.loads(path.read_text())
                    self.simulation = SimulationProfile.model_validate(data)
            except Exception as exc:
                logging.getLogger(__name__).warning("Unable to load simulation profile %s: %s", self.sim_profile_path, exc)
        if self.simulation.seed is None:
            # derive a repeatable seed per node id
            seed_source = int(uuid.uuid5(uuid.NAMESPACE_DNS, str(self.node_id)).int % (2**32 - 1))
            self.simulation.seed = seed_source
        if build_info.BUILD_FLAVOR == "prod" and self.simulation.enabled:
            raise ValueError("Simulation is not allowed in production builds")
        if self.renogy_bt2.enabled and "renogy-bt2" not in self.capabilities:
            self.capabilities.append("renogy-bt2")
        return self

    @property
    def mqtt_host(self) -> str:
        return _parsed_mqtt(self.mqtt_url).hostname or "127.0.0.1"

    @property
    def mqtt_port(self) -> int:
        return _parsed_mqtt(self.mqtt_url).port or 1883

    @property
    def mqtt_scheme(self) -> str:
        return _parsed_mqtt(self.mqtt_url).scheme or "mqtt"

    @property
    def config_file(self) -> Path:
        path = Path(self.config_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        return path

    @property
    def discovery_properties(self) -> Dict[str, str]:
        props: Dict[str, str] = {
            "node_name": self.node_name,
            "fw": self.firmware_version,
            "hw": self.hardware_model,
            "agent_version": self.service_version,
            "capabilities": ",".join(self.capabilities),
            "heartbeat": str(self.heartbeat_interval_seconds),
            "telemetry": str(self.telemetry_interval_seconds),
            "sensors": str(len(self.sensors or [])),
            "outputs": str(len(self.outputs or [])),
            "schedules": str(len(self.schedules or [])),
        }
        props.update(
            {
                "mesh_enabled": str(self.mesh.enabled).lower(),
                "mesh_driver": self.mesh.driver,
                "mesh_protocol": self.mesh.protocol,
                "mesh_channel": str(self.mesh.channel),
                "mesh_pan": self.mesh.pan_id,
                "mesh_epan": self.mesh.extended_pan_id,
            }
        )
        props.update(self.mesh_summary.as_properties())
        if self.mesh_summary.last_rssi is not None:
            props["mesh_rssi"] = str(self.mesh_summary.last_rssi)
        if self.mesh_summary.average_link_quality is not None:
            props["mesh_lqi_avg"] = f"{self.mesh_summary.average_link_quality:.1f}"
        props["mesh_nodes"] = str(self.mesh_summary.node_count or 0)
        if self.simulation.enabled:
            props["sim_profile"] = self.simulation.label or "sim"
        if self.renogy_bt2.enabled:
            props["renogy_bt2"] = "true"
            if self.renogy_bt2.address:
                props["renogy_bt2_address"] = self.renogy_bt2.address
            props["renogy_bt2_poll"] = str(self.renogy_bt2.poll_interval_seconds)
        try:
            if self.started_at_monotonic is not None:
                props["uptime_seconds"] = str(max(int(time.monotonic() - self.started_at_monotonic), 0))
            else:
                props["uptime_seconds"] = "0"
        except Exception:
            props["uptime_seconds"] = "0"
        return props

    @property
    def provision_queue_file(self) -> Path:
        """Path for storing provisioning requests from BLE/iOS setup."""

        if self.provision_queue_path:
            path = Path(self.provision_queue_path)
        else:
            path = Path(self.config_path).parent / "provision_queue.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        return path

    @property
    def firstboot_file(self) -> Path:
        """First-boot metadata injected by imaging script (node name, Wi-Fi hints)."""

        if self.firstboot_path:
            path = Path(self.firstboot_path)
        else:
            path = Path(self.config_path).parent / "node-agent-firstboot.json"
        return path


@lru_cache(maxsize=1)
def get_settings() -> Settings:
    return Settings()


@lru_cache(maxsize=32)
def _parsed_mqtt(url: str):
    return urlparse(url)


def _default_ip() -> str:
    import socket

    try:
        hostname = socket.gethostname()
        ip = socket.gethostbyname(hostname)
        if ip.startswith("127."):
            # fallback to a UDP trick to detect outward interface
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
                s.connect(("8.8.8.8", 80))
                ip = s.getsockname()[0]
        return ip
    except OSError:
        return "127.0.0.1"
