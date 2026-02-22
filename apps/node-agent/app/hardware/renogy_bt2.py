"""Renogy BT-2 BLE collector for Rover charge controllers."""
from __future__ import annotations

import asyncio
import logging
from contextlib import asynccontextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import AsyncIterator, Optional

from app.config import RenogyBt2Config

logger = logging.getLogger(__name__)

KNOWN_SERVICE_UUIDS = [
    "0000FFE0-0000-1000-8000-00805F9B34FB",
    "0000FFF0-0000-1000-8000-00805F9B34FB",
]

KNOWN_CHAR_UUIDS = [
    "0000FFE1-0000-1000-8000-00805F9B34FB",
    "0000FFF1-0000-1000-8000-00805F9B34FB",
    "0000FFF2-0000-1000-8000-00805F9B34FB",
]

PREFERRED_NOTIFY_UUIDS = [
    "0000FFF1-0000-1000-8000-00805F9B34FB",
    "0000FFE1-0000-1000-8000-00805F9B34FB",
    "0000FFF2-0000-1000-8000-00805F9B34FB",
    "0000FFD2-0000-1000-8000-00805F9B34FB",
]

PREFERRED_WRITE_UUIDS = [
    "0000FFD1-0000-1000-8000-00805F9B34FB",
    "0000FFE1-0000-1000-8000-00805F9B34FB",
    "F000FFD1-0451-4000-B000-000000000000",
]

MODBUS_READ_HOLDING_REGS = 0x03
MODBUS_WRITE_SINGLE_REGISTER = 0x06
MODBUS_WRITE_MULTIPLE_REGISTERS = 0x10
RUNTIME_REG_START = 0x0100
RUNTIME_REG_COUNT = 33
SETTINGS_REG_START = 0xE002
SETTINGS_REG_COUNT = 3

RENOGY_BT_FIELD_MAP = {
    "pv_power": "pv_power_w",
    "pv_voltage": "pv_voltage_v",
    "pv_current": "pv_current_a",
    # Energy counters are derived from the runtime register block.
    # In external ingest mode, the renogy-bt sidecar posts these keys verbatim.
    "pv_energy_today_kwh": "pv_energy_today_kwh",
    "pv_energy_total_kwh": "pv_energy_total_kwh",
    "battery_percentage": "battery_soc_percent",
    "battery_voltage": "battery_voltage_v",
    "battery_current": "battery_current_a",
    "battery_temperature": "battery_temp_c",
    "controller_temperature": "controller_temp_c",
    "load_power": "load_power_w",
    "load_voltage": "load_voltage_v",
    "load_current": "load_current_a",
}


try:  # pragma: no cover - optional on Python 3.14+
    from bleak import BleakClient, BleakScanner  # type: ignore
    from bleak.backends.device import BLEDevice  # type: ignore
except Exception:  # pragma: no cover - handled in runtime
    BleakClient = None  # type: ignore[assignment]
    BleakScanner = None  # type: ignore[assignment]
    BLEDevice = None  # type: ignore[assignment]


@dataclass
class RenogyBt2Snapshot:
    metrics: dict[str, float]
    updated_at: datetime | None = None


@dataclass
class _PendingRequest:
    function: int
    address: int
    count: int


class RenogyModbusException(Exception):
    def __init__(self, code: int) -> None:
        self.code = code
        super().__init__(f"Renogy Modbus exception {code}")


class RenogyCRCError(Exception):
    """Raised when a Modbus frame fails CRC validation."""


class RenogyUnexpectedResponse(Exception):
    """Raised when a Modbus response does not match the pending request."""


class RenogyVerificationError(Exception):
    def __init__(self, *, address: int, expected: list[int], actual: list[int]) -> None:
        self.address = address
        self.expected = expected
        self.actual = actual
        super().__init__(f"Verification failed at 0x{address:04X}: expected {expected}, got {actual}")


def _modbus_crc(data: bytes) -> int:
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc & 0xFFFF


def _append_crc(frame: bytes) -> bytes:
    crc = _modbus_crc(frame)
    return frame + bytes([crc & 0xFF, (crc >> 8) & 0xFF])


