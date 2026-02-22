from __future__ import annotations

from fastapi import APIRouter, Depends, HTTPException, Request, status

from app.config import Settings, SimulationProfile, get_settings
from app.http_utils import ensure_simulator, mesh_adapter, persist
from app.schemas import SimulationUpdatePayload
from app.services.publisher import TelemetryPublisher

router = APIRouter(prefix="/v1")


@router.get("/simulation")
async def simulation_profile(settings: Settings = Depends(get_settings)):
    return settings.simulation.model_dump()


def _update_simulation_profile(
    request: Request,
    settings: Settings,
    payload: SimulationUpdatePayload,
) -> dict[str, object]:
    merged = settings.simulation.model_dump()
    merged.update(payload.model_dump(exclude_none=True))
    settings.simulation = SimulationProfile.model_validate(merged)
    sim = ensure_simulator(request.app, settings)
    if sim:
        sim.update_profile(settings.simulation, seed_hint=settings.node_id)
    mesh = mesh_adapter(request.app)
    if mesh:
        mesh.update_simulation(settings.simulation)
    persist(request.app, settings)
    publisher: TelemetryPublisher | None = getattr(request.app.state, "publisher", None)
    if publisher:
        publisher.simulator = sim
    return settings.simulation.model_dump()


@router.put("/simulation")
async def update_simulation(
    payload: SimulationUpdatePayload,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    if not settings.simulation.enabled:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Simulation disabled")
    return _update_simulation_profile(request, settings, payload)

