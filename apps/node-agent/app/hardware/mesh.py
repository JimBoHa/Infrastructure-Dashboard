"""Mesh radio abstraction built around zigpy-compatible adapters."""
from __future__ import annotations

import asyncio
import contextlib
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
import logging
import math
from pathlib import Path
import random
import time
from typing import Any, Callable, Dict, List, Optional
import uuid

from app.config import MeshSimNode, Settings, SimulationProfile

try:  # pragma: no cover - optional dependency
    from zigpy.application import ControllerApplication  # type: ignore
except Exception:  # pragma: no cover - optional dependency
    ControllerApplication = None  # type: ignore

logger = logging.getLogger(__name__)


def _normalize_ieee(value: str) -> str:
    cleaned = value.replace("-", ":").upper()
    parts = cleaned.split(":")
    if len(parts) == 1 and len(cleaned) == 16:
        return ":".join(cleaned[i : i + 2] for i in range(0, 16, 2))
    if len(parts) == 8:
        return ":".join(part.zfill(2) for part in parts)
    return cleaned


@dataclass
class MeshDiagnostics:
    """Diagnostic envelope for mesh nodes."""

    lqi: Optional[int] = None
    rssi: Optional[int] = None
    link_margin: Optional[float] = None
    snr: Optional[float] = None
    battery_percent: Optional[float] = None
    parent: Optional[str] = None
    depth: Optional[int] = None
    last_hop: Optional[str] = None

    def as_dict(self) -> Dict[str, Any]:
        return {
            key: value
            for key, value in {
                "lqi": self.lqi,
                "rssi": self.rssi,
                "link_margin": self.link_margin,
                "snr": self.snr,
                "battery_percent": self.battery_percent,
                "parent": self.parent,
                "depth": self.depth,
                "last_hop": self.last_hop,
            }.items()
            if value is not None
        }


@dataclass
class MeshSample:
    """Materialized attribute report from a mesh endpoint."""

    ieee: str
    endpoint: int
    cluster: int
    attribute: int
    value: Any
    unit: Optional[str] = None
    timestamp: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    metadata: Dict[str, Any] = field(default_factory=dict)
    diagnostics: MeshDiagnostics = field(default_factory=MeshDiagnostics)
    sensor_id: Optional[str] = None

    def identifier(self) -> str:
        """Stable identifier used when publishing telemetry."""
        if self.sensor_id:
            return self.sensor_id
        return f"{self.ieee}:{self.endpoint}:{self.cluster}:{self.attribute}"

    def as_payload(self) -> Dict[str, Any]:
        payload = {
            "sensor_id": self.identifier(),
            "timestamp": self.timestamp.isoformat(),
            "value": self.value,
            "unit": self.unit,
            "ieee": self.ieee,
            "endpoint": self.endpoint,
            "cluster": self.cluster,
            "attribute": self.attribute,
        }
        if self.metadata:
            payload["metadata"] = self.metadata
        diag = self.diagnostics.as_dict()
        if diag:
            payload["diagnostics"] = diag
        return payload