def _crc_ok(frame: bytes) -> bool:
    if len(frame) < 3:
        return False
    expected = _modbus_crc(frame[:-2])
    actual = frame[-2] | (frame[-1] << 8)
    return expected == actual


def _build_read_holding_request(unit_id: int, address: int, count: int) -> bytes:
    payload = bytes(
        [
            unit_id & 0xFF,
            MODBUS_READ_HOLDING_REGS,
            (address >> 8) & 0xFF,
            address & 0xFF,
            (count >> 8) & 0xFF,
            count & 0xFF,
        ]
    )
    return _append_crc(payload)


def _build_write_single_request(unit_id: int, address: int, value: int) -> bytes:
    payload = bytes(
        [
            unit_id & 0xFF,
            MODBUS_WRITE_SINGLE_REGISTER,
            (address >> 8) & 0xFF,
            address & 0xFF,
            (value >> 8) & 0xFF,
            value & 0xFF,
        ]
    )
    return _append_crc(payload)


def _build_write_multiple_request(unit_id: int, address: int, values: list[int]) -> bytes:
    count = len(values)
    byte_count = count * 2
    payload = bytearray(
        [
            unit_id & 0xFF,
            MODBUS_WRITE_MULTIPLE_REGISTERS,
            (address >> 8) & 0xFF,
            address & 0xFF,
            (count >> 8) & 0xFF,
            count & 0xFF,
            byte_count & 0xFF,
        ]
    )
    for value in values:
        payload.append((value >> 8) & 0xFF)
        payload.append(value & 0xFF)
    return _append_crc(bytes(payload))


def _decode_temperature_byte(value: int) -> float:
    raw = value & 0xFF
    if raw >= 128:
        return float(-(raw - 128))
    return float(raw)


def _decode_temperature_word(word: int) -> tuple[float, float]:
    battery_raw = word & 0xFF
    controller_raw = (word >> 8) & 0xFF
    return _decode_temperature_byte(battery_raw), _decode_temperature_byte(controller_raw)


def _decode_signed_16(value: int) -> int:
    value &= 0xFFFF
    if value >= 0x8000:
        return value - 0x10000
    return value


def _decode_u32_be(high_word: int, low_word: int) -> int:
    return ((high_word & 0xFFFF) << 16) | (low_word & 0xFFFF)


def _estimate_runtime_hours(
    *,
    battery_capacity_ah: int,
    battery_voltage: float,
    battery_soc: float,
    load_power_w: float,
) -> float | None:
    if load_power_w <= 0:
        return None
    if battery_capacity_ah <= 0 or battery_voltage <= 0:
        return None
    if battery_soc <= 0:
        return 0.0
    remaining_wh = battery_capacity_ah * battery_voltage * (battery_soc / 100.0)
    return remaining_wh / load_power_w


def _coerce_float(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value.strip())
        except ValueError:
            return None
    return None


def _decode_metrics(runtime_regs: list[int], settings_regs: list[int]) -> dict[str, float]:
    if len(runtime_regs) < 10:
        raise ValueError("Renogy runtime registers missing required values")
    battery_soc = float(runtime_regs[0])
    battery_voltage = float(runtime_regs[1]) / 10.0
    battery_current = float(_decode_signed_16(runtime_regs[2])) / 100.0
    battery_temp_c, controller_temp_c = _decode_temperature_word(runtime_regs[3])
    load_voltage = float(runtime_regs[4]) / 10.0
    load_current = float(_decode_signed_16(runtime_regs[5])) / 100.0
    load_power_w = float(runtime_regs[6])
    pv_voltage = float(runtime_regs[7]) / 10.0
    pv_current = float(_decode_signed_16(runtime_regs[8])) / 100.0
    pv_power_w = float(runtime_regs[9])

    battery_capacity_ah = int(settings_regs[0]) if settings_regs else 0
    runtime_hours = _estimate_runtime_hours(
        battery_capacity_ah=battery_capacity_ah,
        battery_voltage=battery_voltage,
        battery_soc=battery_soc,
        load_power_w=load_power_w,
    )

    metrics: dict[str, float] = {
        "pv_power_w": pv_power_w,
        "pv_voltage_v": pv_voltage,
        "pv_current_a": pv_current,
        "battery_soc_percent": battery_soc,
        "battery_voltage_v": battery_voltage,
        "battery_current_a": battery_current,
        "battery_temp_c": battery_temp_c,
        "controller_temp_c": controller_temp_c,
        "load_power_w": load_power_w,
        "load_voltage_v": load_voltage,
        "load_current_a": load_current,
    }

    # Energy counters (Rover Modbus register map):
    # - 0x0113: Charge Watt-hours Today (Wh)
    # - 0x011C..0x011D: Cumulative power generation (kWh, u32 big-endian)
    # We include these as PV energy sensors because they match the Renogy app totals and
    # avoid accumulating errors vs. integrating instantaneous PV power.
    if len(runtime_regs) > 19:
        # 0x0113: Charge watt-hours today (Wh)
        charge_wh_today = float(runtime_regs[19])
        metrics["pv_energy_today_kwh"] = charge_wh_today / 1000.0

    if len(runtime_regs) > 29:
        # 0x011C..0x011D: Cumulative power generation (kWh, u32 big-endian)
        metrics["pv_energy_total_kwh"] = float(_decode_u32_be(runtime_regs[28], runtime_regs[29]))
    if runtime_hours is not None:
        metrics["runtime_hours"] = float(runtime_hours)
    return metrics


