from __future__ import annotations

import asyncio
from typing import List

import pytest

from app.config import SensorConfig, Settings
from app.services.publisher import TelemetryPublisher


class _RecordingForwarder:
    def __init__(self) -> None:
        self.batches: List[List[dict]] = []

    async def push_samples(self, samples: List[dict]) -> int:
        self.batches.append(list(samples))
        return len(samples)


class _FixedAnalog:
    def __init__(self, volts: float) -> None:
        self._volts = float(volts)

    def read_voltage(self, channel: int, negative_channel: int | None = None) -> float:  # noqa: ARG002
        return self._volts


class _NullPulse:
    def read_pulses(self, channel: int) -> int:  # noqa: ARG002
        return 0


def test_current_loop_depth_converts_and_flags_faults(tmp_path) -> None:
    async def runner() -> None:
        sensor = SensorConfig(
            sensor_id="reservoir-depth",
            name="Reservoir Depth",
            type="analog",
            channel=0,
            unit="m",
            interval_seconds=30.0,
            current_loop_shunt_ohms=240.0,
            current_loop_range_m=5.0,
        )
        settings = Settings(
            node_id="validator-depth",
            sensors=[sensor],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        forwarder = _RecordingForwarder()

        # 12 mA -> mid span -> 2.5 m.
        mid_volts = 0.012 * 240.0
        publisher = TelemetryPublisher(settings, _FixedAnalog(mid_volts), _NullPulse(), mesh_adapter=None)
        await publisher._publish_sensor_batch(forwarder)
        assert len(forwarder.batches) == 1
        payload = forwarder.batches[-1][0]
        assert payload["value"] == pytest.approx(2.5, rel=0.01)
        assert payload["quality"] == 0

        # 3 mA -> low fault (open loop). Conversion clamps to 0m but telemetry is suppressed (fail-closed).
        low_volts = 0.003 * 240.0
        value, quality = TelemetryPublisher._convert_current_loop_depth(sensor, low_volts)
        assert value == pytest.approx(0.0, abs=1e-6)
        assert quality == 1
        forwarder.batches.clear()
        publisher = TelemetryPublisher(settings, _FixedAnalog(low_volts), _NullPulse(), mesh_adapter=None)
        await publisher._publish_sensor_batch(forwarder)
        assert forwarder.batches == []

        # 22 mA -> high fault (short/overrange). Conversion clamps to 5m but telemetry is suppressed (fail-closed).
        high_volts = 0.022 * 240.0
        value, quality = TelemetryPublisher._convert_current_loop_depth(sensor, high_volts)
        assert value == pytest.approx(5.0, rel=0.01)
        assert quality == 2
        forwarder.batches.clear()
        publisher = TelemetryPublisher(settings, _FixedAnalog(high_volts), _NullPulse(), mesh_adapter=None)
        await publisher._publish_sensor_batch(forwarder)
        assert forwarder.batches == []

    asyncio.run(runner())


def test_current_loop_unit_conversion_to_feet(tmp_path) -> None:
    async def runner() -> None:
        sensor = SensorConfig(
            sensor_id="reservoir-depth-ft",
            name="Reservoir Depth",
            type="analog",
            channel=0,
            unit="ft",
            interval_seconds=30.0,
            current_loop_shunt_ohms=240.0,
            current_loop_range_m=5.0,
        )
        settings = Settings(
            node_id="validator-depth-ft",
            sensors=[sensor],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        forwarder = _RecordingForwarder()
        mid_volts = 0.012 * 240.0
        publisher = TelemetryPublisher(settings, _FixedAnalog(mid_volts), _NullPulse(), mesh_adapter=None)
        await publisher._publish_sensor_batch(forwarder)
        payload = forwarder.batches[-1][0]
        assert payload["value"] == pytest.approx(2.5 * 3.28084, rel=0.01)

    asyncio.run(runner())


def test_current_loop_respects_offset_and_scale(tmp_path) -> None:
    async def runner() -> None:
        sensor = SensorConfig(
            sensor_id="reservoir-depth-scale",
            name="Reservoir Depth",
            type="analog",
            channel=0,
            unit="m",
            interval_seconds=30.0,
            current_loop_shunt_ohms=240.0,
            current_loop_range_m=5.0,
            offset=1.0,
            scale=2.0,
        )
        settings = Settings(
            node_id="validator-depth-scale",
            sensors=[sensor],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        forwarder = _RecordingForwarder()
        mid_volts = 0.012 * 240.0
        publisher = TelemetryPublisher(settings, _FixedAnalog(mid_volts), _NullPulse(), mesh_adapter=None)
        await publisher._publish_sensor_batch(forwarder)
        payload = forwarder.batches[-1][0]
        assert payload["value"] == pytest.approx((2.5 + 1.0) * 2.0, rel=0.01)

    asyncio.run(runner())


def test_current_loop_partial_config_is_quality_error(tmp_path) -> None:
    async def runner() -> None:
        sensor = SensorConfig(
            sensor_id="reservoir-depth-bad",
            name="Reservoir Depth",
            type="analog",
            channel=0,
            unit="m",
            interval_seconds=30.0,
            current_loop_shunt_ohms=240.0,
        )
        settings = Settings(
            node_id="validator-depth-bad",
            sensors=[sensor],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        forwarder = _RecordingForwarder()
        value, quality = TelemetryPublisher._convert_current_loop_depth(sensor, 1.0)
        assert quality == 3
        assert value == pytest.approx(0.0, abs=1e-6)
        publisher = TelemetryPublisher(settings, _FixedAnalog(1.0), _NullPulse(), mesh_adapter=None)
        await publisher._publish_sensor_batch(forwarder)
        assert forwarder.batches == []

    asyncio.run(runner())
