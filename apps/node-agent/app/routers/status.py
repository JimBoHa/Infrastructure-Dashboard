from __future__ import annotations

import time
from typing import Dict

import psutil
from fastapi import APIRouter, Depends, Request

from app.config import Settings, get_settings
from app.http_utils import mesh_adapter
from app.services.latency_probe import LatencyProbe

router = APIRouter(prefix="/v1")


@router.get("/status")
async def status_endpoint(request: Request, settings: Settings = Depends(get_settings)) -> Dict[str, object]:
    uptime = int(time.monotonic() - getattr(request.app.state, "started_at", time.monotonic()))
    disk = psutil.disk_usage("/")
    memory = psutil.virtual_memory()
    battery = psutil.sensors_battery()
    mesh = mesh_adapter(request.app)
    probe: LatencyProbe | None = getattr(request.app.state, "latency_probe", None)
    latency = probe.snapshot() if probe else None
    return {
        "node_id": settings.node_id,
        "node_name": settings.node_name,
        "service_version": settings.service_version,
        "hardware_model": settings.hardware_model,
        "firmware_version": settings.firmware_version,
        "mac_eth": settings.mac_eth,
        "mac_wifi": settings.mac_wifi,
        "uptime_seconds": uptime,
        "cpu_percent": psutil.cpu_percent(interval=0.0),
        "memory_percent": memory.percent,
        "storage_used_bytes": disk.used,
        "storage_total_bytes": disk.total,
        "heartbeat_interval_seconds": settings.heartbeat_interval_seconds,
        "telemetry_interval_seconds": settings.telemetry_interval_seconds,
        "capabilities": settings.capabilities,
        "battery": {
            "percent": battery.percent if battery else None,
            "plugged": battery.power_plugged if battery else None,
        },
        "mesh_summary": settings.mesh_summary.model_dump(),
        "mesh_topology": mesh.topology_snapshot() if mesh else [],
        "simulation": settings.simulation.model_dump(),
        "display": {
            "enabled": bool(settings.display.enabled),
            "outputs_enabled": bool(settings.display.outputs_enabled),
            "latency_target": f"{latency.target_host}:{latency.target_port}" if latency else None,
            "last_latency_ms": latency.last_latency_ms if latency else None,
            "jitter_ms": latency.jitter_ms if latency else None,
            "last_sample_at": latency.last_sample_at if latency else None,
        },
    }


@router.get("/discovery")
async def discovery_payload(settings: Settings = Depends(get_settings)):
    battery_percent = None
    try:
        battery = psutil.sensors_battery()
        if battery:
            battery_percent = battery.percent
    except Exception:
        battery_percent = None

    return {
        "service_type": "_iotnode._tcp.local.",
        "service_name": f"{settings.node_id}._iotnode._tcp.local.",
        "ip": settings.advertise_ip,
        "port": settings.advertise_port,
        "properties": settings.discovery_properties,
        "mac_eth": settings.mac_eth,
        "mac_wifi": settings.mac_wifi,
        "mesh_summary": settings.mesh_summary.model_dump(),
        "battery_percent": battery_percent,
    }