def _normalize_uuid(value: str) -> str:
    return value.strip().upper()


def _bluez_device_path(address: str, adapter: str | None) -> str:
    adapter_name = (adapter or "hci0").strip() or "hci0"
    address_key = address.strip().upper().replace(":", "_")
    return f"/org/bluez/{adapter_name}/dev_{address_key}"


def _fallback_bluez_device(
    address: str,
    adapter: str | None,
    name: str | None = None,
) -> "BLEDevice" | None:
    """Best-effort fallback when BleakScanner cannot discover an already-known device.

    BlueZ keeps a stable D-Bus object path for previously discovered devices:
    `/org/bluez/<hci>/dev_<AA_BB_CC...>`.
    If the Renogy BT-2 is connected (or recently discovered) but not advertising,
    `BleakScanner.find_device_by_address()` may return `None`. In that case, we can still
    attempt a connection by constructing a BLEDevice with the known BlueZ path.
    """

    if BLEDevice is None:
        return None
    if not address:
        return None
    path = _bluez_device_path(address, adapter)
    details = {"path": path}
    return BLEDevice(address, name, details, 0)


def _sort_by_preference(values: list[str], preferred: list[str]) -> list[str]:
    preferred_normalized = [_normalize_uuid(v) for v in preferred]
    ranks = {uuid: idx for idx, uuid in enumerate(preferred_normalized)}

    def sort_key(uuid: str) -> tuple[int, str]:
        normalized = _normalize_uuid(uuid)
        return (ranks.get(normalized, len(preferred_normalized)), normalized)

    return sorted({_normalize_uuid(v) for v in values}, key=sort_key)


def _candidate_pairs(
    *,
    config: RenogyBt2Config,
    available_writes: list[str],
    available_notifies: list[str],
) -> list[tuple[str, str]]:
    preferred_writes = []
    if config.write_uuid:
        preferred_writes.append(config.write_uuid)
    preferred_writes.extend(PREFERRED_WRITE_UUIDS)

    preferred_notifies = []
    if config.notify_uuid:
        preferred_notifies.append(config.notify_uuid)
    preferred_notifies.extend(PREFERRED_NOTIFY_UUIDS)

    write_order = _sort_by_preference(available_writes, preferred_writes)
    notify_order = _sort_by_preference(available_notifies, preferred_notifies)

    pairs: list[tuple[str, str]] = []
    for write_uuid in write_order:
        for notify_uuid in notify_order:
            pairs.append((write_uuid, notify_uuid))
    return pairs


