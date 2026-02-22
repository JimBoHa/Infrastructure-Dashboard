from __future__ import annotations

import asyncio

import pytest

from app.config import SensorConfig, Settings
from app.services.publisher import TelemetryPublisher


class _FixedAnalog:
    def read_voltage(self, channel: int) -> float:  # noqa: ARG002
        return 0.0


class _CountingPulse:
    def __init__(self) -> None:
        self._count = 0

    def read_pulses(self, channel: int) -> int:  # noqa: ARG002
        self._count += 10
        return self._count


class _BrokenPulse:
    def read_pulses(self, channel: int) -> int:  # noqa: ARG002
        raise RuntimeError("no counter available")


def test_pulse_delta_reports_deltas(tmp_path) -> None:
    async def runner() -> None:
        settings = Settings(
            node_id="pulse-delta",
            sensors=[
                SensorConfig(
                    sensor_id="flow",
                    name="Flow",
                    type="pulse",
                    channel=17,
                    unit="pulses",
                    interval_seconds=1.0,
                    rolling_average_seconds=0.0,
                    pulses_per_unit=10.0,
                )
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        publisher = TelemetryPublisher(settings, _FixedAnalog(), _CountingPulse())
        # First read: delta == current total (10) -> 1.0 units after scaling.
        value1, quality1 = publisher._measure_sensor(settings.sensors[0])
        assert quality1 == 0
        assert value1 == pytest.approx(1.0)

        # Second read: delta == 10 again -> 1.0 units.
        value2, quality2 = publisher._measure_sensor(settings.sensors[0])
        assert quality2 == 0
        assert value2 == pytest.approx(1.0)

    asyncio.run(runner())


def test_pulse_read_error_surfaces_quality(tmp_path) -> None:
    async def runner() -> None:
        settings = Settings(
            node_id="pulse-error",
            sensors=[
                SensorConfig(
                    sensor_id="rain",
                    name="Rain Gauge",
                    type="pulse",
                    channel=23,
                    unit="pulses",
                    interval_seconds=1.0,
                    rolling_average_seconds=0.0,
                )
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        publisher = TelemetryPublisher(settings, _FixedAnalog(), _BrokenPulse())
        value, quality = publisher._measure_sensor(settings.sensors[0])
        assert value == pytest.approx(0.0)
        assert quality == 4

    asyncio.run(runner())

