from __future__ import annotations

import logging
import os
import socket
import threading
from typing import Optional

logger = logging.getLogger(__name__)


class SystemdNotifier:
    """Minimal sd_notify implementation with watchdog support.

    Works even when running outside of systemd by safely no-op'ing when
    NOTIFY_SOCKET or WATCHDOG_USEC are not present.
    """

    def __init__(self) -> None:
        self._watchdog_thread: Optional[threading.Thread] = None
        self._watchdog_interval: Optional[float] = None
        self._stop_event = threading.Event()

    def ready(self, status: Optional[str] = None) -> bool:
        return self._send(self._compose_message({"READY": "1"}, status=status))

    def status(self, status: str) -> bool:
        return self._send(self._compose_message({}, status=status))

    def stopping(self, status: Optional[str] = None) -> bool:
        return self._send(self._compose_message({"STOPPING": "1"}, status=status))

    def watchdog_ping(self) -> bool:
        return self._send("WATCHDOG=1")

    def start_watchdog(self) -> bool:
        interval = self._watchdog_interval_seconds()
        if interval is None:
            return False
        if self._watchdog_thread and self._watchdog_thread.is_alive():
            return True
        self._watchdog_interval = interval
        self._stop_event.clear()
        thread = threading.Thread(target=self._watchdog_loop, args=(interval,), name="systemd-watchdog", daemon=True)
        thread.start()
        self._watchdog_thread = thread
        return True

    def stop_watchdog(self) -> None:
        if self._watchdog_thread and self._watchdog_thread.is_alive():
            self._stop_event.set()
            interval = self._watchdog_interval or 1.0
            self._watchdog_thread.join(timeout=interval + 0.5)
        self._watchdog_thread = None
        self._watchdog_interval = None
        self._stop_event.clear()

    def _watchdog_loop(self, interval: float) -> None:
        try:
            self.watchdog_ping()
            while not self._stop_event.wait(interval):
                if not self.watchdog_ping():
                    break
        except Exception:  # pragma: no cover - defensive
            logger.exception("systemd watchdog loop failed")

    def _watchdog_interval_seconds(self) -> Optional[float]:
        raw = os.environ.get("WATCHDOG_USEC")
        if not raw:
            return None
        try:
            usec = int(raw)
        except ValueError:
            logger.warning("Invalid WATCHDOG_USEC value: %s", raw)
            return None
        if usec <= 0:
            return None
        interval = usec / 1_000_000.0 / 2.0
        if interval <= 0:
            interval = 1.0
        return interval

    def _compose_message(self, properties: dict[str, str], status: Optional[str] = None) -> str:
        parts = [f"{key}={value}" for key, value in properties.items()]
        if status is not None:
            parts.append(f"STATUS={status}")
        return "\n".join(parts)

    def _notify_socket_address(self) -> Optional[bytes]:
        path = os.environ.get("NOTIFY_SOCKET")
        if not path:
            return None
        if path.startswith("@"):
            path = "\0" + path[1:]
        return path.encode("utf-8")

    def _send(self, payload: str) -> bool:
        address = self._notify_socket_address()
        if not address:
            return False
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as sock:
                sock.settimeout(0.2)
                sock.sendto(payload.encode("utf-8"), address)
            return True
        except OSError as exc:
            logger.debug("Failed to send sd_notify payload %s: %s", payload, exc)
            return False


__all__ = ["SystemdNotifier"]
