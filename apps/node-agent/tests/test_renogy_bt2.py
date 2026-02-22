from __future__ import annotations

import asyncio

import pytest

from app.config import RenogyBt2Config, Settings
from app.hardware.renogy_bt2 import (
    MODBUS_WRITE_MULTIPLE_REGISTERS,
    MODBUS_WRITE_SINGLE_REGISTER,
    RenogyBt2Collector,
    RenogyCRCError,
    RenogyModbusException,
    RenogyUnexpectedResponse,
    _bluez_device_path,
    _fallback_bluez_device,
    _RenogyBleSession,
    _append_crc,
    _build_write_multiple_request,
    _build_write_single_request,
    _candidate_pairs,
    _decode_metrics,
)


class _DummyClient:
    def __init__(self) -> None:
        self.writes: list[tuple[str, bytes, bool]] = []

    async def write_gatt_char(self, uuid: str, data: bytes, response: bool = True):
        self.writes.append((uuid, bytes(data), response))

    async def start_notify(self, _uuid: str, _callback):
        return None

    async def stop_notify(self, _uuid: str):
        return None


def test_modbus_crc_appends_expected_bytes():
    payload = bytes([0x01, 0x03, 0x00, 0x00, 0x00, 0x0A])
    frame = _append_crc(payload)
    assert frame == payload + bytes([0xC5, 0xCD])


def test_decode_metrics_maps_registers():
    runtime_regs = [
        75,  # battery_soc
        130,  # battery_voltage (13.0V)
        250,  # battery_current (2.5A)
        (30 << 8) | 25,  # temps: battery 25C, controller 30C
        120,  # load_voltage (12.0V)
        80,  # load_current (0.8A)
        500,  # load_power (W)
        200,  # pv_voltage (20.0V)
        150,  # pv_current (1.5A)
        600,  # pv_power (W)
    ] + [0] * 23

    # 0x0113 Charge watt-hours today (Wh) -> PV energy today (kWh)
    runtime_regs[19] = 1234
    # 0x011C..0x011D cumulative power generation (kWh, u32 big-endian)
    cumulative_kwh = 70_000
    runtime_regs[28] = (cumulative_kwh >> 16) & 0xFFFF
    runtime_regs[29] = cumulative_kwh & 0xFFFF
    settings_regs = [200, 0, 0]
    metrics = _decode_metrics(runtime_regs, settings_regs)
    assert metrics["battery_soc_percent"] == 75.0
    assert metrics["battery_voltage_v"] == 13.0
    assert metrics["battery_current_a"] == 2.5
    assert metrics["battery_temp_c"] == 25.0
    assert metrics["controller_temp_c"] == 30.0
    assert metrics["load_power_w"] == 500.0
    assert metrics["pv_voltage_v"] == 20.0
    assert metrics["pv_current_a"] == 1.5
    assert metrics["pv_power_w"] == 600.0
    assert metrics["pv_energy_today_kwh"] == pytest.approx(1.234)
    assert metrics["pv_energy_total_kwh"] == 70000.0
    assert metrics["runtime_hours"] > 3.8


def test_external_ingest_maps_fields():
    settings = Settings()
    settings.renogy_bt2 = RenogyBt2Config(
        enabled=True,
        mode="external",
        battery_capacity_ah=200,
    )
    collector = RenogyBt2Collector(settings)
    payload = {
        "pv_power": 420,
        "pv_voltage": 21.5,
        "pv_current": 1.95,
        "pv_energy_today_kwh": 0.5,
        "pv_energy_total_kwh": 12345,
        "battery_percentage": 80,
        "battery_voltage": 12.8,
        "battery_current": 3.2,
        "battery_temperature": 25,
        "controller_temperature": 31,
        "load_power": 120,
        "load_voltage": 12.0,
        "load_current": 1.0,
    }
    metrics = collector.ingest_payload(payload)
    assert metrics["pv_power_w"] == 420.0
    assert metrics["pv_energy_today_kwh"] == 0.5
    assert metrics["pv_energy_total_kwh"] == 12345.0
    assert metrics["battery_soc_percent"] == 80.0
    assert metrics["battery_voltage_v"] == 12.8
    assert metrics["load_power_w"] == 120.0
    assert metrics["runtime_hours"] > 0
    assert collector.read_metric("pv_power_w") == 420.0


def test_candidate_pairs_prefers_known_write_notify_combo():
    cfg = RenogyBt2Config(enabled=True, address="00:00:00:00:00:00")
    pairs = _candidate_pairs(
        config=cfg,
        available_writes=[
            "0000ffd3-0000-1000-8000-00805f9b34fb",
            "0000ffd1-0000-1000-8000-00805f9b34fb",
        ],
        available_notifies=[
            "0000ffd2-0000-1000-8000-00805f9b34fb",
            "0000fff1-0000-1000-8000-00805f9b34fb",
        ],
    )
    assert pairs[0] == (
        "0000FFD1-0000-1000-8000-00805F9B34FB",
        "0000FFF1-0000-1000-8000-00805F9B34FB",
    )


