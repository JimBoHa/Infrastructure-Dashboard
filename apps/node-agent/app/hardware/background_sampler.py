"""Background sampling helpers to keep hardware I/O off the HTTP event loop."""
from __future__ import annotations

import threading
import time
from collections.abc import Callable, Iterable
from typing import Optional

from app.hardware.analog import AnalogReader


class BackgroundAnalogSampler(AnalogReader):
    """Samples analog voltages on a background thread and serves cached values.

    This is intentionally generic so hardware-specific drivers can provide a
    blocking `read_fn(channel) -> volts` without ever blocking the FastAPI loop.
    """

    def __init__(
        self,
        *,
        read_fn: Callable[[int, Optional[int]], float | None],
        inputs: Iterable[tuple[int, Optional[int]]],
        interval_seconds: float,
    ) -> None:
        self._read_fn = read_fn
        self._inputs = sorted({(int(pos), int(neg) if neg is not None else None) for pos, neg in inputs}) or [
            (0, None)
        ]
        self._interval = max(float(interval_seconds), 0.01)
        self._lock = threading.Lock()
        self._last: dict[tuple[int, Optional[int]], Optional[float]] = {}
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        if self._thread and self._thread.is_alive():
            return
        self._stop.clear()
        self._thread = threading.Thread(target=self._run, name="analog-sampler", daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2.0)

    def read_voltage(self, channel: int, negative_channel: int | None = None) -> float | None:
        channel = int(channel)
        negative = int(negative_channel) if negative_channel is not None else None
        with self._lock:
            value = self._last.get((channel, negative))
        return float(value) if value is not None else None

    def _run(self) -> None:
        while not self._stop.is_set():
            cycle_start = time.monotonic()
            for pos, neg in self._inputs:
                if self._stop.is_set():
                    break
                try:
                    volts = self._read_fn(pos, neg)
                except Exception:
                    volts = None
                with self._lock:
                    self._last[(pos, neg)] = float(volts) if volts is not None else None
            elapsed = time.monotonic() - cycle_start
            sleep_for = max(self._interval - elapsed, 0.0)
            self._stop.wait(timeout=sleep_for)
