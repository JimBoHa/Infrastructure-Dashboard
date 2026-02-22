from __future__ import annotations

import json
import asyncio
from datetime import datetime, UTC, timedelta
from typing import Iterable, List

import pytest

from app.config import OutputConfig, SensorConfig, Settings
from app.hardware import NullAnalogReader
from app.hardware.mesh import MeshAdapter, MeshDiagnostics, MeshSample
from app.services.publisher import TelemetryPublisher


class _RecordingClient:
    """Capture MQTT publishes for assertions."""

    def __init__(self) -> None:
        self.messages: List[tuple[str, dict]] = []

    async def publish(self, topic: str, payload: bytes) -> None:
        self.messages.append((topic, json.loads(payload.decode("utf-8"))))


class _RecordingForwarder:
    """Capture node-forwarder pushes for assertions."""

    def __init__(self) -> None:
        self.batches: List[List[dict]] = []

    async def push_samples(self, samples: List[dict]) -> int:
        self.batches.append(list(samples))
        return len(samples)


class _DeterministicAnalog:
    """Return a deterministic sequence of voltages for repeatable tests."""

    def __init__(self, values: Iterable[float]) -> None:
        self._values = list(values)
        self._last = 0.0

    def read_voltage(self, channel: int, negative_channel: int | None = None) -> float:  # noqa: ARG002
        if self._values:
            self._last = float(self._values.pop(0))
        return self._last


class _NullPulse:
    """Return zero pulses to keep telemetry maths simple in tests."""

    def read_pulses(self, channel: int) -> int:
        return 0


