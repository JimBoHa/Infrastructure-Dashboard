from __future__ import annotations

import threading
import time

from app.hardware import BackgroundAnalogSampler


def test_background_sampler_serves_cached_values() -> None:
    called = 0
    ready = threading.Event()

    def read_fn(channel: int, negative_channel: int | None) -> float:  # noqa: ARG001
        nonlocal called
        called += 1
        ready.set()
        return 1.234

    sampler = BackgroundAnalogSampler(
        read_fn=read_fn,
        inputs=[(0, None)],
        interval_seconds=10.0,
    )
    sampler.start()
    try:
        assert ready.wait(timeout=1.0)
        before = called
        for _ in range(100):
            assert sampler.read_voltage(0) == 1.234
        # The background thread is sleeping; reads must be served from cache.
        assert called == before
    finally:
        sampler.stop()
