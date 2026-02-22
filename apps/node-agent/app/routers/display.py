from __future__ import annotations

from dataclasses import asdict
from datetime import datetime, timedelta, timezone
from typing import Dict, List, Optional

import httpx
import psutil
import time
from fastapi import APIRouter, Depends, HTTPException, Request, status
from fastapi.responses import HTMLResponse

from app.config import DisplayTile, Settings, get_settings
from app.display_ui import render_display_page
from app.services.latency_probe import LatencyProbe
from app.services.publisher import TelemetryPublisher

router = APIRouter()


def _core_api_base(settings: Settings) -> str:
    base = (settings.display.core_api_base_url or "").strip()
    if base:
        return base.rstrip("/")
    return f"http://{settings.mqtt_host}:8000"


def _display_tiles(settings: Settings) -> List[DisplayTile]:
    tiles = list(settings.display.tiles or [])
    if tiles:
        return tiles
    return [
        DisplayTile(type="core_status"),
        DisplayTile(type="latency"),
        DisplayTile(type="sensors"),
        DisplayTile(type="trends"),
        DisplayTile(type="outputs"),
    ]


def _resolve_sensor_tiles(
    settings: Settings,
    latest: Dict[str, Dict[str, object]],
    *,
    now: datetime,
) -> List[Dict[str, object]]:
    sensors_by_id = {sensor.sensor_id: sensor for sensor in settings.sensors or []}
    out: List[Dict[str, object]] = []

    for tile in _display_tiles(settings):
        if tile.type != "sensor":
            continue
        sensor_id = (tile.sensor_id or "").strip()
        if not sensor_id:
            continue
        sensor_cfg = sensors_by_id.get(sensor_id)
        reading = latest.get(sensor_id)
        if not sensor_cfg:
            out.append(
                {
                    "type": "sensor",
                    "sensor_id": sensor_id,
                    "name": sensor_id,
                    "missing": True,
                    "stale": True,
                    "value": None,
                    "unit": "",
                    "quality": 4,
                    "age_seconds": None,
                }
            )
            continue
        out.append(_sensor_row(sensor_cfg, reading, now=now))
    return out


def _sensor_row(sensor_cfg, reading: Optional[Dict[str, object]], *, now: datetime) -> Dict[str, object]:
    timestamp_raw = reading.get("timestamp") if reading else None
    ts = None
    if isinstance(timestamp_raw, str) and timestamp_raw:
        try:
            ts = datetime.fromisoformat(timestamp_raw.replace("Z", "+00:00")).astimezone(timezone.utc)
        except ValueError:
            ts = None

    age_seconds = (now - ts).total_seconds() if ts else None
    interval = float(getattr(sensor_cfg, "interval_seconds", 30.0) or 30.0)
    stale_threshold = max(interval * 3.0, 30.0)
    stale = age_seconds is None or age_seconds > stale_threshold
    return {
        "type": "sensor",
        "sensor_id": sensor_cfg.sensor_id,
        "name": sensor_cfg.name,
        "missing": False,
        "stale": bool(stale),
        "value": reading.get("value") if reading else None,
        "unit": sensor_cfg.unit,
        "quality": int(reading.get("quality", 0)) if reading else 4,
        "age_seconds": age_seconds,
    }


@router.get("/display", response_class=HTMLResponse)
async def display_page(settings: Settings = Depends(get_settings)) -> HTMLResponse:
    return HTMLResponse(render_display_page(settings))


