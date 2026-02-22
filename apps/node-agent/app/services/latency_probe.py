from __future__ import annotations

import math
import socket
import subprocess
import sys
import threading
import time
from collections import deque
from dataclasses import dataclass
from typing import Deque, Optional, Tuple


def jitter_ms(samples_ms: list[float]) -> float:
    """Population standard deviation for latency jitter."""

    if len(samples_ms) < 2:
        return 0.0
    mean = sum(samples_ms) / len(samples_ms)
    variance = sum((value - mean) ** 2 for value in samples_ms) / len(samples_ms)
    return math.sqrt(max(variance, 0.0))


def percentile(values: list[float], quantile: float) -> Optional[float]:
    if not values:
        return None
    q = max(0.0, min(float(quantile), 1.0))
    ordered = sorted(float(value) for value in values)
    if len(ordered) == 1:
        return ordered[0]
    idx = (len(ordered) - 1) * q
    lo = int(math.floor(idx))
    hi = int(math.ceil(idx))
    if lo == hi:
        return ordered[lo]
    frac = idx - lo
    return ordered[lo] + (ordered[hi] - ordered[lo]) * frac


@dataclass
class LatencySnapshot:
    probe_kind: str
    target_host: str
    target_port: int
    interval_seconds: float
    window_samples: int
    last_latency_ms: Optional[float]
    avg_latency_ms: Optional[float]
    p50_latency_ms_30m: Optional[float]
    jitter_ms: Optional[float]
    uptime_percent_24h: Optional[float]
    sample_count: int
    sample_count_30m: int
    last_error: Optional[str]
    last_sample_at: Optional[str]