def test_sensor_batch_rolling_average_smoke(tmp_path) -> None:
    async def runner() -> None:
        settings = Settings(
            node_id="validator-sensor",
            telemetry_interval_seconds=1.0,
            heartbeat_interval_seconds=10.0,
            sensors=[
                SensorConfig(
                    sensor_id="sensor-rolling",
                    name="Rolling Sensor",
                    type="analog",
                    channel=0,
                    unit="A",
                    interval_seconds=60.0,
                    rolling_average_seconds=0.5,
                )
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        analog = _DeterministicAnalog([1.0, 2.0, 3.0, 4.0, 5.0])
        pulse = _NullPulse()
        publisher = TelemetryPublisher(settings, analog, pulse, mesh_adapter=None)
        forwarder = _RecordingForwarder()

        await publisher._publish_sensor_batch(forwarder)

        assert len(forwarder.batches) == 1
        assert len(forwarder.batches[0]) == 1
        sample = forwarder.batches[0][0]
        assert sample["sensor_id"] == "sensor-rolling"
        assert sample["value"] == pytest.approx(3.0, rel=0.01)

    asyncio.run(runner())


def test_sensor_batch_change_of_value_smoke(tmp_path) -> None:
    async def runner() -> None:
        settings = Settings(
            node_id="validator-cov",
            telemetry_interval_seconds=1.0,
            heartbeat_interval_seconds=10.0,
            sensors=[
                SensorConfig(
                    sensor_id="sensor-cov",
                    name="COV Sensor",
                    type="analog",
                    channel=0,
                    unit="V",
                    interval_seconds=0.0,
                    rolling_average_seconds=0.0,
                )
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        analog = _DeterministicAnalog([1.25])
        pulse = _NullPulse()
        publisher = TelemetryPublisher(settings, analog, pulse, mesh_adapter=None)
        forwarder = _RecordingForwarder()

        await publisher._publish_sensor_batch(forwarder)
        await publisher._publish_sensor_batch(forwarder)

        assert len(forwarder.batches) == 1
        assert len(forwarder.batches[0]) == 1
        sample = forwarder.batches[0][0]
        assert sample["sensor_id"] == "sensor-cov"
        assert sample["value"] == pytest.approx(1.25)

    asyncio.run(runner())


def test_heartbeat_outputs_payload_shape(tmp_path) -> None:
    async def runner() -> None:
        from types import SimpleNamespace

        settings = Settings(
            node_id="validator-heartbeat",
            telemetry_interval_seconds=1.0,
            heartbeat_interval_seconds=10.0,
            outputs=[
                OutputConfig(
                    output_id="out-1",
                    name="Pump",
                    type="relay",
                    channel=0,
                    state="on",
                    default_state="off",
                )
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        analog = _DeterministicAnalog([])
        pulse = _NullPulse()
        fake_latency_probe = SimpleNamespace(
            snapshot=lambda: SimpleNamespace(
                avg_latency_ms=10.0,
                last_latency_ms=10.0,
                jitter_ms=1.0,
                p50_latency_ms_30m=9.5,
                uptime_percent_24h=99.0,
            )
        )
        publisher = TelemetryPublisher(
            settings, analog, pulse, mesh_adapter=None, latency_probe=fake_latency_probe
        )
        client = _RecordingClient()

        await publisher._publish_heartbeat(client)

        topic, payload = client.messages[0]
        assert topic == f"iot/{settings.node_id}/status"
        assert isinstance(payload.get("outputs"), list)
        assert payload["outputs"][0]["output_id"] == "out-1"
        assert payload["outputs"][0]["state"] == "on"
        assert isinstance(payload.get("cpu_percent_per_core"), list)
        assert "memory_used_bytes" in payload
        assert "uptime_percent_24h" in payload
        assert payload.get("ping_p50_30m_ms") == pytest.approx(9.5)
        assert isinstance(payload.get("ip"), str)
        assert payload.get("ip")
        assert isinstance(payload.get("mac_eth"), str)
        assert payload.get("mac_eth")

    asyncio.run(runner())


def test_per_sensor_intervals_and_scaling(tmp_path) -> None:
    async def runner() -> None:
        settings = Settings(
            node_id="validator-intervals",
            telemetry_interval_seconds=1.0,
            heartbeat_interval_seconds=10.0,
            sensors=[
                SensorConfig(
                    sensor_id="fast-psi",
                    name="Pressure",
                    type="analog",
                    channel=0,
                    unit="psi",
                    interval_seconds=0.2,
                    rolling_average_seconds=0.0,
                    input_min=0.0,
                    input_max=10.0,
                    output_min=0.0,
                    output_max=300.0,
                ),
                SensorConfig(
                    sensor_id="slow-flow",
                    name="Flow",
                    type="pulse",
                    channel=1,
                    unit="gpm",
                    interval_seconds=0.6,
                    rolling_average_seconds=0.0,
                    pulses_per_unit=10.0,
                ),
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        analog = _DeterministicAnalog([5.0, 6.0])

        class _PulseCounter:
            def __init__(self) -> None:
                self._count = 0

            def read_pulses(self, channel: int) -> int:  # noqa: ARG002
                self._count += 5
                return self._count

        pulse = _PulseCounter()
        publisher = TelemetryPublisher(settings, analog, pulse, mesh_adapter=None)
        forwarder = _RecordingForwarder()

        next_publish = {sensor.sensor_id: 0.0 for sensor in settings.sensors}
        cov_poll_interval = min(settings.telemetry_interval_seconds, 1.0)

        await publisher._forward_sensors_due(0.0, forwarder, next_publish, cov_poll_interval)
        assert len(forwarder.batches) == 1
        batch = forwarder.batches[-1]
        assert {item["sensor_id"] for item in batch} == {"fast-psi", "slow-flow"}
        first_pressure = next(item for item in batch if item["sensor_id"] == "fast-psi")
        assert first_pressure["value"] == pytest.approx(150.0)  # scaled from 5V to 300 psi span
        first_flow = next(item for item in batch if item["sensor_id"] == "slow-flow")
        assert first_flow["value"] == pytest.approx(0.5)  # 5 pulses / 10 pulses per unit

        await publisher._forward_sensors_due(0.3, forwarder, next_publish, cov_poll_interval)
        batch = forwarder.batches[-1]
        assert {item["sensor_id"] for item in batch} == {"fast-psi"}

        await publisher._forward_sensors_due(0.65, forwarder, next_publish, cov_poll_interval)
        batch = forwarder.batches[-1]
        assert {item["sensor_id"] for item in batch} == {"fast-psi", "slow-flow"}

    asyncio.run(runner())


def test_mesh_buffer_flushes_smoke(tmp_path) -> None:
    async def runner() -> None:
        settings = Settings(
            node_id="validator-mesh",
            telemetry_interval_seconds=5.0,
            heartbeat_interval_seconds=30.0,
            mesh={"enabled": True, "max_backfill_seconds": 60.0},
            sensors=[],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        analog = _DeterministicAnalog([1.0])
        pulse = _NullPulse()
        mesh_adapter = MeshAdapter(settings)
        publisher = TelemetryPublisher(settings, analog, pulse, mesh_adapter=mesh_adapter)
        client = _RecordingClient()

        diagnostics = MeshDiagnostics(lqi=200, rssi=-58, battery_percent=92.0, parent="00:11:22:33:44:55")
        mesh_adapter.ingest_attribute_report(
            ieee="AA:BB:CC:DD:EE:FF",
            endpoint=1,
            cluster=1026,
            attribute=0,
            value=44.2,
            unit="kPa",
            diagnostics=diagnostics,
            metadata={"sensor_type": "pressure"},
            sensor_id="mesh-pressure-1",
        )
        stale_sample = MeshSample(
            ieee="AA:BB:CC:DD:EE:FF",
            endpoint=1,
            cluster=1026,
            attribute=0,
            value=11.0,
            timestamp=datetime.now(UTC)
            - timedelta(seconds=settings.mesh.max_backfill_seconds + 5),
        )
        publisher.ingest_mesh_sample(stale_sample)

        await publisher._publish_mesh_samples(client)

        assert len(client.messages) == 1
        topic, payload = client.messages[0]
        assert topic == f"iot/{settings.node_id}/mesh/mesh-pressure-1/telemetry"
        assert payload["sensor_id"] == "mesh-pressure-1"
        assert payload["source"] == "mesh"
        assert payload["node_id"] == settings.node_id
        assert payload["value"] == pytest.approx(44.2)
        assert payload["unit"] == "kPa"
        assert payload["diagnostics"]["lqi"] == 200
        assert payload["diagnostics"]["battery_percent"] == pytest.approx(92.0)
        assert payload["age_seconds"] <= settings.mesh.max_backfill_seconds
        assert not publisher._mesh_buffer
        assert settings.mesh_summary.node_count == 1
        assert settings.mesh_summary.health == "online"
        assert settings.mesh_summary.last_battery_percent == pytest.approx(92.0)

    asyncio.run(runner())


def test_fail_closed_analog_unhealthy_publishes_no_telemetry(tmp_path) -> None:
    async def runner() -> None:
        from types import SimpleNamespace

        settings = Settings(
            node_id="validator-fail-closed",
            telemetry_interval_seconds=1.0,
            heartbeat_interval_seconds=10.0,
            sensors=[
                SensorConfig(
                    sensor_id="analog-1",
                    name="Analog Sensor",
                    type="analog",
                    channel=0,
                    unit="V",
                    interval_seconds=10.0,
                    rolling_average_seconds=0.0,
                )
            ],
            advertise_ip="127.0.0.1",
            config_path=str(tmp_path / "config.json"),
        )
        analog = NullAnalogReader(required=True, reason="backend unhealthy")
        pulse = _NullPulse()
        fake_latency_probe = SimpleNamespace(
            snapshot=lambda: SimpleNamespace(
                avg_latency_ms=None,
                last_latency_ms=None,
                jitter_ms=None,
                p50_latency_ms_30m=None,
                uptime_percent_24h=None,
            )
        )
        publisher = TelemetryPublisher(
            settings,
            analog,
            pulse,
            mesh_adapter=None,
            latency_probe=fake_latency_probe,
        )
        forwarder = _RecordingForwarder()
        client = _RecordingClient()

        await publisher._publish_sensor_batch(forwarder)
        assert forwarder.batches == []

        await publisher._publish_heartbeat(client)
        assert len(client.messages) == 1
        topic, payload = client.messages[0]
        assert topic == f"iot/{settings.node_id}/status"
        assert payload["analog_backend"] == "disabled"
        assert payload["analog_health"]["ok"] is False
        assert payload["analog_health"]["last_error"] == "backend unhealthy"

    asyncio.run(runner())


def test_prod_build_rejects_simulator_in_publisher(tmp_path) -> None:
    from app import build_info

    if build_info.BUILD_FLAVOR != "prod":
        return

    settings = Settings(
        node_id="validator-prod-sim-reject",
        telemetry_interval_seconds=1.0,
        heartbeat_interval_seconds=10.0,
        sensors=[],
        advertise_ip="127.0.0.1",
        config_path=str(tmp_path / "config.json"),
    )
    analog = NullAnalogReader()
    pulse = _NullPulse()

    with pytest.raises(RuntimeError, match="Simulation is not allowed in production builds"):
        TelemetryPublisher(settings, analog, pulse, mesh_adapter=None, simulator=object())  # type: ignore[arg-type]
