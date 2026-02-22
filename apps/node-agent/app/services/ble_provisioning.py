"""BLE GATT provisioning service for first-boot onboarding.

The node agent advertises a custom BLE service that iOS can use to push
Wi-Fi credentials, a friendly node name, and an adoption token before the
node is on the LAN.

This implementation targets Linux BlueZ via D-Bus. When BlueZ or D-Bus are
unavailable (e.g., during unit tests or on non-Linux hosts), the service
silently disables itself.
"""

from __future__ import annotations

import asyncio
import json
import logging
import platform
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Awaitable, Callable, Dict, List, Optional

from app.config import Settings

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# BLE UUIDs (must match iOS app)

PROVISION_SERVICE_UUID = "9F0C9A30-8B1D-4E64-9A0A-0F2ED01F9F60"
INFO_CHARACTERISTIC_UUID = "9F0C9A31-8B1D-4E64-9A0A-0F2ED01F9F60"
PROVISION_CHARACTERISTIC_UUID = "9F0C9A32-8B1D-4E64-9A0A-0F2ED01F9F60"
STATUS_CHARACTERISTIC_UUID = "9F0C9A33-8B1D-4E64-9A0A-0F2ED01F9F60"

_BLUEZ_SERVICE = "org.bluez"
_OBJ_MANAGER_IFACE = "org.freedesktop.DBus.ObjectManager"
_ADAPTER_IFACE = "org.bluez.Adapter1"
_GATT_MANAGER_IFACE = "org.bluez.GattManager1"
_ADV_MANAGER_IFACE = "org.bluez.LEAdvertisingManager1"


# ---------------------------------------------------------------------------
# D-Bus helpers (import lazily so tests don't need BlueZ)

if platform.system().lower() == "linux":
    try:  # pragma: no cover - optional runtime dependency
        from dbus_next.aio import MessageBus
        from dbus_next.constants import BusType, PropertyAccess
        from dbus_next.service import ServiceInterface, dbus_property, method
        from dbus_next import Variant

        DBUS_AVAILABLE = True
    except Exception:  # pragma: no cover
        MessageBus = object  # type: ignore
        ServiceInterface = object  # type: ignore
        Variant = object  # type: ignore
        BusType = object  # type: ignore

        class _DummyAccess:  # pragma: no cover
            READ = "read"

        PropertyAccess = _DummyAccess  # type: ignore
        dbus_property = method = lambda *args, **kwargs: (lambda fn: fn)  # type: ignore
        DBUS_AVAILABLE = False
else:  # pragma: no cover - non-Linux hosts skip dbus imports
    MessageBus = object  # type: ignore
    ServiceInterface = object  # type: ignore
    Variant = object  # type: ignore
    BusType = object  # type: ignore

    class _DummyAccess:  # pragma: no cover
        READ = "read"

    PropertyAccess = _DummyAccess  # type: ignore
    dbus_property = method = lambda *args, **kwargs: (lambda fn: fn)  # type: ignore
    DBUS_AVAILABLE = False


@dataclass
class ProvisioningEvent:
    state: str
    message: str
    timestamp: str

    @classmethod
    def now(cls, state: str, message: str) -> "ProvisioningEvent":
        return cls(
            state=state,
            message=message,
            timestamp=datetime.now(timezone.utc).isoformat(),
        )

    def as_json_bytes(self) -> bytes:
        return json.dumps({"state": self.state, "message": self.message, "timestamp": self.timestamp}).encode(
            "utf-8"
        )