@router.get("/v1/display/state")
async def display_state(request: Request, settings: Settings = Depends(get_settings)) -> Dict[str, object]:
    publisher: TelemetryPublisher | None = getattr(request.app.state, "publisher", None)
    probe: LatencyProbe | None = getattr(request.app.state, "latency_probe", None)
    snapshot = publisher.display_snapshot() if publisher else {}
    latest = snapshot.get("latest") or {}
    if not isinstance(latest, dict):
        latest = {}

    now = datetime.now(timezone.utc)
    sensors_by_id = {sensor.sensor_id: sensor for sensor in settings.sensors or []}

    configured_sensors: List[Dict[str, object]] = []
    for sensor_id, sensor_cfg in sensors_by_id.items():
        reading = latest.get(sensor_id) if isinstance(latest, dict) else None
        configured_sensors.append(_sensor_row(sensor_cfg, reading if isinstance(reading, dict) else None, now=now))

    tile_sensors = _resolve_sensor_tiles(settings, latest if isinstance(latest, dict) else {}, now=now)

    comms_status = "unknown"
    comms_detail = "—"
    mqtt_connected = bool(snapshot.get("mqtt_connected"))
    forwarder = snapshot.get("forwarder") or {}
    if not isinstance(forwarder, dict):
        forwarder = {}
    spool = forwarder.get("last_status")
    if not isinstance(spool, dict):
        spool = None

    backlog_samples = None
    spool_bytes = None
    losses_pending = None
    if spool:
        backlog_samples = spool.get("backlog_samples")
        spool_bytes = spool.get("spool_bytes")
        losses_pending = spool.get("losses_pending")
        if backlog_samples is None:
            try:
                next_seq = int(spool.get("next_seq") or 0)
                acked_seq = int(spool.get("acked_seq") or 0)
                backlog_samples = max(next_seq - acked_seq - 1, 0)
            except Exception:
                backlog_samples = None
    last_publish_at = snapshot.get("last_publish_at")
    if mqtt_connected:
        comms_status = "connected"
        comms_detail = "MQTT connected"
    else:
        comms_status = "offline"
        reason = snapshot.get("last_mqtt_error") or "reconnecting"
        comms_detail = f"MQTT {reason}"
    if backlog_samples is not None:
        comms_detail += f"; backlog_samples={backlog_samples}"
    if spool_bytes is not None:
        try:
            comms_detail += f"; spool_bytes={int(spool_bytes)}"
        except Exception:
            pass
    if losses_pending is not None:
        try:
            comms_detail += f"; losses_pending={int(losses_pending)}"
        except Exception:
            pass
    if last_publish_at:
        comms_detail += f"; last_publish={last_publish_at}"

    latency = probe.snapshot() if probe else None
    latency_payload = asdict(latency) if latency else {}

    uptime = int(
        time.monotonic() - getattr(request.app.state, "started_at", time.monotonic())
    )
    disk = psutil.disk_usage("/")
    memory = psutil.virtual_memory()

    return {
        "display": {
            "enabled": bool(settings.display.enabled),
            "outputs_enabled": bool(settings.display.outputs_enabled),
            "ui_refresh_seconds": int(settings.display.ui_refresh_seconds),
            "latency_sample_seconds": int(settings.display.latency_sample_seconds),
            "latency_window_samples": int(settings.display.latency_window_samples),
            "tiles": [tile.model_dump(mode="json") for tile in _display_tiles(settings)],
            "trends": [
                {"sensor_id": t.sensor_id, "default_range": t.default_range}
                for t in (settings.display.trends or [])
            ],
            "trend_ranges": list(settings.display.trend_ranges or []),
            "local_pin_required": bool(settings.display.local_pin_hash),
            "core_api_base_url": _core_api_base(settings),
        },
        "node": {
            "node_id": settings.node_id,
            "node_name": settings.node_name,
            "service_version": settings.service_version,
            "ip": settings.advertise_ip,
        },
        "system": {
            "uptime_seconds": uptime,
            "cpu_percent": psutil.cpu_percent(interval=0.0),
            "memory_percent": memory.percent,
            "storage_used_bytes": disk.used,
        },
        "comms": {
            "status": comms_status,
            "detail": comms_detail,
            "mqtt_connected": mqtt_connected,
            "spool_backlog_samples": backlog_samples,
            "spool_bytes": spool_bytes,
            "spool_losses_pending": losses_pending,
            "forwarder": forwarder,
        },
        "latency": latency_payload,
        "sensors": configured_sensors,
        "tiles": {
            "sensors": tile_sensors,
        },
    }