class _RenogyBleSession:
    def __init__(
        self,
        client: "BleakClient",
        *,
        unit_id: int,
        write_uuid: str,
        write_response: bool,
        notify_uuid: str,
        timeout_seconds: float,
    ) -> None:
        self._client = client
        self._unit_id = unit_id
        self._write_uuid = write_uuid
        self._write_response = write_response
        self._notify_uuid = notify_uuid
        self._timeout_seconds = timeout_seconds
        self._buffer = bytearray()
        self._lock = asyncio.Lock()
        self._future: asyncio.Future[object] | None = None
        self._pending: _PendingRequest | None = None

    async def start(self) -> None:
        await self._client.start_notify(self._notify_uuid, self._on_notify)

    async def stop(self) -> None:
        try:
            await self._client.stop_notify(self._notify_uuid)
        except Exception as exc:  # pragma: no cover - depends on bluez/dbus behavior
            logger.debug("Renogy BLE stop_notify failed: %s", exc)

    async def _send_request(self, request: bytes, pending: _PendingRequest) -> object:
        async with self._lock:
            if self._future is not None:
                raise RuntimeError("Renogy BLE session already waiting on response")
            self._buffer = bytearray()
            loop = asyncio.get_running_loop()
            self._future = loop.create_future()
            self._pending = pending
            try:
                await self._client.write_gatt_char(
                    self._write_uuid,
                    request,
                    response=self._write_response,
                )
                return await asyncio.wait_for(self._future, timeout=self._timeout_seconds)
            finally:
                self._future = None
                self._pending = None

    async def read_holding_registers(self, address: int, count: int) -> list[int]:
        pending = _PendingRequest(MODBUS_READ_HOLDING_REGS, address, count)
        request = _build_read_holding_request(self._unit_id, address, count)
        result = await self._send_request(request, pending)
        return list(result) if isinstance(result, list) else []

    async def write_single_register(self, address: int, value: int) -> int:
        pending = _PendingRequest(MODBUS_WRITE_SINGLE_REGISTER, address, 1)
        request = _build_write_single_request(self._unit_id, address, value & 0xFFFF)
        result = await self._send_request(request, pending)
        return int(result)

    async def write_multiple_registers(self, address: int, values: list[int]) -> int:
        if not values:
            raise ValueError("At least one register value is required")
        pending = _PendingRequest(MODBUS_WRITE_MULTIPLE_REGISTERS, address, len(values))
        normalized = [value & 0xFFFF for value in values]
        request = _build_write_multiple_request(self._unit_id, address, normalized)
        result = await self._send_request(request, pending)
        return int(result)

    def _on_notify(self, _sender: int, data: bytearray) -> None:
        if not data:
            return
        self._buffer.extend(data)
        self._try_parse()

    def _try_parse(self) -> None:
        if not self._future or self._future.done() or not self._pending:
            return
        buffer = self._buffer
        while buffer:
            if buffer[0] != (self._unit_id & 0xFF):
                buffer.pop(0)
                continue
            if len(buffer) < 3:
                return
            function = buffer[1]
            if function & 0x80:
                if len(buffer) < 5:
                    return
                code = buffer[2]
                frame = bytes(buffer[:5])
                if _crc_ok(frame):
                    self._future.set_exception(RenogyModbusException(code))
                    del buffer[:5]
                    return
                buffer.pop(0)
                continue
            pending = self._pending
            if function != pending.function:
                buffer.pop(0)
                continue
            if function == MODBUS_READ_HOLDING_REGS:
                byte_count = buffer[2]
                frame_len = 3 + byte_count + 2
                if len(buffer) < frame_len:
                    return
                frame = bytes(buffer[:frame_len])
                del buffer[:frame_len]
                if not _crc_ok(frame):
                    self._future.set_exception(RenogyCRCError("CRC check failed"))
                    return
                if byte_count != pending.count * 2:
                    self._future.set_exception(
                        RenogyUnexpectedResponse(
                            f"Unexpected byte count {byte_count} (expected {pending.count * 2})"
                        )
                    )
                    return
                payload = frame[3 : 3 + byte_count]
                regs = [payload[i] << 8 | payload[i + 1] for i in range(0, len(payload), 2)]
                self._future.set_result(regs)
                return
            if function in (MODBUS_WRITE_SINGLE_REGISTER, MODBUS_WRITE_MULTIPLE_REGISTERS):
                frame_len = 8
                if len(buffer) < frame_len:
                    return
                frame = bytes(buffer[:frame_len])
                del buffer[:frame_len]
                if not _crc_ok(frame):
                    self._future.set_exception(RenogyCRCError("CRC check failed"))
                    return
                address = frame[2] << 8 | frame[3]
                value_or_count = frame[4] << 8 | frame[5]
                if address != pending.address:
                    self._future.set_exception(
                        RenogyUnexpectedResponse(
                            f"Unexpected address 0x{address:04X} (expected 0x{pending.address:04X})"
                        )
                    )
                    return
                if function == MODBUS_WRITE_MULTIPLE_REGISTERS and value_or_count != pending.count:
                    self._future.set_exception(
                        RenogyUnexpectedResponse(
                            f"Unexpected register count {value_or_count} (expected {pending.count})"
                        )
                    )
                    return
                self._future.set_result(value_or_count)
                return
            buffer.pop(0)