class MeshAdapter:
    """Coordinates the mesh radio, normalizing incoming reports for the publisher."""

    def __init__(
        self,
        settings: Settings,
        application_factory: Optional[Callable[..., Any]] = None,
    ):
        self.settings = settings
        self.config = settings.mesh
        if settings.simulation.enabled and application_factory is None:
            self._application_factory = None
        elif application_factory is None and ControllerApplication is not None:
            self._application_factory = self._default_application_factory
        else:
            self._application_factory = application_factory
        self._listeners: List[Callable[[MeshSample], None]] = []
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._application: Any = None
        self._running = False
        self._link_stats: Dict[str, Dict[str, float]] = defaultdict(lambda: {"count": 0, "lqi_total": 0.0})
        self._topology: Dict[str, Dict[str, Any]] = {}
        self._polling_task: Optional[asyncio.Task] = None
        self._diagnostics_task: Optional[asyncio.Task] = None
        self._stop_event: asyncio.Event = asyncio.Event()
        self._sim_nodes: List[MeshSimNode] = self._build_sim_nodes()
        self._sim_rng = random.Random(self.settings.simulation.seed or 1)
        self._sim_started = time.monotonic()

    @property
    def enabled(self) -> bool:
        return bool(self.config.enabled)

    def add_listener(self, callback: Callable[[MeshSample], None]) -> None:
        if callback not in self._listeners:
            self._listeners.append(callback)

    def remove_listener(self, callback: Callable[[MeshSample], None]) -> None:
        if callback in self._listeners:
            self._listeners.remove(callback)

    async def start_join(self, duration_seconds: int = 120) -> bool:
        """Permit new devices to join the mesh for a limited window."""

        if not self.enabled:
            logger.info("Mesh adapter join requested while disabled")
            return False
        self.settings.mesh_summary.health_details["join_requested_at"] = datetime.now(timezone.utc).isoformat()
        self.settings.mesh_summary.health_details["join_until"] = (
            datetime.now(timezone.utc) + timedelta(seconds=duration_seconds)
        ).isoformat()
        if self._application and hasattr(self._application, "permit"):
            try:  # pragma: no cover - exercised with real zigpy
                await self._application.permit(time_s=duration_seconds)
                logger.info("Mesh join permitted for %ss", duration_seconds)
                return True
            except Exception:
                logger.exception("Failed to open mesh join window")
                return False
        logger.info("Mesh adapter running in mock mode; treating join as no-op")
        return True

    async def remove_device(self, ieee: str) -> bool:
        """Attempt to remove/ban a device by IEEE identifier."""

        normalized = _normalize_ieee(ieee)
        if not self.enabled:
            logger.info("Mesh adapter remove requested while disabled")
            return False
        if self._application and hasattr(self._application, "remove"):
            try:  # pragma: no cover - hardware path
                await self._application.remove(normalized)
                logger.info("Requested mesh leave for %s", normalized)
                return True
            except Exception:
                logger.exception("Failed to remove mesh device %s", normalized)
                return False
        logger.info("Mesh adapter mock mode; removal logged only for %s", normalized)
        return True

    async def start(self) -> None:
        if not self.enabled or self._running:
            return
        self._loop = asyncio.get_running_loop()
        if self._application_factory is None:
            mode = "simulated" if self._sim_nodes else "mock"
            logger.info("Mesh adapter running in %s mode (zigpy not available or factory not provided)", mode)
            self.settings.mesh_summary.health = mode
            self._running = True
            self._stop_event.clear()
            self._start_background_tasks()
            return
        try:
            self._application = await self._application_factory()
        except Exception:  # pragma: no cover - defensive logging
            logger.exception("Failed to start mesh controller application")
            self.settings.mesh_summary.health = "error"
            return
        coordinator_ieee = getattr(self._application, "ieee", None)
        if coordinator_ieee:
            self.settings.mesh_summary.coordinator_ieee = _normalize_ieee(str(coordinator_ieee))
        self.settings.mesh_summary.health = "online"
        self._running = True
        self._stop_event.clear()
        self._start_background_tasks()
        if hasattr(self._application, "add_listener"):
            try:
                self._application.add_listener(self._handle_zigpy_event)
            except Exception:  # pragma: no cover - defensive logging
                logger.exception("Unable to attach zigpy listener; continuing without live events")

    async def stop(self) -> None:
        if not self._running:
            return
        self._running = False
        self._stop_event.set()
        await self._stop_background_tasks()
        if self._application and hasattr(self._application, "shutdown"):
            try:
                await self._application.shutdown()
            except Exception:  # pragma: no cover - defensive logging
                logger.exception("Failed to shut down mesh controller cleanly")
        self._application = None

    def ingest_attribute_report(
        self,
        *,
        ieee: str,
        endpoint: int,
        cluster: int,
        attribute: int,
        value: Any,
        unit: Optional[str] = None,
        diagnostics: Optional[MeshDiagnostics] = None,
        metadata: Optional[Dict[str, Any]] = None,
        sensor_id: Optional[str] = None,
    ) -> MeshSample:
        normalized_ieee = _normalize_ieee(ieee)
        sample = MeshSample(
            ieee=normalized_ieee,
            endpoint=endpoint,
            cluster=cluster,
            attribute=attribute,
            value=value,
            unit=unit,
            metadata=metadata or {},
            diagnostics=diagnostics or MeshDiagnostics(),
            sensor_id=sensor_id,
        )
        self._emit(sample)
        return sample

    def _emit(self, sample: MeshSample) -> None:
        self._update_summary(sample)
        self._record_topology(sample)
        for listener in list(self._listeners):
            try:
                listener(sample)
            except Exception:  # pragma: no cover - defensive logging
                logger.exception("Mesh listener raised an exception")

    def update_simulation(self, profile: SimulationProfile) -> None:
        """Refresh simulated mesh nodes at runtime."""

        self.settings.simulation = profile
        self._sim_rng.seed(profile.seed or 1)
        if not profile.enabled:
            self._sim_nodes = []
            self.settings.mesh_summary.health_details["last_poll_status"] = "sim_disabled"
            return
        if profile.mesh_nodes:
            self._sim_nodes = list(profile.mesh_nodes)
        else:
            self._sim_nodes = self._build_sim_nodes()
        self._sim_started = time.monotonic()

    def _update_summary(self, sample: MeshSample) -> None:
        summary = self.settings.mesh_summary
        diag = sample.diagnostics
        summary.last_updated = sample.timestamp
        if diag.parent:
            summary.last_parent = diag.parent
        if diag.battery_percent is not None:
            summary.last_battery_percent = diag.battery_percent
        if diag.rssi is not None:
            summary.last_rssi = diag.rssi
        stats = self._link_stats[sample.ieee]
        if diag.lqi is not None:
            stats["count"] += 1
            stats["lqi_total"] += float(diag.lqi)
        summary.node_count = len(self._link_stats)
        total_samples = sum(item["count"] for item in self._link_stats.values())
        total_lqi = sum(item["lqi_total"] for item in self._link_stats.values())
        if total_samples:
            summary.average_link_quality = total_lqi / total_samples
        if summary.health in {"unknown", "starting"}:
            summary.health = "online"

    def snapshot_summary(self) -> Dict[str, Any]:
        """Return a serializable copy of the current summary."""
        return self.settings.mesh_summary.model_dump()

    def topology_snapshot(self) -> List[Dict[str, Any]]:
        """Return the latest known mesh devices/diagnostics."""

        return sorted(self._topology.values(), key=lambda item: item.get("ieee", ""))

    def _build_sim_nodes(self) -> List[MeshSimNode]:
        profile = getattr(self.settings, "simulation", None)
        if not profile or not profile.enabled:
            return []
        nodes = list(getattr(profile, "mesh_nodes", []) or [])
        if nodes:
            return nodes

        parent = self.settings.mesh_summary.coordinator_ieee
        if parent and len(parent.replace(":", "")) != 16:
            parent = None
        parent = _normalize_ieee(parent or self._sim_ieee("coord"))
        return [
            MeshSimNode(
                ieee=self._sim_ieee("leaf-1"),
                cluster=0x0402,
                attribute=0x0000,
                unit="C",
                base_value=21.5,
                amplitude=1.6,
                jitter=0.15,
                battery_percent=72.0,
                parent=parent,
                depth=1,
                lqi=215,
                rssi=-42,
            ),
            MeshSimNode(
                ieee=self._sim_ieee("leaf-2"),
                cluster=0x0405,
                attribute=0x0000,
                unit="%",
                base_value=38.0,
                amplitude=4.2,
                jitter=0.25,
                battery_percent=63.0,
                parent=parent,
                depth=2,
                lqi=188,
                rssi=-55,
            ),
        ]

    def _sim_ieee(self, label: str) -> str:
        seed = uuid.uuid5(uuid.NAMESPACE_DNS, f"{self.settings.node_id}-{label}").hex[:16].upper()
        return ":".join(seed[i : i + 2] for i in range(0, 16, 2))

    # --------------------------------------------------------------------- #
    # Zigpy configuration + polling (optional, runs only when zigpy is available)
    # --------------------------------------------------------------------- #

    async def _default_application_factory(self):  # pragma: no cover - exercised with real zigpy
        if ControllerApplication is None:
            raise RuntimeError("zigpy is not installed")
        config = self._build_zigpy_config()
        logger.info("Starting zigpy ControllerApplication with %s", config)
        app = await ControllerApplication.new(config=config, auto_form=True)
        return app

    def _build_zigpy_config(self) -> Dict[str, Any]:
        network_key = self.config.network_key.replace("0x", "").replace(" ", "")
        tc_link_key = self.config.tc_link_key.replace("0x", "").replace(" ", "") if self.config.tc_link_key else None
        db_path = Path(self.settings.config_path).parent / "zigpy.db"
        device_path = self.config.serial_device or "/dev/ttyUSB0"
        return {
            "database_path": str(db_path),
            "device": {
                "path": device_path,
                "baudrate": self.config.baudrate,
            },
            "network": {
                "channel": self.config.channel,
                "pan_id": int(self.config.pan_id, 16),
                "extended_pan_id": bytes.fromhex(self.config.extended_pan_id.replace(":", "")),
                "network_key": bytes.fromhex(network_key),
                **({"tc_link_key": bytes.fromhex(tc_link_key)} if tc_link_key else {}),
            },
        }

    def _start_background_tasks(self) -> None:
        interval = max(self.config.polling_interval_seconds, 0.5)
        diag_interval = max(self.config.diagnostics_interval_seconds, 10.0)
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            return
        if not self._polling_task:
            self._polling_task = loop.create_task(self._poll_loop(interval))
        if not self._diagnostics_task:
            self._diagnostics_task = loop.create_task(self._diagnostics_loop(diag_interval))

    async def _stop_background_tasks(self) -> None:
        tasks = [task for task in (self._polling_task, self._diagnostics_task) if task]
        if not tasks:
            return
        for task in tasks:
            task.cancel()
        with contextlib.suppress(Exception):
            await asyncio.gather(*tasks, return_exceptions=True)
        self._polling_task = None
        self._diagnostics_task = None

    async def _poll_loop(self, interval: float) -> None:
        while not self._stop_event.is_set():
            try:
                await self._emit_topology_snapshot()
            except Exception:  # pragma: no cover - defensive logging
                logger.exception("Mesh topology poll failed")
                self.settings.mesh_summary.health_details["last_poll_status"] = "error"
            try:
                await asyncio.wait_for(self._stop_event.wait(), timeout=interval)
            except asyncio.TimeoutError:
                continue

    async def _diagnostics_loop(self, interval: float) -> None:
        while not self._stop_event.is_set():
            summary = self.settings.mesh_summary
            summary.health_details["known_devices"] = len(self._topology)
            avg_lqi = round(summary.average_link_quality, 2) if summary.average_link_quality else None
            if avg_lqi is not None:
                summary.health_details["avg_lqi"] = avg_lqi
            else:
                summary.health_details.pop("avg_lqi", None)
            summary.health_details["last_poll_at"] = datetime.now(timezone.utc).isoformat()
            try:
                await asyncio.wait_for(self._stop_event.wait(), timeout=interval)
            except asyncio.TimeoutError:
                continue

    async def _emit_topology_snapshot(self) -> None:
        """Poll the controller for neighbor tables and synthesize diagnostics samples."""

        app = self._application
        summary = self.settings.mesh_summary
        if not app:
            if self._sim_nodes:
                self._emit_simulated_snapshot()
                return
            summary.health_details["last_poll_status"] = "unavailable"
            return
        devices = getattr(app, "devices", {}) or {}
        neighbor_count = 0
        for device in list(devices.values()):
            ieee = getattr(device, "ieee", None) or getattr(device, "dev_ieee", None)
            if not ieee:
                continue
            ieee_normalized = _normalize_ieee(str(ieee))
            neighbors = self._extract_neighbors(device)
            neighbor_count += len(neighbors)
            diagnostics = MeshDiagnostics(
                lqi=getattr(device, "lqi", None),
                rssi=getattr(device, "rssi", None),
                link_margin=getattr(device, "link_margin", None),
                snr=getattr(device, "snr", None),
                battery_percent=self._extract_battery(device),
                parent=_normalize_ieee(str(getattr(device, "parent_ieee", ""))) if getattr(device, "parent_ieee", None) else None,
                depth=getattr(device, "depth", None),
                last_hop=_normalize_ieee(str(getattr(device, "last_hop", ""))) if getattr(device, "last_hop", None) else None,
            )
            metadata = {
                "neighbors": neighbors,
                "last_seen": getattr(device, "last_seen", None),
            }
            sample = MeshSample(
                ieee=ieee_normalized,
                endpoint=0,
                cluster=0,
                attribute=0xFFF1,
                value=diagnostics.lqi if diagnostics.lqi is not None else diagnostics.rssi or 0,
                metadata=metadata,
                diagnostics=diagnostics,
                sensor_id=f"mesh-{ieee_normalized.replace(':', '').lower()}-diag",
            )
            self._emit(sample)
        summary.health_details["neighbors"] = neighbor_count
        summary.health_details["last_poll_status"] = "ok"

    def _emit_simulated_snapshot(self) -> None:
        summary = self.settings.mesh_summary
        elapsed = max(time.monotonic() - self._sim_started, 0.0)
        summary.health = "simulated"
        summary.health_details["last_poll_status"] = "simulated"
        summary.health_details["known_devices"] = len(self._sim_nodes)
        summary.health_details["neighbors"] = max(len(self._sim_nodes) - 1, 0)
        for idx, node in enumerate(self._sim_nodes):
            base = float(node.base_value)
            amplitude = float(node.amplitude)
            value = base + math.sin((elapsed / 5.0) + idx) * amplitude
            if node.jitter:
                value += self._sim_rng.uniform(-node.jitter, node.jitter)
            diagnostics = MeshDiagnostics(
                lqi=node.lqi,
                rssi=node.rssi,
                battery_percent=node.battery_percent,
                parent=_normalize_ieee(node.parent) if node.parent else None,
                depth=node.depth,
            )
            sample = MeshSample(
                ieee=_normalize_ieee(node.ieee),
                endpoint=node.endpoint,
                cluster=node.cluster,
                attribute=node.attribute,
                value=round(value, 3),
                unit=node.unit,
                diagnostics=diagnostics,
                sensor_id=node.sensor_id,
            )
            self._emit(sample)

    def _extract_neighbors(self, device: Any) -> List[Dict[str, Any]]:
        neighbors = getattr(device, "neighbors", None)
        result: List[Dict[str, Any]] = []
        if not neighbors:
            return result
        for neighbor in neighbors:
            ieee = getattr(neighbor, "ieee", None) or getattr(neighbor, "neighbor", None)
            if not ieee:
                continue
            entry = {
                "ieee": _normalize_ieee(str(ieee)),
                "lqi": getattr(neighbor, "lqi", None),
                "depth": getattr(neighbor, "depth", None),
                "relationship": getattr(neighbor, "relationship", None),
                "rssi": getattr(neighbor, "rssi", None),
            }
            result.append({k: v for k, v in entry.items() if v is not None})
        return result

    def _record_topology(self, sample: MeshSample) -> None:
        self._topology[sample.ieee] = {
            "ieee": sample.ieee,
            "last_seen": sample.timestamp.isoformat(),
            "diagnostics": sample.diagnostics.as_dict(),
            "metadata": sample.metadata,
            "sensor_id": sample.sensor_id,
            "attribute": sample.attribute,
            "cluster": sample.cluster,
            "endpoint": sample.endpoint,
            "value": sample.value,
        }

    def _extract_battery(self, device: Any) -> Optional[float]:
        """Best-effort extraction of battery percentage from zigpy devices."""

        battery_source = getattr(device, "battery", None) or getattr(device, "power_source", None)
        if battery_source and hasattr(battery_source, "battery_percentage_remaining"):
            try:
                value = battery_source.battery_percentage_remaining
                return float(value)
            except Exception:
                return None
        if hasattr(device, "battery_percent"):
            try:
                return float(getattr(device, "battery_percent"))
            except Exception:
                return None
        return None

    # --------------------------------------------------------------------- #
    # Zigpy event plumbing (optional, runs only when zigpy is available)
    # --------------------------------------------------------------------- #

    def _handle_zigpy_event(self, event: str, *args, **kwargs) -> None:  # pragma: no cover - exercised with real hardware
        if event not in {"attribute_updated", "cluster_command"}:
            return
        device = kwargs.get("device") or (args[0] if args else None)
        if not device:
            return
        ieee = getattr(device, "ieee", None)
        if ieee is None:
            return
        endpoint_id = kwargs.get("endpoint_id") or getattr(args[1], "endpoint_id", None) if len(args) > 1 else None
        cluster_id = kwargs.get("cluster_id") or getattr(args[1], "cluster_id", None) if len(args) > 1 else None
        attribute_id = kwargs.get("attribute_id") or kwargs.get("attr_id")
        value = kwargs.get("value") or kwargs.get("data")
        diagnostics = MeshDiagnostics(
            lqi=getattr(device, "lqi", None),
            rssi=getattr(device, "rssi", None),
            parent=getattr(device, "parent_ieee", None),
        )
        metadata = kwargs.get("metadata") or {}
        if None in (endpoint_id, cluster_id) or attribute_id is None:
            return
        self.ingest_attribute_report(
            ieee=str(ieee),
            endpoint=int(endpoint_id),
            cluster=int(cluster_id),
            attribute=int(attribute_id),
            value=value,
            metadata=metadata,
            diagnostics=diagnostics,
        )


__all__ = ["MeshAdapter", "MeshDiagnostics", "MeshSample"]