class BLEProvisioningManager:
    """Owns the GATT application and advertisement lifecycle."""

    def __init__(
        self,
        settings: Settings,
        apply_callback: Callable[[Dict[str, Any]], Awaitable[ProvisioningEvent]],
        *,
        local_name: Optional[str] = None,
    ) -> None:
        self.settings = settings
        self.apply_callback = apply_callback
        self.local_name = local_name or f"FarmNode-{(settings.mac_wifi or settings.mac_eth or settings.node_id)[-4:]}"
        self._bus: Optional[MessageBus] = None
        self._adapter_path: Optional[str] = None
        self._gatt_manager = None
        self._adv_manager = None
        self._app = None
        self._adv = None
        self._status_char: Optional["StatusCharacteristic"] = None
        self.available = False

    async def start(self) -> None:
        if not DBUS_AVAILABLE or platform.system().lower() != "linux":
            if self.settings.simulation.enabled:
                self.available = True
                logger.info("BLE provisioning simulated for %s", self.local_name)
            else:
                logger.info("BLE provisioning disabled (non-Linux or dbus-next missing)")
            return
        try:
            bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
            self._bus = bus
            adapter_path = await _find_adapter(bus)
            if not adapter_path:
                logger.warning("No BlueZ adapter found; BLE provisioning disabled")
                return
            self._adapter_path = adapter_path
            adapter_obj = await _proxy(bus, adapter_path)
            self._gatt_manager = adapter_obj.get_interface(_GATT_MANAGER_IFACE)
            self._adv_manager = adapter_obj.get_interface(_ADV_MANAGER_IFACE)

            # Build app objects.
            app_path = "/com/farmdashboard/provisioning"
            app = ProvisioningApplication(app_path)
            service = ProvisioningService(app_path + "/service0")
            info_char = InfoCharacteristic(
                service.path + "/char0",
                service=service,
                settings=self.settings,
            )
            status_char = StatusCharacteristic(
                service.path + "/char1",
                service=service,
            )
            provision_char = ProvisionCharacteristic(
                service.path + "/char2",
                service=service,
                on_payload=self._handle_payload,
                status_char=status_char,
            )
            service.add_characteristics([info_char, status_char, provision_char])
            app.add_service(service)

            # Export objects.
            bus.export(app.path, app)
            for obj in app.managed_objects():
                bus.export(obj.path, obj)

            await self._gatt_manager.call_register_application(app.path, {})

            adv_path = app_path + "/advertisement0"
            adv = ProvisioningAdvertisement(adv_path, local_name=self.local_name)
            bus.export(adv.path, adv)
            await self._adv_manager.call_register_advertisement(adv.path, {})

            self._app = app
            self._adv = adv
            self._status_char = status_char
            self.available = True
            logger.info("BLE provisioning advertised as %s on %s", self.local_name, adapter_path)
        except Exception:  # pragma: no cover - runtime only
            logger.exception("Failed to start BLE provisioning")

    async def stop(self) -> None:
        if self.available and not self._bus:
            self.available = False
            return
        if not self._bus or not self.available:
            return
        try:  # pragma: no cover - runtime only
            if self._adv_manager and self._adv:
                await self._adv_manager.call_unregister_advertisement(self._adv.path)
            if self._gatt_manager and self._app:
                await self._gatt_manager.call_unregister_application(self._app.path)
        except Exception:
            logger.exception("Failed to stop BLE provisioning cleanly")
        finally:
            try:
                self._bus.disconnect()
            except Exception:
                pass
            self.available = False

    async def notify(self, event: ProvisioningEvent) -> None:
        if self._status_char:
            self._status_char.update(event.as_json_bytes())
        elif self.available:
            logger.debug("BLE provisioning event: %s", event.state)

    async def _handle_payload(self, payload: Dict[str, Any]) -> ProvisioningEvent:
        await self.notify(ProvisioningEvent.now("received", "Provisioning payload received"))
        try:
            event = await self.apply_callback(payload)
        except Exception as exc:  # pragma: no cover
            logger.exception("Provisioning apply callback failed")
            event = ProvisioningEvent.now("error", f"apply_failed: {exc}")
        await self.notify(event)
        return event


# ---------------------------------------------------------------------------
# GATT application objects


