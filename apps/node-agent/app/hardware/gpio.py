"""GPIO pulse input driver.

Design constraints (generic Pi 5 stack):
- Pulse capture must not rely on busy polling.
- Node-agent should read **deltas** at publish cadence, derived from a cumulative counter.

This module provides a cumulative pulse counter backend:
- Preferred: pigpio (DMA-backed edge sampling via pigpiod).
- Fallback: a deterministic-ish stub used in non-Linux/test environments.
"""
from __future__ import annotations

import logging
import random
import threading
from collections.abc import Iterable
from typing import Protocol

logger = logging.getLogger(__name__)

try:  # pragma: no cover - optional at runtime on Linux
    import pigpio  # type: ignore
except Exception:  # pragma: no cover
    pigpio = None  # type: ignore[assignment]


class PulseReader(Protocol):
    def read_pulses(self, channel: int) -> int:
        ...


class PulseInputDriver:
    """Cumulative pulse counter per channel.

    `read_pulses(channel)` returns a monotonically increasing total count, not a delta.
    The telemetry publisher is responsible for converting totals into per-interval deltas.
    """

    def __init__(self, *, channels: Iterable[int] | None = None):
        self._channels = sorted({int(ch) for ch in (channels or [])})
        self._lock = threading.Lock()
        self._counts: dict[int, int] = {ch: 0 for ch in self._channels}
        self._random = random.Random()
        self._callbacks = []
        self._pi = None
        self._healthy = True

        if not self._channels:
            return

        if pigpio is None:
            self._healthy = False
            logger.info("pigpio not available; pulse inputs will run in stub mode")
            return

        try:
            pi = pigpio.pi()
            if not getattr(pi, "connected", False):
                raise RuntimeError("pigpiod not reachable (pi.connected=false)")
            for gpio in self._channels:
                pi.set_mode(gpio, pigpio.INPUT)
                pi.set_pull_up_down(gpio, pigpio.PUD_UP)
                cb = pi.callback(gpio, pigpio.RISING_EDGE, self._on_edge)
                self._callbacks.append(cb)
            self._pi = pi
        except Exception as exc:
            self._healthy = False
            logger.warning("Unable to initialize pigpio pulse counters (%s); using stub mode", exc)

    @property
    def healthy(self) -> bool:
        return bool(self._healthy)

    def start(self) -> None:
        # pigpio callbacks start immediately; stub mode needs no setup.
        return

    def stop(self) -> None:
        for cb in self._callbacks:
            try:
                cb.cancel()
            except Exception:
                pass
        self._callbacks.clear()
        if self._pi is not None:
            try:
                self._pi.stop()
            except Exception:
                pass
        self._pi = None

    def read_pulses(self, channel: int) -> int:
        channel = int(channel)
        if self._channels and channel not in self._counts:
            raise ValueError(f"Pulse channel {channel} not configured (configured={self._channels})")

        if self._pi is None:
            # Stub mode: increment counts by a small random amount per read.
            bump = self._random.randint(0, 5)
            with self._lock:
                self._counts[channel] = int(self._counts.get(channel, 0)) + int(bump)
                return int(self._counts[channel])

        with self._lock:
            return int(self._counts.get(channel, 0))

    def _on_edge(self, gpio: int, level: int, _tick: int) -> None:
        # pigpio callback signature: (gpio, level, tick). We only count rising edges.
        if level != 1:
            return
        with self._lock:
            self._counts[gpio] = int(self._counts.get(gpio, 0)) + 1
