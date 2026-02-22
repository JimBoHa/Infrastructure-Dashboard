from __future__ import annotations

import asyncio
import logging
import shutil
import subprocess
import time
from typing import Dict, List, Optional

from fastapi import HTTPException, Request, status
from fastapi.applications import FastAPI

from app.config import RenogyBt2Config, Settings
from app.hardware import MeshAdapter
from app.services.config_store import export_config
from app.services.discovery import DiscoveryAdvertiser
from app.services.output_listener import OutputCommandListener
from app.services.provisioning import ProvisioningStore
from app.services.publisher import TelemetryPublisher
from app.services.simulator import SimulatedDevice
from app.services.ble_provisioning import ProvisioningEvent

logger = logging.getLogger(__name__)


def apply_wifi_credentials(ssid: Optional[str], password: Optional[str]) -> Dict[str, object]:
    """Attempt to apply Wi-Fi credentials via common CLI tools."""

    timestamp = time.time()
    if not ssid:
        return {"state": "skipped", "message": "missing_ssid", "timestamp": timestamp}

    nmcli = shutil.which("nmcli")
    wpa_cli = shutil.which("wpa_cli")
    if not nmcli and not wpa_cli:
        return {
            "state": "stored",
            "message": "nmcli/wpa_cli unavailable; persisted only",
            "timestamp": timestamp,
        }

    def _run(cmd: List[str]) -> tuple[int, str]:
        proc = subprocess.run(cmd, capture_output=True, text=True)
        out = (proc.stdout or proc.stderr or "").strip()
        return proc.returncode, out

    if nmcli:
        cmd = ["nmcli", "dev", "wifi", "connect", ssid]
        if password:
            cmd += ["password", password]
        rc, out = _run(cmd)
        return {
            "state": "applied" if rc == 0 else "error",
            "message": out or ("nmcli exit " + str(rc)),
            "timestamp": timestamp,
            "method": "nmcli",
        }

    cmd = ["wpa_cli", "add_network"]
    rc, out = _run(cmd)
    network_id = out.strip() if rc == 0 else None
    if rc == 0 and network_id and network_id.isdigit():
        _run(["wpa_cli", "set_network", network_id, "ssid", f'"{ssid}"'])
        if password:
            _run(["wpa_cli", "set_network", network_id, "psk", f'"{password}"'])
        _run(["wpa_cli", "enable_network", network_id])
        return {
            "state": "applied",
            "message": "wpa_cli configured",
            "timestamp": timestamp,
            "method": "wpa_cli",
        }

    return {
        "state": "error",
        "message": out or "wpa_cli failed",
        "timestamp": timestamp,
        "method": "wpa_cli",
    }


async def _apply_wifi_credentials_background(
    app: FastAPI,
    settings: Settings,
    *,
    ssid: str,
    password: Optional[str],
    session_id: Optional[str],
    provision_store: Optional[ProvisioningStore],
) -> None:
    """Run Wi-Fi apply work without blocking the asyncio event loop."""

    lock: asyncio.Lock = getattr(app.state, "wifi_apply_lock", None)  # type: ignore[assignment]
    if lock is None:
        lock = asyncio.Lock()
        app.state.wifi_apply_lock = lock

    runner = getattr(app.state, "wifi_apply_runner", None)
    if runner is None:
        runner = apply_wifi_credentials

    if session_id and provision_store:
        provision_store.update_status(session_id, "wifi_applying", "Applying Wi-Fi credentials")

    async with lock:
        try:
            result = await asyncio.to_thread(runner, ssid, password)
        except Exception as exc:  # pragma: no cover (defensive)
            logger.exception("Wi-Fi credential apply failed")
            result = {
                "state": "error",
                "message": f"wifi_apply_exception: {exc}",
                "timestamp": time.time(),
            }

    try:
        wifi_hints = settings.wifi_hints or {}
        if isinstance(wifi_hints, dict):
            wifi_hints["apply_status"] = result
            settings.wifi_hints = wifi_hints
        persist(app, settings)
    except Exception:  # pragma: no cover (defensive; persistence failures shouldn't crash the service)
        logger.exception("Failed to persist Wi-Fi apply status")

    if session_id and provision_store:
        provision_store.update_status(
            session_id,
            "wifi_applied" if result.get("state") == "applied" else "wifi_error",
            str(result.get("message") or ""),
        )


