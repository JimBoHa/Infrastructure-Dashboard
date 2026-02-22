from __future__ import annotations

"""Zeroconf advertiser for node discovery."""

import logging
import socket
from typing import Dict, Optional

from zeroconf import IPVersion, ServiceInfo, Zeroconf

from app.config import Settings

logger = logging.getLogger(__name__)

SERVICE_TYPE = "_iotnode._tcp.local."


def _build_properties(settings: Settings) -> Dict[bytes, bytes]:
    props: Dict[bytes, bytes] = {}
    for key, value in settings.discovery_properties.items():
        props[key.encode("utf-8")] = str(value).encode("utf-8")
    if settings.mac_eth:
        props[b"mac_eth"] = settings.mac_eth.encode("utf-8")
    if settings.mac_wifi:
        props[b"mac_wifi"] = settings.mac_wifi.encode("utf-8")
    return props


def _as_address(ip: str) -> Optional[bytes]:
    try:
        return socket.inet_aton(ip)
    except OSError:
        return None


class DiscoveryAdvertiser:
    """Registers the node via Zeroconf so the core server can discover it."""

    def __init__(self, settings: Settings):
        self.settings = settings
        self._zeroconf: Optional[Zeroconf] = None
        self._info: Optional[ServiceInfo] = None

    async def start(self) -> None:
        if self._zeroconf:
            return
        ip = self.settings.advertise_ip or "127.0.0.1"
        address = _as_address(ip)
        if not address:
            logger.warning("Unable to advertise IP %s; defaulting to 127.0.0.1", ip)
            address = socket.inet_aton("127.0.0.1")
        properties = _build_properties(self.settings)
        service_name = f"{self.settings.node_id}.{SERVICE_TYPE}"
        self._info = ServiceInfo(
            SERVICE_TYPE,
            service_name,
            addresses=[address],
            port=self.settings.advertise_port,
            properties=properties,
            server=f"{self.settings.node_id}.local.",
        )
        self._zeroconf = Zeroconf(ip_version=IPVersion.V4Only)
        try:
            await self._zeroconf.async_register_service(self._info)
            logger.info("Advertised node %s on %s:%s", service_name, ip, self.settings.advertise_port)
        except Exception:  # pragma: no cover - defensive logging
            logger.exception("Failed to advertise node via Zeroconf")

    async def stop(self) -> None:
        if not self._zeroconf:
            return
        try:
            if self._info:
                await self._zeroconf.async_unregister_service(self._info)
        finally:
            self._zeroconf.close()
            self._zeroconf = None
            self._info = None
