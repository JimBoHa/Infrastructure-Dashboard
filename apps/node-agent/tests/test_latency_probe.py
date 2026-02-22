import time
from collections import deque

from app.services.latency_probe import LatencyProbe


def test_latency_probe_uptime_percent_24h_snapshot():
    probe = LatencyProbe(target_host="127.0.0.1", target_port=1883, interval_seconds=60, window_samples=3)

    now = time.time()
    probe._outcomes = deque(  # type: ignore[attr-defined]
        [
            (now - 10, True),
            (now - 20, False),
            (now - 30, True),
            (now - 60 * 60 * 25, True),  # outside 24h window
        ]
    )

    snapshot = probe.snapshot()
    assert snapshot.uptime_percent_24h is not None
    assert abs(snapshot.uptime_percent_24h - (2 / 3 * 100)) < 0.01


def test_latency_probe_p50_uses_recent_window():
    probe = LatencyProbe(
        probe_kind="icmp",
        target_host="127.0.0.1",
        target_port=0,
        interval_seconds=60,
        window_samples=5,
    )

    now = time.time()
    probe._samples = deque(  # type: ignore[attr-defined]
        [
            ("old", 100.0, now - 3600),  # outside 30m
            ("a", 10.0, now - 100),
            ("b", 20.0, now - 90),
            ("c", 30.0, now - 80),
            ("d", 40.0, now - 70),
            ("e", 50.0, now - 60),
        ]
    )

    snapshot = probe.snapshot()
    assert snapshot.p50_latency_ms_30m is not None
    assert snapshot.p50_latency_ms_30m == 30.0