def persist(app: FastAPI, settings: Settings) -> None:
    store = getattr(app.state, "config_store")
    store.save(export_config(settings))


def simulator(app: FastAPI) -> SimulatedDevice | None:
    return getattr(app.state, "simulator", None)


def mesh_adapter(app: FastAPI) -> MeshAdapter | None:
    mesh: MeshAdapter | None = getattr(app.state, "mesh", None)
    return mesh


def ensure_simulator(app: FastAPI, settings: Settings) -> SimulatedDevice | None:
    sim = simulator(app)
    if sim:
        return sim
    if not settings.simulation.enabled:
        return None
    sim = SimulatedDevice(settings.simulation, seed_hint=settings.node_id)
    app.state.simulator = sim
    publisher: TelemetryPublisher | None = getattr(app.state, "publisher", None)
    if publisher:
        publisher.simulator = sim
    if settings.outputs and not getattr(app.state, "command_listener", None):
        listener = OutputCommandListener(settings, simulator=sim)
        listener.start()
        app.state.command_listener = listener
    return sim


async def apply_provisioning_request(
    app: FastAPI,
    request: "BluetoothProvisionRequest",
    settings: Settings,
    *,
    session_id: Optional[str] = None,
) -> ProvisioningEvent:
    """Apply a provisioning request immediately and persist settings."""

    provision_store: ProvisioningStore | None = None
    try:
        provision_store = getattr(app.state, "provision_store", None)
    except Exception:
        provision_store = None

    if session_id and provision_store:
        provision_store.update_status(session_id, "applying", "Applying provisioning payload")

    changed = False
    wifi_apply_scheduled = False
    if request.device_name and settings.node_name != request.device_name:
        settings.node_name = request.device_name
        changed = True
    if request.wifi_ssid:
        settings.wifi_hints = {
            "ssid": request.wifi_ssid,
            "password": request.wifi_password,
        }
        settings.wifi_hints["apply_status"] = {
            "state": "queued",
            "message": "pending",
            "timestamp": time.time(),
        }
        changed = True
        wifi_apply_scheduled = True
    if request.adoption_token and settings.adoption_token != request.adoption_token:
        settings.adoption_token = request.adoption_token
        changed = True
    if request.preferred_protocol and settings.mesh.protocol != request.preferred_protocol:
        settings.mesh.protocol = request.preferred_protocol
        changed = True

    if changed:
        persist(app, settings)
        advertiser: DiscoveryAdvertiser | None = getattr(app.state, "advertiser", None)
        if advertiser:
            try:
                await advertiser.stop()
                await advertiser.start()
            except Exception:
                logger.exception("Failed to restart discovery advertiser after provisioning")

    if wifi_apply_scheduled and request.wifi_ssid:
        if session_id and provision_store:
            provision_store.update_status(session_id, "wifi_queued", "Wi-Fi apply queued")
        asyncio.create_task(
            _apply_wifi_credentials_background(
                app,
                settings,
                ssid=request.wifi_ssid,
                password=request.wifi_password,
                session_id=session_id,
                provision_store=provision_store,
            )
        )
        event = ProvisioningEvent.now("in_progress", "Wi-Fi apply queued")
    else:
        event = ProvisioningEvent.now("provisioned", "Provisioning applied")
    if session_id and provision_store:
        provision_store.update_status(session_id, event.state, event.message)
    return event


def validate_renogy_ingest_token(request: Request, config: RenogyBt2Config) -> None:
    if not config.ingest_token:
        return
    auth_header = request.headers.get("Authorization") or ""
    if not auth_header.startswith("Bearer "):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing bearer token",
        )
    token = auth_header.removeprefix("Bearer ").strip()
    if token != config.ingest_token:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Invalid token",
        )


# Lazy import: keep schemas decoupled from HTTP utilities.
from app.schemas import BluetoothProvisionRequest  # noqa: E402  (import after defs)
