from __future__ import annotations

import logging

from fastapi import APIRouter, Depends, HTTPException, Request, status

from app.auth import require_node_auth
from app.config import Settings, get_settings
from app.http_utils import apply_provisioning_request
from app.schemas import BluetoothProvisionRequest, ProvisioningSessionRequest
from app.services.provisioning import ProvisioningStore

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/v1", dependencies=[Depends(require_node_auth)])


def _provision_store(request: Request) -> ProvisioningStore:
    store: ProvisioningStore = getattr(request.app.state, "provision_store")
    return store


@router.post("/provision/bluetooth")
async def bluetooth_provision(
    request_body: BluetoothProvisionRequest,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    logger.info("Received Bluetooth provisioning request for %s", request_body.device_name)
    store = _provision_store(request)
    record = store.append(
        device_name=request_body.device_name,
        pin=request_body.pin,
        wifi_ssid=request_body.wifi_ssid,
        wifi_password=request_body.wifi_password,
        node_name=settings.node_name,
        adoption_token=request_body.adoption_token or settings.adoption_token,
        mesh_join_code=request_body.mesh_join_code,
        preferred_protocol=request_body.preferred_protocol,
        status="received",
        message="http_provision",
    )
    event = await apply_provisioning_request(
        request.app,
        request_body,
        settings,
        session_id=record.session_id,
    )
    return {
        "status": event.state,
        "message": event.message,
        "device": request_body.device_name,
        "session_id": record.session_id,
        "requested_at": record.requested_at,
    }


@router.post("/provisioning/session")
async def provisioning_session(
    request_body: ProvisioningSessionRequest,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    """Create or apply a provisioning session (for BLE token exchange or HTTP fallback)."""

    store = _provision_store(request)
    record = store.append(
        device_name=request_body.device_name,
        pin=request_body.pin,
        wifi_ssid=request_body.wifi_ssid,
        wifi_password=request_body.wifi_password,
        node_name=settings.node_name,
        adoption_token=request_body.adoption_token or settings.adoption_token,
        mesh_join_code=request_body.mesh_join_code,
        preferred_protocol=request_body.preferred_protocol,
        status="session_created",
        message="waiting_for_ble" if request_body.start_only else "applying",
    )
    if request_body.start_only:
        return {
            "status": "session_created",
            "message": "Session staged; ready for BLE handshake",
            "session_id": record.session_id,
            "device": record.device_name,
        }

    event = await apply_provisioning_request(
        request.app,
        request_body,
        settings,
        session_id=record.session_id,
    )
    return {
        "status": event.state,
        "message": event.message,
        "session_id": record.session_id,
        "device": request_body.device_name,
    }

def _sanitize_record(record) -> dict:
    data = dict(record.__dict__)
    data.pop("wifi_password", None)
    data.pop("adoption_token", None)
    return data


@router.get("/provision/queue")
async def provisioning_queue(request: Request):
    store = _provision_store(request)
    return {"pending": [_sanitize_record(record) for record in store.all()]}


@router.delete("/provision/queue")
async def clear_provisioning_queue(request: Request):
    store = _provision_store(request)
    store.clear()
    return {"status": "cleared"}


@router.get("/provisioning/sessions")
async def provisioning_sessions(request: Request):
    store = _provision_store(request)
    return {"sessions": [_sanitize_record(record) for record in store.all()]}


@router.get("/provisioning/session/{session_id}")
async def provisioning_session_status(session_id: str, request: Request):
    store = _provision_store(request)
    record = store.get(session_id)
    if not record:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Session not found")
    return _sanitize_record(record)
