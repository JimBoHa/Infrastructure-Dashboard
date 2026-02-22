from __future__ import annotations

import asyncio
import logging
from typing import Dict

from fastapi import APIRouter, Depends, HTTPException, Query, Request, status

from app.config import Settings, get_settings
from app.hardware import RenogyBt2Collector
from app.hardware.renogy_bt2 import (
    SETTINGS_REG_COUNT,
    SETTINGS_REG_START,
    RenogyCRCError,
    RenogyModbusException,
    RenogyUnexpectedResponse,
    RenogyVerificationError,
)
from app.http_utils import validate_renogy_ingest_token
from app.schemas import (
    RenogyApplyRequest,
    RenogyApplyResponse,
    RenogyApplyResult,
    RenogyErrorDetail,
    RenogyIngestResponse,
    RenogySettingsReadResponse,
)

router = APIRouter(prefix="/v1")
logger = logging.getLogger(__name__)


def _collector_or_error(request: Request, settings: Settings) -> RenogyBt2Collector:
    config = settings.renogy_bt2
    if not config.enabled:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Renogy BT-2 disabled",
        )
    if config.mode != "ble":
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Renogy BT-2 BLE mode required for settings access",
        )
    renogy: RenogyBt2Collector | None = getattr(request.app.state, "renogy", None)
    if not renogy:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Renogy collector unavailable",
        )
    return renogy


def _format_error(exc: Exception) -> RenogyErrorDetail:
    if isinstance(exc, asyncio.TimeoutError):
        return RenogyErrorDetail(type="timeout", message="Renogy BT-2 request timed out")
    if isinstance(exc, RenogyCRCError):
        return RenogyErrorDetail(type="crc", message=str(exc))
    if isinstance(exc, RenogyModbusException):
        return RenogyErrorDetail(type="modbus_exception", code=exc.code, message=str(exc))
    if isinstance(exc, RenogyVerificationError):
        return RenogyErrorDetail(type="verification_failed", message=str(exc))
    if isinstance(exc, RenogyUnexpectedResponse):
        return RenogyErrorDetail(type="unexpected_response", message=str(exc))
    if isinstance(exc, RuntimeError):
        return RenogyErrorDetail(type="runtime_error", message=str(exc))
    return RenogyErrorDetail(type="unknown_error", message=str(exc))


@router.post("/renogy-bt", response_model=RenogyIngestResponse)
async def renogy_bt_ingest(
    payload: Dict[str, object],
    request: Request,
    settings: Settings = Depends(get_settings),
):
    config = settings.renogy_bt2
    if not config.enabled or config.mode != "external":
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Renogy external ingest disabled",
        )
    validate_renogy_ingest_token(request, config)
    renogy: RenogyBt2Collector | None = getattr(request.app.state, "renogy", None)
    if not renogy:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Renogy collector unavailable",
        )
    metrics = renogy.ingest_payload(payload)
    if not metrics:
        return RenogyIngestResponse(status="ignored", metrics=[])
    return RenogyIngestResponse(status="ok", metrics=sorted(metrics.keys()))


@router.get("/renogy-bt/settings", response_model=RenogySettingsReadResponse)
async def renogy_bt_settings(
    request: Request,
    settings: Settings = Depends(get_settings),
    start_address: int = Query(SETTINGS_REG_START, ge=0, le=0xFFFF),
    count: int = Query(SETTINGS_REG_COUNT, ge=1, le=64),
):
    renogy = _collector_or_error(request, settings)
    try:
        registers = await renogy.read_settings_block(start_address, count)
        return RenogySettingsReadResponse(
            status="ok",
            start_address=start_address,
            count=len(registers),
            registers=registers,
        )
    except Exception as exc:
        logger.warning("Renogy settings read failed: %s", exc)
        error = _format_error(exc)
        return RenogySettingsReadResponse(
            status="error",
            start_address=start_address,
            count=count,
            registers=[],
            error=error,
        )


@router.post("/renogy-bt/settings/apply", response_model=RenogyApplyResponse)
async def renogy_bt_apply_settings(
    payload: RenogyApplyRequest,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    if not payload.writes:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="No writes specified",
        )
    renogy = _collector_or_error(request, settings)
    writes = [(write.address, write.values) for write in payload.writes]
    try:
        results = await renogy.apply_settings(writes, verify=payload.verify)
        applied = [
            RenogyApplyResult(
                address=item["address"],
                values=list(item.get("values", [])),
                read_back=item.get("read_back"),
            )
            for item in results
        ]
        return RenogyApplyResponse(status="ok", applied=applied)
    except Exception as exc:
        logger.warning("Renogy settings apply failed: %s", exc)
        error = _format_error(exc)
        return RenogyApplyResponse(status="error", applied=[], error=error)
