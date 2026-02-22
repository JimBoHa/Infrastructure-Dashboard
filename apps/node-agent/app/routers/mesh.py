from __future__ import annotations

from typing import Dict

from fastapi import APIRouter, Depends, HTTPException, Request, status

from app.config import Settings, get_settings
from app.http_utils import mesh_adapter
from app.schemas import MeshJoinRequest, MeshRemoveRequest

router = APIRouter(prefix="/v1")


@router.get("/mesh")
async def mesh_status(request: Request, settings: Settings = Depends(get_settings)) -> Dict[str, object]:
    mesh = mesh_adapter(request.app)
    topology = mesh.topology_snapshot() if mesh else []
    return {
        "enabled": settings.mesh.enabled,
        "driver": settings.mesh.driver,
        "protocol": settings.mesh.protocol,
        "config": settings.mesh.model_dump(),
        "summary": settings.mesh_summary.model_dump(),
        "topology": topology,
        "simulation": settings.simulation.model_dump(),
    }


@router.post("/mesh/join")
async def mesh_join(
    request_body: MeshJoinRequest,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    mesh = mesh_adapter(request.app)
    if not mesh or not mesh.enabled:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Mesh adapter disabled")
    permitted = await mesh.start_join(request_body.duration_seconds)
    return {
        "status": "permitting" if permitted else "error",
        "summary": mesh.snapshot_summary(),
    }


@router.post("/mesh/remove")
async def mesh_remove(request_body: MeshRemoveRequest, request: Request):
    mesh = mesh_adapter(request.app)
    if not mesh or not mesh.enabled:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Mesh adapter disabled")
    removed = await mesh.remove_device(request_body.ieee)
    return {
        "status": "removed" if removed else "error",
        "summary": mesh.snapshot_summary(),
    }

