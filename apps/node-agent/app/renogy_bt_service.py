"""Systemd service entrypoint for Renogy BT-2 collection (renogy-bt.service).

This runs as a separate process from the FastAPI node-agent service. It polls the
Renogy BT-2 over BLE and POSTs the raw Renogy payload shape into the node-agent
external ingest endpoint (`/v1/renogy-bt`).

Rationale:
- Keeps BLE I/O off the node-agent HTTP/MQTT event loop.
- Allows enabling/disabling Renogy collection per-node while shipping a single
  generic Pi 5 software stack.
"""

from __future__ import annotations

import asyncio
import logging
import signal
from typing import Any

import httpx

from app.config import get_settings
from app.hardware.renogy_bt2 import (
    BleakClient,
    BleakScanner,
    RENOGY_BT_FIELD_MAP,
    RUNTIME_REG_COUNT,
    RUNTIME_REG_START,
    SETTINGS_REG_COUNT,
    SETTINGS_REG_START,
    RenogyBt2Collector,
    _decode_metrics,
    _RenogyBleSession,
)
from app.services.config_store import ConfigStore, apply_config

logger = logging.getLogger(__name__)


def _reverse_field_map() -> dict[str, str]:
    reverse: dict[str, str] = {}
    for source_key, dest_key in RENOGY_BT_FIELD_MAP.items():
        reverse[dest_key] = source_key
    return reverse


DEST_TO_SOURCE = _reverse_field_map()


def _metrics_to_payload(metrics: dict[str, float]) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for dest_key, value in metrics.items():
        if dest_key == "runtime_hours":
            payload["runtime_hours"] = value
            continue
        source_key = DEST_TO_SOURCE.get(dest_key)
        if source_key:
            payload[source_key] = value
    return payload


async def _post_payload(
    client: httpx.AsyncClient,
    *,
    url: str,
    token: str,
    payload: dict[str, Any],
) -> None:
    headers = {"Authorization": f"Bearer {token}"}
    response = await client.post(url, json=payload, headers=headers)
    response.raise_for_status()


async def _find_device(collector: RenogyBt2Collector):
    cfg = collector.config
    if BleakScanner is None:
        raise RuntimeError("bleak not available")

    if cfg.adapter:
        devices = await BleakScanner.discover(
            timeout=cfg.connect_timeout_seconds,
            bluez={"adapter": cfg.adapter},
        )
        return collector._select_device(devices, cfg.address, cfg.device_name)

    device = None
    if cfg.address:
        device = await BleakScanner.find_device_by_address(
            cfg.address,
            timeout=cfg.connect_timeout_seconds,
        )
    if device is None and cfg.device_name:
        device = await BleakScanner.find_device_by_name(
            cfg.device_name,
            timeout=cfg.connect_timeout_seconds,
        )
    return device


async def _run_ble_session(
    *,
    collector: RenogyBt2Collector,
    client: httpx.AsyncClient,
    ingest_url: str,
    token: str,
    stop: asyncio.Event,
) -> None:
    if BleakClient is None:
        raise RuntimeError("bleak not available")

    device = await _find_device(collector)
    if device is None:
        raise RuntimeError("Renogy BT-2 device not found")

    cfg = collector.config
    async with BleakClient(device) as ble_client:
        if not ble_client.is_connected:
            raise RuntimeError("Renogy BT-2 connection failed")
        write_uuid, notify_uuid, write_response = await collector._resolve_characteristics(ble_client)
        session = _RenogyBleSession(
            ble_client,
            unit_id=cfg.unit_id,
            write_uuid=write_uuid,
            write_response=write_response,
            notify_uuid=notify_uuid,
            timeout_seconds=cfg.request_timeout_seconds,
        )
        await session.start()
        try:
            while not stop.is_set():
                runtime = await session.read_holding_registers(RUNTIME_REG_START, RUNTIME_REG_COUNT)
                settings_regs = await session.read_holding_registers(
                    SETTINGS_REG_START, SETTINGS_REG_COUNT
                )
                metrics = _decode_metrics(runtime, settings_regs)
                payload = _metrics_to_payload(metrics)
                await _post_payload(client, url=ingest_url, token=token, payload=payload)
                try:
                    await asyncio.wait_for(stop.wait(), timeout=float(cfg.poll_interval_seconds))
                except asyncio.TimeoutError:
                    continue
        finally:
            await session.stop()


async def _runner() -> int:
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    settings = get_settings()
    store = ConfigStore(settings.config_file)
    persisted = store.load()
    if persisted:
        apply_config(settings, persisted)
    cfg = settings.renogy_bt2

    if not cfg.enabled:
        logger.info("renogy-bt: disabled; exiting")
        return 0
    if cfg.mode != "external":
        logger.info("renogy-bt: mode=%s (expected external); exiting", cfg.mode)
        return 0
    if not cfg.ingest_token:
        logger.error("renogy-bt: NODE_RENOGY_BT2_INGEST_TOKEN is required in external mode")
        return 2
    if BleakClient is None or BleakScanner is None:
        logger.error("renogy-bt: bleak not available on this platform")
        return 2

    ingest_url = f"http://127.0.0.1:{settings.advertise_port}/v1/renogy-bt"
    token = cfg.ingest_token

    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        try:
            loop.add_signal_handler(sig, stop.set)
        except NotImplementedError:
            pass

    timeout = httpx.Timeout(
        connect=cfg.connect_timeout_seconds,
        read=cfg.request_timeout_seconds,
        write=cfg.request_timeout_seconds,
        pool=cfg.request_timeout_seconds,
    )
    async with httpx.AsyncClient(timeout=timeout) as client:
        backoff_seconds = 5.0
        while not stop.is_set():
            try:
                await _run_ble_session(
                    collector=RenogyBt2Collector(settings),
                    client=client,
                    ingest_url=ingest_url,
                    token=token,
                    stop=stop,
                )
                backoff_seconds = 5.0
            except asyncio.CancelledError:
                break
            except Exception as exc:
                logger.warning("renogy-bt: poll error: %s", exc)
                backoff_seconds = min(backoff_seconds * 2, 60.0)

            wait_for = cfg.poll_interval_seconds if backoff_seconds == 5.0 else backoff_seconds
            try:
                await asyncio.wait_for(stop.wait(), timeout=float(wait_for))
            except asyncio.TimeoutError:
                continue
    return 0


def main() -> None:
    raise SystemExit(asyncio.run(_runner()))


if __name__ == "__main__":  # pragma: no cover
    main()