def test_build_write_single_register_request():
    payload = bytes([0x01, MODBUS_WRITE_SINGLE_REGISTER, 0xE0, 0x02, 0x12, 0x34])
    frame = _build_write_single_request(1, 0xE002, 0x1234)
    assert frame == _append_crc(payload)


def test_bluez_device_path_formatting():
    assert _bluez_device_path("10:ca:bf:aa:83:07", None) == "/org/bluez/hci0/dev_10_CA_BF_AA_83_07"
    assert _bluez_device_path("10:CA:BF:AA:83:07", "hci1") == "/org/bluez/hci1/dev_10_CA_BF_AA_83_07"


def test_fallback_bluez_device_returns_bledevice_when_available():
    device = _fallback_bluez_device("10:CA:BF:AA:83:07", "hci0", "renogy")
    assert device is not None
    assert device.address == "10:CA:BF:AA:83:07"
    assert device.name == "renogy"
    assert device.details["path"] == "/org/bluez/hci0/dev_10_CA_BF_AA_83_07"


def test_build_write_multiple_register_request():
    payload = bytes(
        [
            0x01,
            MODBUS_WRITE_MULTIPLE_REGISTERS,
            0xE0,
            0x02,
            0x00,
            0x02,
            0x04,
            0x12,
            0x34,
            0x00,
            0x02,
        ]
    )
    frame = _build_write_multiple_request(1, 0xE002, [0x1234, 0x0002])
    assert frame == _append_crc(payload)


def test_write_single_response_parsed():
    async def run():
        client = _DummyClient()
        session = _RenogyBleSession(
            client,
            unit_id=1,
            write_uuid="w",
            write_response=True,
            notify_uuid="n",
            timeout_seconds=1.0,
        )
        await session.start()
        task = asyncio.create_task(session.write_single_register(0xE002, 0x1234))
        await asyncio.sleep(0)
        frame = _append_crc(bytes([0x01, MODBUS_WRITE_SINGLE_REGISTER, 0xE0, 0x02, 0x12, 0x34]))
        session._on_notify(0, bytearray(frame))
        assert await task == 0x1234
        await session.stop()

    asyncio.run(run())


def test_write_single_exception_frame():
    async def run():
        client = _DummyClient()
        session = _RenogyBleSession(
            client,
            unit_id=1,
            write_uuid="w",
            write_response=True,
            notify_uuid="n",
            timeout_seconds=1.0,
        )
        await session.start()
        task = asyncio.create_task(session.write_single_register(0xE002, 0x0001))
        await asyncio.sleep(0)
        frame = _append_crc(bytes([0x01, MODBUS_WRITE_SINGLE_REGISTER | 0x80, 0x02]))
        session._on_notify(0, bytearray(frame))
        with pytest.raises(RenogyModbusException) as excinfo:
            await task
        assert excinfo.value.code == 0x02
        await session.stop()

    asyncio.run(run())


def test_write_response_address_mismatch():
    async def run():
        client = _DummyClient()
        session = _RenogyBleSession(
            client,
            unit_id=1,
            write_uuid="w",
            write_response=True,
            notify_uuid="n",
            timeout_seconds=1.0,
        )
        await session.start()
        task = asyncio.create_task(session.write_single_register(0xE002, 0x0001))
        await asyncio.sleep(0)
        frame = _append_crc(bytes([0x01, MODBUS_WRITE_SINGLE_REGISTER, 0xE0, 0x03, 0x00, 0x01]))
        session._on_notify(0, bytearray(frame))
        with pytest.raises(RenogyUnexpectedResponse):
            await task
        await session.stop()

    asyncio.run(run())


def test_write_response_crc_error():
    async def run():
        client = _DummyClient()
        session = _RenogyBleSession(
            client,
            unit_id=1,
            write_uuid="w",
            write_response=True,
            notify_uuid="n",
            timeout_seconds=1.0,
        )
        await session.start()
        task = asyncio.create_task(session.write_single_register(0xE002, 0x0001))
        await asyncio.sleep(0)
        frame = bytearray(
            _append_crc(
                bytes([0x01, MODBUS_WRITE_SINGLE_REGISTER, 0xE0, 0x02, 0x00, 0x01])
            )
        )
        frame[-1] ^= 0xFF
        session._on_notify(0, frame)
        with pytest.raises(RenogyCRCError):
            await task
        await session.stop()

    asyncio.run(run())
