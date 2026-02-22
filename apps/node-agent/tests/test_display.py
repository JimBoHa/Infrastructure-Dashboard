from __future__ import annotations

import importlib
import math
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from app.config import get_settings
from app.services.latency_probe import jitter_ms


def _auth() -> dict:
    return {"Authorization": "Bearer test-provisioning-secret"}


@pytest.fixture()
def api(tmp_path) -> TestClient:
    tmp_dir = Path(tmp_path)
    mp = pytest.MonkeyPatch()
    mp.setenv("NODE_CONFIG_PATH", str(tmp_dir / "node_config.json"))
    mp.setenv("NODE_PROVISION_QUEUE_PATH", str(tmp_dir / "provision_queue.json"))
    mp.setenv("NODE_PROVISIONING_SECRET", "test-provisioning-secret")
    get_settings.cache_clear()

    import app.main as main_module

    importlib.reload(main_module)
    client_context = TestClient(main_module.app)
    client = client_context.__enter__()
    try:
        yield client
    finally:
        client_context.__exit__(None, None, None)
        mp.undo()
        get_settings.cache_clear()


def test_latency_jitter_population_stddev():
    samples = [100.0, 110.0, 90.0]
    mean = sum(samples) / len(samples)
    variance = sum((value - mean) ** 2 for value in samples) / len(samples)
    assert jitter_ms(samples) == pytest.approx(math.sqrt(variance))


def test_latency_jitter_single_sample_zero():
    assert jitter_ms([123.0]) == 0.0


def test_display_routes_available(api):
    resp = api.get("/display")
    assert resp.status_code == 200
    assert "<title" in resp.text.lower()

    state = api.get("/v1/display/state")
    assert state.status_code == 200
    body = state.json()
    assert "display" in body and "node" in body and "system" in body
    assert body["display"]["enabled"] is False


def test_display_state_missing_sensor_tile(api):
    payload = {
        "sensors": [
            {
                "sensor_id": "sensor-a",
                "name": "Sensor A",
                "type": "moisture",
                "channel": 1,
                "interval_seconds": 30,
                "rolling_average_seconds": 0,
                "unit": "%",
            }
        ],
        "display": {
            "enabled": True,
            "tiles": [
                {"type": "sensor", "sensor_id": "missing-sensor"},
            ],
        },
    }
    resp = api.post("/v1/config/import", json=payload, headers=_auth())
    assert resp.status_code == 200

    state = api.get("/v1/display/state")
    assert state.status_code == 200
    data = state.json()
    tile_sensors = data["tiles"]["sensors"]
    assert len(tile_sensors) == 1
    assert tile_sensors[0]["sensor_id"] == "missing-sensor"
    assert tile_sensors[0]["missing"] is True