class ProvisioningApplication(ServiceInterface):
    """ObjectManager root for our GATT services."""

    def __init__(self, path: str):
        super().__init__(_OBJ_MANAGER_IFACE)
        self.path = path
        self.services: List[ProvisioningService] = []

    def add_service(self, service: "ProvisioningService") -> None:
        self.services.append(service)

    def managed_objects(self) -> List[Any]:
        objs: List[Any] = []
        for service in self.services:
            objs.append(service)
            objs.extend(service.characteristics)
        return objs

    @method()
    def GetManagedObjects(self) -> "a{oa{sa{sv}}}":
        response: Dict[str, Dict[str, Dict[str, Variant]]] = {}
        for service in self.services:
            response[service.path] = service.get_properties()
            for characteristic in service.characteristics:
                response[characteristic.path] = characteristic.get_properties()
        return response


class ProvisioningService(ServiceInterface):
    def __init__(self, path: str):
        super().__init__("org.bluez.GattService1")
        self.path = path
        self.uuid = PROVISION_SERVICE_UUID
        self.primary = True
        self.characteristics: List[ProvisioningCharacteristic] = []

    def add_characteristics(self, characteristics: List["ProvisioningCharacteristic"]) -> None:
        self.characteristics.extend(characteristics)

    def get_properties(self) -> Dict[str, Dict[str, Variant]]:
        return {
            "org.bluez.GattService1": {
                "UUID": Variant("s", self.uuid),
                "Primary": Variant("b", self.primary),
                "Characteristics": Variant("ao", [char.path for char in self.characteristics]),
            }
        }

    @dbus_property(access=PropertyAccess.READ)
    def UUID(self) -> "s":  # noqa: N802
        return self.uuid

    @dbus_property(access=PropertyAccess.READ)
    def Primary(self) -> "b":  # noqa: N802
        return self.primary

    @dbus_property(access=PropertyAccess.READ)
    def Characteristics(self) -> "ao":  # noqa: N802
        return [char.path for char in self.characteristics]


class ProvisioningCharacteristic(ServiceInterface):
    def __init__(self, path: str, *, service: ProvisioningService, uuid: str, flags: List[str]):
        super().__init__("org.bluez.GattCharacteristic1")
        self.path = path
        self.service = service
        self.uuid = uuid
        self.flags = flags
        self._value: bytes = b""
        self._notifying = False

    def get_properties(self) -> Dict[str, Dict[str, Variant]]:
        return {
            "org.bluez.GattCharacteristic1": {
                "UUID": Variant("s", self.uuid),
                "Service": Variant("o", self.service.path),
                "Flags": Variant("as", self.flags),
                "Notifying": Variant("b", self._notifying),
            }
        }

    @dbus_property(access=PropertyAccess.READ)
    def UUID(self) -> "s":  # noqa: N802
        return self.uuid

    @dbus_property(access=PropertyAccess.READ)
    def Service(self) -> "o":  # noqa: N802
        return self.service.path

    @dbus_property(access=PropertyAccess.READ)
    def Flags(self) -> "as":  # noqa: N802
        return self.flags

    @dbus_property(access=PropertyAccess.READ)
    def Notifying(self) -> "b":  # noqa: N802
        return self._notifying

    @method()
    def ReadValue(self, options: "a{sv}") -> "ay":  # noqa: N802
        return list(self._value)

    @method()
    def WriteValue(self, value: "ay", options: "a{sv}"):  # noqa: N802
        self._value = bytes(value)

    @method()
    def StartNotify(self):  # noqa: N802
        self._notifying = True

    @method()
    def StopNotify(self):  # noqa: N802
        self._notifying = False

    def update(self, value: bytes) -> None:
        self._value = value
        if self._notifying:
            self.emit_properties_changed(
                {
                    "Value": Variant("ay", list(self._value)),
                },
                [],
            )


class InfoCharacteristic(ProvisioningCharacteristic):
    def __init__(self, path: str, *, service: ProvisioningService, settings: Settings):
        super().__init__(path, service=service, uuid=INFO_CHARACTERISTIC_UUID, flags=["read"])
        self.settings = settings

    @method()
    def ReadValue(self, options: "a{sv}") -> "ay":  # noqa: N802
        payload = {
            "node_id": self.settings.node_id,
            "node_name": self.settings.node_name,
            "mac_eth": self.settings.mac_eth,
            "mac_wifi": self.settings.mac_wifi,
            "hardware": self.settings.hardware_model,
            "firmware": self.settings.firmware_version,
            "capabilities": self.settings.capabilities,
            "adoption_token": self.settings.adoption_token,
        }
        data = json.dumps(payload).encode("utf-8")
        self._value = data
        return list(data)


