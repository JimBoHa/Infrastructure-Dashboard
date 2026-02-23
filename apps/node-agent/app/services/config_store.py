from __future__ import annotations

"""Persistence helpers for the node agent configuration."""

import json
from pathlib import Path
from typing import Any, Dict, Optional

from pydantic import ValidationError

from app.config import Settings


class ConfigStore:
    """Handles serialization of node configuration for restore/backups."""

    def __init__(self, path: Path):
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def load(self) -> Optional[Dict[str, Any]]:
        if not self.path.exists():
            return None
        try:
            return json.loads(self.path.read_text())
        except json.JSONDecodeError:
            return None

    def save(self, payload: Dict[str, Any]) -> None:
        temp_path = self.path.with_suffix(".tmp")
        temp_path.write_text(json.dumps(payload, indent=2, sort_keys=True))
        temp_path.replace(self.path)


def export_config(settings: Settings) -> Dict[str, Any]:
    """Serialize current settings to a plain dict."""

    return {
        "node": {
            "node_id": settings.node_id,
            "node_name": settings.node_name,
            "hardware_model": settings.hardware_model,
            "firmware_version": settings.firmware_version,
            "mac_eth": settings.mac_eth,
            "mac_wifi": settings.mac_wifi,
            "adoption_token": settings.adoption_token,
            "heartbeat_interval_seconds": settings.heartbeat_interval_seconds,
            "telemetry_interval_seconds": settings.telemetry_interval_seconds,
            "capabilities": settings.capabilities,
        },
        "wifi_hints": settings.wifi_hints or {},
        "sensors": [sensor.model_dump(mode="json") for sensor in settings.sensors],
        "outputs": [output.model_dump(mode="json") for output in settings.outputs],
        "schedules": [schedule.model_dump(mode="json") for schedule in settings.schedules],
        "mesh": settings.mesh.model_dump(mode="json"),
        "mesh_summary": settings.mesh_summary.model_dump(mode="json"),
        "renogy_bt2": settings.renogy_bt2.model_dump(mode="json"),
        "ads1263": settings.ads1263.model_dump(mode="json"),
        "display": settings.display.model_dump(mode="json"),
        "saved_at": settings.model_dump(mode="json").get("saved_at", None) or None,
        "simulation": settings.simulation.model_dump(mode="json"),
    }


def apply_config(settings: Settings, payload: Dict[str, Any]) -> Settings:
    """Apply a persisted config payload safely.

    This function is intentionally *transactional*:
      - It validates/clamps timing fields via the Settings model validators.
      - It rejects invalid payloads without partially mutating the live Settings object.
    """

    current = settings.model_dump(mode="python")
    candidate = dict(current)

    node_payload = payload.get("node") or {}
    if isinstance(node_payload, dict):
        for field in [
            "node_id",
            "node_name",
            "hardware_model",
            "firmware_version",
            "mac_eth",
            "mac_wifi",
            "adoption_token",
            "heartbeat_interval_seconds",
            "telemetry_interval_seconds",
            "capabilities",
        ]:
            if field in node_payload:
                if field == "adoption_token" and node_payload[field] is None:
                    continue
                candidate[field] = node_payload[field]

    if "sensors" in payload and payload.get("sensors") is not None:
        candidate["sensors"] = payload.get("sensors") or []

    if "wifi_hints" in payload and payload.get("wifi_hints") is not None:
        wifi_payload = payload.get("wifi_hints") or {}
        if not isinstance(wifi_payload, dict):
            raise ValueError("wifi_hints must be an object")
        candidate["wifi_hints"] = dict(wifi_payload)

    if "outputs" in payload and payload.get("outputs") is not None:
        candidate["outputs"] = payload.get("outputs") or []

    if "schedules" in payload and payload.get("schedules") is not None:
        candidate["schedules"] = payload.get("schedules") or []

    if payload.get("mesh"):
        candidate["mesh"] = payload.get("mesh")

    if payload.get("mesh_summary"):
        candidate["mesh_summary"] = payload.get("mesh_summary")

    if payload.get("renogy_bt2"):
        candidate["renogy_bt2"] = payload.get("renogy_bt2")

    if payload.get("ads1263"):
        candidate["ads1263"] = payload.get("ads1263")

    if payload.get("display"):
        candidate["display"] = payload.get("display")

    if payload.get("simulation"):
        candidate["simulation"] = payload.get("simulation")

    # Validate/clamp (and enforce prod restrictions) before mutating live settings.
    try:
        validated = Settings.model_validate(candidate)
    except ValidationError as exc:
        raise ValueError(str(exc)) from exc

    for field in Settings.model_fields:
        setattr(settings, field, getattr(validated, field))

    return settings