class RenogyBt2Collector:
    """Poll a Renogy BT-2 BLE module and expose latest metrics."""

    def __init__(self, settings) -> None:
        self.settings = settings
        self._task: asyncio.Task | None = None
        self._stop = asyncio.Event()
        self._metrics: dict[str, float] = {}
        self._updated_at: datetime | None = None
        self._apply_lock = asyncio.Lock()
        self._active_session: _RenogyBleSession | None = None

    @property
    def config(self) -> RenogyBt2Config:
        return self.settings.renogy_bt2

    def start(self) -> None:
        if self._task and not self._task.done():
            return
        if not self.config.enabled:
            return
        if self.config.mode == "external":
            logger.info("Renogy BT-2 external ingest enabled; skipping BLE polling")
            return
        if BleakClient is None:
            logger.warning("Renogy BT-2 disabled: bleak not available on this platform")
            return
        self._stop.clear()
        self._task = asyncio.create_task(self._run(), name="renogy-bt2-collector")

    async def stop(self) -> None:
        self._stop.set()
        if self._task:
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        self._active_session = None

    def snapshot(self) -> RenogyBt2Snapshot:
        return RenogyBt2Snapshot(metrics=dict(self._metrics), updated_at=self._updated_at)

    def read_metric(self, metric: str | None) -> float | None:
        if not metric:
            return None
        return self._metrics.get(metric)

    def ingest_payload(self, payload: dict[str, object]) -> dict[str, float]:
        if not self.config.enabled:
            logger.debug("Renogy BT-2 ingest ignored: collector disabled")
            return {}
        metrics: dict[str, float] = {}
        for source_key, dest_key in RENOGY_BT_FIELD_MAP.items():
            if source_key not in payload:
                continue
            value = _coerce_float(payload[source_key])
            if value is not None:
                metrics[dest_key] = value

        runtime_val = _coerce_float(payload.get("runtime_hours"))
        if runtime_val is not None:
            metrics["runtime_hours"] = runtime_val

        if "runtime_hours" not in metrics:
            capacity_ah = self.config.battery_capacity_ah or 0
            battery_soc = metrics.get("battery_soc_percent", 0.0)
            battery_voltage = metrics.get("battery_voltage_v", 0.0)
            load_power = metrics.get("load_power_w", 0.0)
            runtime_hours = _estimate_runtime_hours(
                battery_capacity_ah=int(capacity_ah),
                battery_voltage=battery_voltage,
                battery_soc=battery_soc,
                load_power_w=load_power,
            )
            if runtime_hours is not None:
                metrics["runtime_hours"] = float(runtime_hours)

        if metrics:
            self._metrics.update(metrics)
            self._updated_at = datetime.now(timezone.utc)
        return metrics

    async def _run(self) -> None:
        retry_delay = 5.0
        while not self._stop.is_set():
            config = self.config
            if not config.enabled:
                await asyncio.sleep(1.0)
                continue
            if not config.address and not config.device_name:
                logger.warning("Renogy BT-2 enabled without address or device_name; waiting")
                await asyncio.sleep(retry_delay)
                continue
            try:
                await self._poll_loop()
            except asyncio.CancelledError:
                break
            except Exception as exc:
                logger.warning("Renogy BT-2 loop error: %s", exc)
                await asyncio.sleep(retry_delay)

    async def _find_device(self):
        config = self.config
        if BleakScanner is None:  # pragma: no cover - environment specific
            raise RuntimeError("bleak not available")
        if config.adapter:
            devices = await BleakScanner.discover(
                timeout=config.connect_timeout_seconds,
                bluez={"adapter": config.adapter},
            )
            selected = self._select_device(devices, config.address, config.device_name)
            if selected is not None:
                return selected
            if config.address:
                return _fallback_bluez_device(config.address, config.adapter, config.device_name)
            return None
        device = None
        if config.address:
            device = await BleakScanner.find_device_by_address(
                config.address,
                timeout=config.connect_timeout_seconds,
            )
            if device is None:
                device = _fallback_bluez_device(config.address, config.adapter, config.device_name)
        if device is None and config.device_name:
            device = await BleakScanner.find_device_by_name(
                config.device_name,
                timeout=config.connect_timeout_seconds,
            )
        return device

    async def _poll_loop(self) -> None:
        config = self.config
        if BleakScanner is None or BleakClient is None:  # pragma: no cover
            raise RuntimeError("bleak not available")

        device = await self._find_device()
        if device is None:
            raise RuntimeError("Renogy BT-2 device not found")

        client = BleakClient(device)
        try:
            await client.connect(timeout=config.connect_timeout_seconds)
            if not client.is_connected:
                raise RuntimeError("Renogy BT-2 connection failed")

            write_uuid, notify_uuid, write_response = await self._resolve_characteristics(client)
            session = _RenogyBleSession(
                client,
                unit_id=config.unit_id,
                write_uuid=write_uuid,
                write_response=write_response,
                notify_uuid=notify_uuid,
                timeout_seconds=config.request_timeout_seconds,
            )
            await session.start()
            self._active_session = session
            try:
                while not self._stop.is_set() and self.config.enabled:
                    try:
                        async with self._apply_lock:
                            runtime = await session.read_holding_registers(
                                RUNTIME_REG_START, RUNTIME_REG_COUNT
                            )
                            settings_regs = await session.read_holding_registers(
                                SETTINGS_REG_START, SETTINGS_REG_COUNT
                            )
                    except (
                        RenogyCRCError,
                        RenogyModbusException,
                        RenogyUnexpectedResponse,
                        asyncio.TimeoutError,
                    ) as exc:
                        logger.warning("Renogy BT-2 poll error: %s", exc)
                        await asyncio.sleep(self.config.poll_interval_seconds)
                        continue

                    metrics = _decode_metrics(runtime, settings_regs)
                    self._metrics = metrics
                    self._updated_at = datetime.now(timezone.utc)
                    await asyncio.sleep(self.config.poll_interval_seconds)
            finally:
                self._active_session = None
                await session.stop()
        finally:
            try:
                await client.disconnect()
            except Exception as exc:  # pragma: no cover - depends on bluez/dbus behavior
                logger.debug("Renogy BT-2 disconnect failed: %s", exc)

    @staticmethod
    def _select_device(devices, address: str | None, name: str | None):
        if address:
            for device in devices:
                if str(device.address).lower() == address.lower():
                    return device
        if name:
            for device in devices:
                if (device.name or "").lower() == name.lower():
                    return device
        return None

    async def _resolve_characteristics(self, client: "BleakClient") -> tuple[str, str, bool]:
        config = self.config

        services = await client.get_services()
        service_filter = _normalize_uuid(config.service_uuid) if config.service_uuid else None

        available_writes: list[str] = []
        available_notifies: list[str] = []
        write_props: dict[str, set[str]] = {}

        for service in services:
            if service_filter and _normalize_uuid(service.uuid) != service_filter:
                continue
            for char in service.characteristics:
                props = {p.lower() for p in char.properties or []}
                if "notify" in props:
                    available_notifies.append(char.uuid)
                if "write" in props or "write-without-response" in props:
                    available_writes.append(char.uuid)
                    write_props[_normalize_uuid(char.uuid)] = props

        if config.write_uuid and config.notify_uuid:
            props = write_props.get(_normalize_uuid(config.write_uuid), set())
            write_response = "write-without-response" not in props
            return config.write_uuid, config.notify_uuid, write_response

        # If the user did not specify a service filter, consider characteristics across all services.
        if not available_writes or not available_notifies:
            for service in services:
                for char in service.characteristics:
                    props = {p.lower() for p in char.properties or []}
                    if "notify" in props:
                        available_notifies.append(char.uuid)
                    if "write" in props or "write-without-response" in props:
                        available_writes.append(char.uuid)
                        write_props.setdefault(_normalize_uuid(char.uuid), props)

        available_writes = _sort_by_preference(available_writes, PREFERRED_WRITE_UUIDS + KNOWN_CHAR_UUIDS)
        available_notifies = _sort_by_preference(available_notifies, PREFERRED_NOTIFY_UUIDS + KNOWN_CHAR_UUIDS)

        if not available_writes or not available_notifies:
            raise RuntimeError("Unable to resolve Renogy BT-2 BLE characteristics (no candidates)")

        probe_timeout = max(1.0, min(float(config.request_timeout_seconds), 3.0))
        for write_uuid, notify_uuid in _candidate_pairs(
            config=config,
            available_writes=available_writes,
            available_notifies=available_notifies,
        ):
            props = write_props.get(_normalize_uuid(write_uuid), set())
            write_response = "write-without-response" not in props
            session = _RenogyBleSession(
                client,
                unit_id=config.unit_id,
                write_uuid=write_uuid,
                write_response=write_response,
                notify_uuid=notify_uuid,
                timeout_seconds=probe_timeout,
            )
            try:
                await session.start()
                await session.read_holding_registers(RUNTIME_REG_START, 2)
                logger.info(
                    "Renogy BT-2 characteristics selected write=%s notify=%s",
                    write_uuid,
                    notify_uuid,
                )
                return write_uuid, notify_uuid, write_response
            except asyncio.TimeoutError:
                continue
            except Exception as exc:  # pragma: no cover - depends on hardware/bluez
                logger.debug(
                    "Renogy BT-2 characteristic probe failed write=%s notify=%s err=%s",
                    write_uuid,
                    notify_uuid,
                    exc,
                )
                continue
            finally:
                await session.stop()

        raise RuntimeError(
            "Unable to resolve Renogy BT-2 BLE characteristics (probe failed; set write_uuid/notify_uuid overrides)"
        )

    @asynccontextmanager
    async def _session_context(self) -> AsyncIterator[_RenogyBleSession]:
        if self._active_session is not None:
            yield self._active_session
            return
        config = self.config
        if BleakClient is None or BleakScanner is None:  # pragma: no cover
            raise RuntimeError("bleak not available")
        device = await self._find_device()
        if device is None:
            raise RuntimeError("Renogy BT-2 device not found")
        async with BleakClient(device) as client:
            if not client.is_connected:
                raise RuntimeError("Renogy BT-2 connection failed")
            write_uuid, notify_uuid, write_response = await self._resolve_characteristics(client)
            session = _RenogyBleSession(
                client,
                unit_id=config.unit_id,
                write_uuid=write_uuid,
                write_response=write_response,
                notify_uuid=notify_uuid,
                timeout_seconds=config.request_timeout_seconds,
            )
            await session.start()
            try:
                yield session
            finally:
                await session.stop()

    async def read_settings_block(self, start_address: int, count: int) -> list[int]:
        async with self._apply_lock:
            async with self._session_context() as session:
                return await session.read_holding_registers(start_address, count)

    async def apply_settings(
        self,
        writes: list[tuple[int, list[int]]],
        *,
        verify: bool = True,
    ) -> list[dict[str, object]]:
        if not writes:
            return []
        async with self._apply_lock:
            async with self._session_context() as session:
                results: list[dict[str, object]] = []
                for address, values in writes:
                    if not values:
                        raise ValueError("Values required for write")
                    normalized = [int(v) & 0xFFFF for v in values]
                    if len(normalized) == 1:
                        await session.write_single_register(address, normalized[0])
                    else:
                        await session.write_multiple_registers(address, normalized)
                    read_back: list[int] | None = None
                    if verify:
                        read_back = await session.read_holding_registers(address, len(normalized))
                        if list(read_back) != normalized:
                            raise RenogyVerificationError(
                                address=address,
                                expected=normalized,
                                actual=list(read_back),
                            )
                    results.append(
                        {
                            "address": address,
                            "values": normalized,
                            "read_back": read_back if verify else None,
                        }
                    )
                return results