class LatencyProbe:
    """Background latency probe that supports TCP and ICMP sampling."""

    def __init__(
        self,
        *,
        probe_kind: str = "tcp",
        target_host: str,
        target_port: int,
        interval_seconds: float,
        window_samples: int,
        timeout_seconds: float = 2.0,
        uptime_window_seconds: float = 60.0 * 60.0 * 24.0,
        p50_window_seconds: float = 60.0 * 30.0,
    ) -> None:
        self._probe_kind = str(probe_kind).strip().lower() or "tcp"
        self._target_host = str(target_host)
        self._target_port = int(target_port)
        self._interval = float(interval_seconds)
        self._window = int(window_samples)
        self._timeout = float(timeout_seconds)
        self._uptime_window_seconds = max(float(uptime_window_seconds), 60.0)
        self._p50_window_seconds = max(float(p50_window_seconds), 60.0)

        self._lock = threading.Lock()
        self._samples: Deque[Tuple[str, float, float]] = deque()
        self._outcomes: Deque[Tuple[float, bool]] = deque()
        self._last_error: Optional[str] = None
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def configure(
        self,
        *,
        probe_kind: Optional[str] = None,
        target_host: str,
        target_port: int,
        interval_seconds: float,
        window_samples: int,
    ) -> None:
        with self._lock:
            if probe_kind is not None and probe_kind.strip():
                self._probe_kind = probe_kind.strip().lower()
            self._target_host = str(target_host)
            self._target_port = int(target_port)
            self._interval = max(float(interval_seconds), 1.0)
            self._window = max(int(window_samples), 3)
            cutoff = time.time() - self._p50_window_seconds
            while self._samples and self._samples[0][2] < cutoff:
                self._samples.popleft()

    def start(self) -> None:
        if self._thread and self._thread.is_alive():
            return
        self._stop.clear()
        self._thread = threading.Thread(target=self._run, name="latency-probe", daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2.0)

    def snapshot(self) -> LatencySnapshot:
        with self._lock:
            probe_kind = self._probe_kind
            host = self._target_host
            port = self._target_port
            interval = self._interval
            window = self._window
            samples = list(self._samples)
            last_error = self._last_error
            outcomes = list(self._outcomes)
            uptime_window_seconds = float(self._uptime_window_seconds)
            p50_window_seconds = float(self._p50_window_seconds)

        last_latency = samples[-1][1] if samples else None
        now = time.time()
        values_30m = [value for _, value, ts in samples if now - float(ts) <= p50_window_seconds]
        values_window = values_30m[-max(int(window), 1) :]
        avg = sum(values_window) / len(values_window) if values_window else None
        p50_30m = percentile(values_30m, 0.5)
        jit = jitter_ms(values_window) if values_window else None
        last_sample_at = samples[-1][0] if samples else None
        now = time.time()
        recent_outcomes = [
            ok for ts, ok in outcomes if now - float(ts) <= uptime_window_seconds
        ]
        attempts = len(recent_outcomes)
        successes = sum(1 for ok in recent_outcomes if ok)
        uptime_percent_24h = (
            (float(successes) / float(attempts) * 100.0) if attempts else None
        )

        return LatencySnapshot(
            probe_kind=probe_kind,
            target_host=host,
            target_port=port,
            interval_seconds=float(interval),
            window_samples=int(window),
            last_latency_ms=last_latency,
            avg_latency_ms=avg,
            p50_latency_ms_30m=p50_30m,
            jitter_ms=jit,
            uptime_percent_24h=uptime_percent_24h,
            sample_count=len(values_window),
            sample_count_30m=len(values_30m),
            last_error=last_error,
            last_sample_at=last_sample_at,
        )

    def _run(self) -> None:
        next_sample = time.monotonic()
        while not self._stop.is_set():
            next_sample += self._interval
            kind, host, port, timeout, window, uptime_window_seconds, p50_window_seconds = (
                self._read_config()
            )
            ts = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            latency = self._sample_latency_ms(kind, host, port, timeout)
            outcome_ts = time.time()
            with self._lock:
                if latency is None:
                    if self._last_error is None:
                        suffix = f"{host}:{port}" if kind == "tcp" else host
                        self._last_error = f"{kind}_probe_failed:{suffix}"
                    self._outcomes.append((outcome_ts, False))
                else:
                    self._last_error = None
                    self._samples.append((ts, float(latency), outcome_ts))
                    self._outcomes.append((outcome_ts, True))
                    while len(self._samples) > max(window * 4, 64):
                        self._samples.popleft()
                    while self._samples and (outcome_ts - self._samples[0][2]) > p50_window_seconds:
                        self._samples.popleft()
                while self._outcomes and (outcome_ts - self._outcomes[0][0]) > uptime_window_seconds:
                    self._outcomes.popleft()

            self._stop.wait(timeout=max(next_sample - time.monotonic(), 0.2))

    def _read_config(self) -> tuple[str, str, int, float, int, float, float]:
        with self._lock:
            return (
                self._probe_kind,
                self._target_host,
                self._target_port,
                self._timeout,
                self._window,
                float(self._uptime_window_seconds),
                float(self._p50_window_seconds),
            )

    @classmethod
    def _sample_latency_ms(
        cls,
        kind: str,
        host: str,
        port: int,
        timeout: float,
    ) -> Optional[float]:
        if kind == "icmp":
            return cls._icmp_latency_ms(host=host, timeout=timeout)
        return cls._tcp_latency_ms(host=host, port=port, timeout=timeout)

    @staticmethod
    def _tcp_latency_ms(host: str, port: int, timeout: float) -> Optional[float]:
        start = time.monotonic()
        try:
            with socket.create_connection((host, int(port)), timeout=float(timeout)):
                pass
        except OSError:
            return None
        return (time.monotonic() - start) * 1000.0

    @staticmethod
    def _icmp_latency_ms(host: str, timeout: float) -> Optional[float]:
        host = str(host).strip()
        if not host:
            return None

        timeout = max(float(timeout), 0.25)
        if sys.platform == "darwin":
            timeout_arg = str(max(int(timeout * 1000), 250))
            cmd = ["ping", "-n", "-q", "-c", "1", "-W", timeout_arg, host]
        else:
            timeout_arg = str(max(int(math.ceil(timeout)), 1))
            cmd = ["ping", "-n", "-q", "-c", "1", "-W", timeout_arg, host]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout + 1.0,
                check=False,
            )
        except (OSError, subprocess.SubprocessError):
            return None

        if result.returncode != 0:
            return None

        combined = "\n".join((result.stdout or "", result.stderr or ""))
        direct = _extract_first_float(r"time[=<]([0-9]+(?:\.[0-9]+)?)\s*ms", combined)
        if direct is not None:
            return direct
        summary = _extract_first_float(
            r"(?:round-trip|rtt)\s+min/avg/max/(?:stddev|mdev)\s*=\s*[0-9.]+/([0-9.]+)/",
            combined,
        )
        return summary


def _extract_first_float(pattern: str, text: str) -> Optional[float]:
    import re

    match = re.search(pattern, text, flags=re.IGNORECASE)
    if not match:
        return None
    raw = match.group(1)
    try:
        value = float(raw)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(value) or value < 0.0:
        return None
    return value