@router.get("/v1/display/trends")
async def display_trends(
    request: Request,
    sensor_id: str,
    range: str = "6h",
    settings: Settings = Depends(get_settings),
) -> Dict[str, object]:
    """Fetch trend data from the core server, falling back to local history when offline."""

    if not any(sensor.sensor_id == sensor_id for sensor in (settings.sensors or [])):
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Sensor not found")
    if range not in {"1h", "6h", "24h"}:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Invalid range")
    hours = {"1h": 1, "6h": 6, "24h": 24}[range]
    interval = 60 if hours <= 1 else 300 if hours <= 6 else 900
    end = datetime.now(timezone.utc)
    start = end - timedelta(hours=hours)
    params = [
        ("sensor_ids", sensor_id),
        ("start", start.isoformat().replace("+00:00", "Z")),
        ("end", end.isoformat().replace("+00:00", "Z")),
        ("interval", str(interval)),
    ]

    url = f"{_core_api_base(settings)}/api/metrics/query"
    headers = {}
    auth = request.headers.get("authorization")
    if auth:
        headers["authorization"] = auth

    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            resp = await client.get(url, params=params, headers=headers)
        if resp.status_code >= 400:
            raise RuntimeError(f"{resp.status_code}: {resp.text}")
        payload = resp.json()
        if isinstance(payload, dict) and "series" in payload:
            return payload
    except Exception:
        publisher: TelemetryPublisher | None = getattr(request.app.state, "publisher", None)
        points = []
        if publisher:
            history = publisher.display_history(sensor_id, max_points=400)
            for entry in history:
                ts = entry.get("timestamp")
                value = entry.get("value")
                if ts and value is not None:
                    points.append({"timestamp": ts, "value": float(value), "samples": 1})
        return {"series": [{"sensor_id": sensor_id, "label": None, "sensor_name": None, "points": points}]}

    return {"series": [{"sensor_id": sensor_id, "label": None, "sensor_name": None, "points": []}]}


@router.get("/v1/display/outputs")
async def display_outputs(request: Request, settings: Settings = Depends(get_settings)) -> List[Dict[str, object]]:
    if not settings.display.outputs_enabled:
        return []
    allowed_ids = {out.output_id for out in (settings.outputs or []) if out.output_id}
    if not allowed_ids:
        return []

    url = f"{_core_api_base(settings)}/api/outputs"
    headers = {}
    auth = request.headers.get("authorization")
    if auth:
        headers["authorization"] = auth
    async with httpx.AsyncClient(timeout=10.0) as client:
        resp = await client.get(url, headers=headers)
    if resp.status_code >= 400:
        raise HTTPException(status_code=resp.status_code, detail=resp.text)
    payload = resp.json()
    if not isinstance(payload, list):
        raise HTTPException(status_code=500, detail="Invalid core outputs payload")
    return [item for item in payload if isinstance(item, dict) and item.get("id") in allowed_ids]


def _sha256_hex(value: str) -> str:
    import hashlib

    return hashlib.sha256(value.encode("utf-8")).hexdigest()


@router.post("/v1/display/outputs/{output_id}/command")
async def display_output_command(
    output_id: str,
    request: Request,
    settings: Settings = Depends(get_settings),
) -> Dict[str, object]:
    if not settings.display.outputs_enabled:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Outputs not enabled")
    if not any(output.output_id == output_id for output in (settings.outputs or [])):
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Output not configured for this node")

    auth = request.headers.get("authorization")
    if not auth:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Missing bearer token")

    body = await request.json()
    desired = str(body.get("state") or "").strip()
    if not desired:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Missing state")

    if settings.display.local_pin_hash:
        supplied = str(body.get("pin") or "").strip()
        if not supplied:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="PIN required")
        if _sha256_hex(supplied).lower() != str(settings.display.local_pin_hash).lower():
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Invalid PIN")

    reason = str(body.get("reason") or "local_display").strip()
    reason = f"actor=local_display node_id={settings.node_id} {reason}".strip()

    url = f"{_core_api_base(settings)}/api/outputs/{output_id}/command"
    headers = {"authorization": auth, "content-type": "application/json"}
    payload = {"state": desired, "reason": reason}
    async with httpx.AsyncClient(timeout=10.0) as client:
        resp = await client.post(url, json=payload, headers=headers)
    if resp.status_code >= 400:
        raise HTTPException(status_code=resp.status_code, detail=resp.text)
    decoded = resp.json()
    if not isinstance(decoded, dict):
        raise HTTPException(status_code=500, detail="Invalid core output response")
    return decoded
