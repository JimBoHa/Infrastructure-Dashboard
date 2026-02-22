from __future__ import annotations

from fastapi import APIRouter, Depends, Request
from fastapi.responses import HTMLResponse

from app.config import Settings, get_settings
from app.ui import render_landing_page

router = APIRouter()


@router.get("/", response_class=HTMLResponse)
async def landing(request: Request, settings: Settings = Depends(get_settings)):
    ble_available = False
    try:
        ble_manager = getattr(request.app.state, "ble_provisioning", None)
        ble_available = bool(ble_manager and getattr(ble_manager, "available", False))
    except Exception:
        ble_available = False
    return HTMLResponse(render_landing_page(settings, ble_available=ble_available))


@router.get("/healthz")
async def healthz():
    return {"status": "ok"}

