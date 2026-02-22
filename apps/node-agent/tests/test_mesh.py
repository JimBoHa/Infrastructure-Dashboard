from __future__ import annotations

import asyncio
import json
from datetime import datetime, UTC, timedelta
from unittest.mock import AsyncMock

import pytest

from app.config import Settings
from app.hardware.mesh import MeshAdapter, MeshDiagnostics, MeshSample
from app.services.publisher import TelemetryPublisher


class _StubAnalog:
    def read_voltage(self, channel: int) -> float:
        return 0.0


class _StubPulse:
    def read_pulses(self, channel: int) -> int:
        return 0


def test_mesh_adapter_updates_summary():
    settings = Settings(mesh={"enabled": True})
    adapter = MeshAdapter(settings)
    captured: list[MeshSample] = []
    adapter.add_listener(captured.append)

    diagnostics = MeshDiagnostics(lqi=182, rssi=-55, battery_percent=88.5, parent="AA:BB:CC:DD:EE:FF")
    sample = adapter.ingest_attribute_report(
        ieee="00158d0001a2b3c4",
        endpoint=1,
        cluster=0x0402,
        attribute=0x0000,
        value=21.7,
        unit="C",
        diagnostics=diagnostics,
    )

    assert captured and captured[0] is sample
    assert sample.ieee == "00:15:8D:00:01:A2:B3:C4"
    summary = settings.mesh_summary
    assert summary.node_count == 1
    assert summary.last_parent == diagnostics.parent
    assert summary.last_battery_percent == diagnostics.battery_percent
    assert summary.last_rssi == diagnostics.rssi
    assert summary.health == "online"
    assert summary.average_link_quality == pytest.approx(182.0)


def test_publisher_publishes_mesh_samples():
    settings = Settings(mesh={"enabled": True})
    publisher = TelemetryPublisher(settings, _StubAnalog(), _StubPulse())

    sample = MeshSample(
        ieee="00:15:8D:00:01:A2:B3:C4",
        endpoint=1,
        cluster=0x0402,
        attribute=0x0000,
        value=19.4,
        unit="C",
        diagnostics=MeshDiagnostics(lqi=190, battery_percent=77.0, parent="BB:CC:DD:EE:FF:00:11:22"),
    )

    async def runner():
        publisher._loop = asyncio.get_running_loop()  # type: ignore[attr-defined]
        publisher.ingest_mesh_sample(sample)
        client = AsyncMock()
        await publisher._publish_mesh_samples(client)
        assert client.publish.await_count == 1
        topic, payload = client.publish.await_args.args
        assert sample.identifier() in topic
        body = json.loads(payload.decode("utf-8"))
        assert body["diagnostics"]["lqi"] == 190
        assert body["source"] == "mesh"
        assert settings.mesh_summary.node_count == 1
        assert settings.mesh_summary.health == "online"

    asyncio.run(runner())


def test_publisher_drops_stale_mesh_sample():
    settings = Settings(mesh={"enabled": True})
    publisher = TelemetryPublisher(settings, _StubAnalog(), _StubPulse())

    stale = MeshSample(
        ieee="00:15:8D:00:01:A2:B3:C4",
        endpoint=1,
        cluster=0x0006,
        attribute=0x0000,
        value=1,
        timestamp=datetime.now(UTC) - timedelta(seconds=settings.mesh.max_backfill_seconds + 5),
    )
    async def runner():
        publisher._loop = asyncio.get_running_loop()  # type: ignore[attr-defined]
        publisher.ingest_mesh_sample(stale)
        client = AsyncMock()
        await publisher._publish_mesh_samples(client)
        assert client.publish.await_count == 0

    asyncio.run(runner())


def test_mesh_adapter_join_and_remove_mock_mode():
    settings = Settings(mesh={"enabled": True})
    adapter = MeshAdapter(settings)
    # No zigpy controller present, should still return True in mock mode.
    assert asyncio.run(adapter.start_join(5)) is True
    assert asyncio.run(adapter.remove_device("00:11:22:33:44:55:66:77")) is True


def test_mesh_adapter_simulates_topology():
    from app import build_info

    payload = {
        "enabled": True,
        "mesh_nodes": [
            {
                "ieee": "AA:BB:CC:DD:EE:FF:00:11",
                "cluster": 0x0402,
                "attribute": 0x0000,
                "unit": "C",
                "base_value": 22.0,
                "amplitude": 1.0,
                "battery_percent": 80.0,
                "lqi": 200,
                "rssi": -50,
            }
        ],
    }

    if build_info.BUILD_FLAVOR == "prod":
        with pytest.raises(ValueError, match="Simulation is not allowed in production builds"):
            Settings(mesh={"enabled": True, "polling_interval_seconds": 0.5}, simulation=payload)
        return

    settings = Settings(mesh={"enabled": True, "polling_interval_seconds": 0.5}, simulation=payload)
    adapter = MeshAdapter(settings)

    async def runner():
        await adapter.start()
        await adapter._emit_topology_snapshot()
        await adapter.stop()

    asyncio.run(runner())
    topo = adapter.topology_snapshot()
    assert topo
    assert settings.mesh_summary.node_count >= 1
    assert settings.mesh_summary.health == "simulated"


def test_mesh_adapter_polls_and_tracks_topology():
    settings = Settings(mesh={"enabled": True, "polling_interval_seconds": 0.5, "diagnostics_interval_seconds": 10.0})

    class _Neighbor:
        def __init__(self):
            self.ieee = "8899AABBCCDDEEFF"
            self.lqi = 172
            self.relationship = "child"
            self.rssi = -65

    class _Device:
        def __init__(self):
            self.ieee = "0011223344556677"
            self.lqi = 188
            self.rssi = -48
            self.neighbors = [_Neighbor()]
            self.parent_ieee = "0102030405060708"
            self.depth = 2
            self.last_seen = "just now"

    class _App:
        def __init__(self):
            device = _Device()
            self.ieee = "AABBCCDDEEFF0011"
            self.devices = {device.ieee: device}

        async def permit(self, time_s: int):
            self.permit_time = time_s

        async def remove(self, ieee: str):
            self.removed = ieee

        async def shutdown(self):
            self.shut_down = True

        def add_listener(self, listener):
            self.listener = listener

    async def _factory():
        return _App()

    adapter = MeshAdapter(settings, application_factory=_factory)

    async def runner():
        await adapter.start()
        await asyncio.sleep(0.6)
        await adapter.stop()
        topo = adapter.topology_snapshot()
        assert topo and topo[0]["ieee"] == "00:11:22:33:44:55:66:77"
        assert settings.mesh_summary.node_count >= 1
        assert settings.mesh_summary.health_details.get("last_poll_status") in {"ok", "unavailable"}
        assert settings.mesh_summary.health in {"online", "mock"}

    asyncio.run(runner())