class StatusCharacteristic(ProvisioningCharacteristic):
    def __init__(self, path: str, *, service: ProvisioningService):
        super().__init__(path, service=service, uuid=STATUS_CHARACTERISTIC_UUID, flags=["read", "notify"])
        self.update(ProvisioningEvent.now("idle", "waiting").as_json_bytes())


class ProvisionCharacteristic(ProvisioningCharacteristic):
    def __init__(
        self,
        path: str,
        *,
        service: ProvisioningService,
        on_payload: Callable[[Dict[str, Any]], Awaitable[ProvisioningEvent]],
        status_char: StatusCharacteristic,
    ) -> None:
        super().__init__(path, service=service, uuid=PROVISION_CHARACTERISTIC_UUID, flags=["write", "write-without-response"])
        self.on_payload = on_payload
        self.status_char = status_char
        self._buffer = bytearray()
        self._max_buffer = 4096

    @method()
    def WriteValue(self, value: "ay", options: "a{sv}"):  # noqa: N802
        chunk = bytes(value)
        if not chunk:
            return
        if len(self._buffer) + len(chunk) > self._max_buffer:
            self._buffer.clear()
            self.status_char.update(ProvisioningEvent.now("error", "payload_too_large").as_json_bytes())
            return
        self._buffer.extend(chunk)

        # Attempt to parse JSON when complete.
        try:
            payload = json.loads(self._buffer.decode("utf-8"))
        except json.JSONDecodeError:
            return
        except UnicodeDecodeError:
            self._buffer.clear()
            self.status_char.update(ProvisioningEvent.now("error", "invalid_utf8").as_json_bytes())
            return

        self._buffer.clear()

        async def _apply():
            try:
                await self.on_payload(payload)
            except Exception:
                logger.exception("Error applying provisioning payload")

        try:
            asyncio.get_running_loop().create_task(_apply())
        except RuntimeError:  # pragma: no cover - defensive fallback
            asyncio.get_event_loop().create_task(_apply())


# ---------------------------------------------------------------------------
# Advertisement object


class ProvisioningAdvertisement(ServiceInterface):
    def __init__(self, path: str, *, local_name: str):
        super().__init__("org.bluez.LEAdvertisement1")
        self.path = path
        self.local_name = local_name
        self.type = "peripheral"

    def get_properties(self) -> Dict[str, Dict[str, Variant]]:
        return {
            "org.bluez.LEAdvertisement1": {
                "Type": Variant("s", self.type),
                "ServiceUUIDs": Variant("as", [PROVISION_SERVICE_UUID]),
                "LocalName": Variant("s", self.local_name),
                "Discoverable": Variant("b", True),
            }
        }

    @dbus_property(access=PropertyAccess.READ)
    def Type(self) -> "s":  # noqa: N802
        return self.type

    @dbus_property(access=PropertyAccess.READ)
    def ServiceUUIDs(self) -> "as":  # noqa: N802
        return [PROVISION_SERVICE_UUID]

    @dbus_property(access=PropertyAccess.READ)
    def LocalName(self) -> "s":  # noqa: N802
        return self.local_name

    @method()
    def Release(self):  # noqa: N802
        logger.info("BLE advertisement released")


# ---------------------------------------------------------------------------
# BlueZ discovery helpers


async def _proxy(bus: MessageBus, path: str):
    introspection = await bus.introspect(_BLUEZ_SERVICE, path)
    return bus.get_proxy_object(_BLUEZ_SERVICE, path, introspection)


async def _find_adapter(bus: MessageBus) -> Optional[str]:
    root = await _proxy(bus, "/")
    manager = root.get_interface(_OBJ_MANAGER_IFACE)
    objects = await manager.call_get_managed_objects()
    for path, interfaces in objects.items():
        if _ADAPTER_IFACE in interfaces:
            return path
    return None
