from __future__ import annotations

import pytest

from app.config import OfflineCycleConfig, SimulationProfile, SensorConfig, Settings, OutputConfig
from app.services.output_listener import OutputCommandListener
from app.services.simulator import SimulatedDevice
from app import build_info


@pytest.fixture
def anyio_backend():
    return "asyncio"


def test_simulated_device_offline_cycle():
    profile = SimulationProfile(enabled=True, offline_cycle=OfflineCycleConfig(period_seconds=10, offline_seconds=5))
    if build_info.BUILD_FLAVOR == "prod":
        with pytest.raises(RuntimeError, match="Simulation is not allowed in production builds"):
            SimulatedDevice(profile, seed_hint="cycle")
        return
    device = SimulatedDevice(profile, seed_hint="cycle")
    assert device.is_offline(now=0.0) is True
    assert device.is_offline(now=6.0) is False


def test_simulated_device_time_multiplier_scales_offline_cycle():
    profile = SimulationProfile(
        enabled=True,
        time_multiplier=2.0,
        offline_cycle=OfflineCycleConfig(period_seconds=10, offline_seconds=5),
    )
    if build_info.BUILD_FLAVOR == "prod":
        with pytest.raises(RuntimeError, match="Simulation is not allowed in production builds"):
            SimulatedDevice(profile, seed_hint="multiplier")
        return
    device = SimulatedDevice(profile, seed_hint="multiplier")
    assert device.is_offline(now=2.0) is True
    assert device.is_offline(now=3.0) is False


def test_simulated_device_spike_and_jitter_changes_values():
    profile = SimulationProfile(
        enabled=True,
        spikes={"s1": {"every_seconds": 0.1, "magnitude": 3.0}},
        jitter={"s1": 0.2},
        seed=123,
    )
    if build_info.BUILD_FLAVOR == "prod":
        with pytest.raises(RuntimeError, match="Simulation is not allowed in production builds"):
            SimulatedDevice(profile, seed_hint="spikes")
        return
    device = SimulatedDevice(profile, seed_hint="spikes")
    sensor = SensorConfig(
        sensor_id="s1",
        name="Pump Load",
        type="power",
        channel=0,
        unit="kW",
        interval_seconds=5,
        rolling_average_seconds=0,
    )
    first = device.read_sensor(sensor, now=0.0)
    second = device.read_sensor(sensor, now=1.0)
    assert first is not None and second is not None
    assert abs(second - first) > 0.05


def test_simulated_device_stuck_output_state():
    profile = SimulationProfile(enabled=True, stuck_outputs=["out-1"], seed=42)
    if build_info.BUILD_FLAVOR == "prod":
        with pytest.raises(RuntimeError, match="Simulation is not allowed in production builds"):
            SimulatedDevice(profile, seed_hint="outputs")
        return
    device = SimulatedDevice(profile, seed_hint="outputs")
    first = device.apply_command("out-1", "on")
    assert first.applied_state == "on"
    second = device.apply_command("out-1", "off")
    assert second.applied_state == "on"
    assert second.stuck is True


def test_simulated_device_renogy_metrics_are_bounded():
    profile = SimulationProfile(enabled=True, seed=7)
    if build_info.BUILD_FLAVOR == "prod":
        with pytest.raises(RuntimeError, match="Simulation is not allowed in production builds"):
            SimulatedDevice(profile, seed_hint="renogy")
        return
    device = SimulatedDevice(profile, seed_hint="renogy")
    soc_sensor = SensorConfig(
        sensor_id="renogy-soc",
        name="Battery SOC",
        type="renogy_bt2",
        metric="battery_soc_percent",
        channel=0,
        unit="%",
        interval_seconds=10,
        rolling_average_seconds=0,
    )
    pv_sensor = SensorConfig(
        sensor_id="renogy-pv",
        name="PV Power",
        type="renogy_bt2",
        metric="pv_power_w",
        channel=1,
        unit="W",
        interval_seconds=10,
        rolling_average_seconds=0,
    )
    soc_value = device.read_sensor(soc_sensor, now=0.0)
    pv_value = device.read_sensor(pv_sensor, now=0.0)
    assert soc_value is not None and pv_value is not None
    assert 0.0 <= soc_value <= 100.0
    assert pv_value >= 0.0


class DummyClient:
    def __init__(self):
        self.published = []

    async def publish(self, topic, payload):
        self.published.append((topic, payload))


@pytest.mark.anyio("asyncio")
async def test_output_listener_updates_state_and_ack():
    settings = Settings()
    settings.node_id = "node-xyz"
    settings.outputs = [
        OutputConfig(output_id="out-1", name="Pump", type="relay", channel=0, supported_states=["off", "on"])
    ]
    listener = OutputCommandListener(settings, simulator=None)
    listener._build_topic_map()
    client = DummyClient()
    await listener._handle_message(client, f"iot/{settings.node_id}/out-1/command", b'{"state":"on","reason":"manual"}')
    assert settings.outputs[0].state == "on"
    assert client.published
    topic, payload = client.published[0]
    assert topic.endswith("/out-1/state")
    assert b"on" in payload
