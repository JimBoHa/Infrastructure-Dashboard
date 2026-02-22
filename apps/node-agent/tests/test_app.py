from __future__ import annotations

import importlib
import json
import time
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from app.config import get_settings
from app.services.config_store import ConfigStore, apply_config
from app.services.provisioning import ProvisioningStore
from app.services.simulator import SimulatedDevice
from app import build_info


@pytest.fixture(scope="module")
def api(tmp_path_factory) -> TestClient:
    """Isolated TestClient so defaults can be production-safe (empty sensors/outputs)."""

    tmp_dir = tmp_path_factory.mktemp("node-agent")
    mp = pytest.MonkeyPatch()
    mp.setenv("NODE_CONFIG_PATH", str(Path(tmp_dir) / "node_config.json"))
    mp.setenv("NODE_PROVISION_QUEUE_PATH", str(Path(tmp_dir) / "provision_queue.json"))
    mp.setenv("NODE_ADOPTION_TOKEN", "test-token")
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


def _auth() -> dict:
    return {"Authorization": "Bearer test-provisioning-secret"}


def test_config_endpoint_requires_auth(api):
    resp = api.get("/v1/config")
    assert resp.status_code == 401


def test_config_endpoint_returns_settings(api):
    resp = api.get("/v1/config", headers=_auth())
    assert resp.status_code == 200
    data = resp.json()
    assert data["node"]["node_id"]
    assert "adoption_token" not in data["node"]
    assert "password" not in (data.get("wifi_hints") or {})
    assert isinstance(data["sensors"], list)
    assert isinstance(data["outputs"], list)


def test_status_endpoint_contains_metadata(api):
    resp = api.get("/v1/status")
    assert resp.status_code == 200
    body = resp.json()
    assert body["node_id"] == get_settings().node_id
    assert "uptime_seconds" in body
    assert "cpu_percent" in body
    assert body["capabilities"]
    assert "mesh_topology" in body


def test_update_sensor_and_persist(api):
    # create a sensor first (production default is empty)
    create_payload = {
        "sensors": [
            {
                "sensor_id": "sensor-for-patch",
                "name": "Patch Sensor",
                "type": "moisture",
                "channel": 1,
                "interval_seconds": 60,
                "rolling_average_seconds": 0,
                "unit": "%",
            }
        ]
    }
    resp = api.post("/v1/config/import", json=create_payload, headers=_auth())
    assert resp.status_code == 200

    sensor_id = "sensor-for-patch"
    patch = {
        "name": "Updated Sensor",
        "interval_seconds": 120,
        "rolling_average_seconds": 15,
        "location": "Greenhouse",
    }
    resp = api.patch(f"/v1/sensors/{sensor_id}", json=patch, headers=_auth())
    assert resp.status_code == 200
    payload = resp.json()
    assert payload["name"] == "Updated Sensor"
    refreshed = api.get("/v1/config", headers=_auth()).json()
    sensor = next(item for item in refreshed["sensors"] if item["sensor_id"] == sensor_id)
    assert sensor["location"] == "Greenhouse"


def test_restore_config_replaces_outputs(api):
    payload = {
        "outputs": [
            {
                "output_id": "out-new",
                "name": "Vent Fan",
                "type": "relay",
                "channel": 2,
                "supported_states": ["off", "on", "auto"],
                "default_state": "auto",
            }
        ]
    }
    resp = api.post("/v1/config/restore", json=payload, headers=_auth())
    assert resp.status_code == 200
    config = api.get("/v1/config", headers=_auth()).json()
    assert any(output["output_id"] == "out-new" for output in config["outputs"])


def test_import_config_aliases_restore(api):
    payload = {
        "sensors": [
            {
                "sensor_id": "sensor-xyz",
                "name": "Soil Moisture",
                "type": "moisture",
                "channel": 1,
                "interval_seconds": 300,
                "rolling_average_seconds": 0,
                "unit": "%",
            }
        ]
    }
    resp = api.post("/v1/config/import", json=payload, headers=_auth())
    assert resp.status_code == 200
    config = api.get("/v1/config", headers=_auth()).json()
    assert any(sensor["sensor_id"] == "sensor-xyz" for sensor in config["sensors"])


def test_discovery_payload_matches_settings(api):
    settings = get_settings()
    resp = api.get("/v1/discovery")
    data = resp.json()
    assert data["service_type"].endswith("_tcp.local.")
    assert data["mac_eth"] == settings.mac_eth
    props = data["properties"]
    assert props["node_name"] == settings.node_name
    assert props["fw"] == settings.firmware_version
    assert props["hw"] == settings.hardware_model
    assert int(props["sensors"]) >= 0
    assert int(props["outputs"]) >= 0
    assert "capabilities" in props
    assert int(props["uptime_seconds"]) >= 0
    assert "battery_percent" in data
    assert "mesh_nodes" in props
    assert "mesh_health" in props


def test_mesh_status_endpoint_available(api):
    resp = api.get("/v1/mesh")
    assert resp.status_code == 200
    body = resp.json()
    assert "summary" in body and "topology" in body
    assert isinstance(body["topology"], list)


def test_mesh_join_requires_enabled(api):
    resp = api.post("/v1/mesh/join", json={"duration_seconds": 5})
    assert resp.status_code == 400


def test_bluetooth_provision_persists_request(tmp_path, monkeypatch, api):
    # route provision queue to a temp file
    monkeypatch.setattr(
        get_settings(),
        "provision_queue_path",
        str(tmp_path / "provision_queue.json"),
    )
    # rebind store after path change
    api.app.state.provision_store = ProvisioningStore(get_settings().provision_queue_file)

    payload = {
        "device_name": "Test Node",
        "pin": "123456",
        "wifi_ssid": "FarmWiFi",
        "wifi_password": "secret",
    }
    resp = api.post("/v1/provision/bluetooth", json=payload, headers=_auth())
    assert resp.status_code == 200
    body = resp.json()
    assert body["status"] in {"in_progress", "provisioned"}
    queue_file = get_settings().provision_queue_file
    assert queue_file.exists()
    saved = queue_file.read_text()
    assert "FarmWiFi" in saved


def test_provision_queue_returns_entries(tmp_path, monkeypatch, api):
    settings = get_settings()
    queue = tmp_path / "provision_queue.json"
    monkeypatch.setattr(settings, "provision_queue_path", str(queue))
    api.app.state.provision_store = ProvisioningStore(settings.provision_queue_file)

    payload = {
        "device_name": "Test Node",
        "pin": "654321",
        "wifi_ssid": "FarmWiFi",
        "wifi_password": "secret2",
    }
    api.post("/v1/provision/bluetooth", json=payload, headers=_auth())
    resp = api.get("/v1/provision/queue", headers=_auth())
    assert resp.status_code == 200
    body = resp.json()
    assert len(body["pending"]) == 1
    assert body["pending"][0]["device_name"] == "Test Node"

    cleared = api.delete("/v1/provision/queue", headers=_auth())
    assert cleared.status_code == 200
    assert api.get("/v1/provision/queue", headers=_auth()).json()["pending"] == []


def test_provisioning_session_start_only(tmp_path, monkeypatch, api):
    settings = get_settings()
    queue = tmp_path / "provision_queue.json"
    monkeypatch.setattr(settings, "provision_queue_path", str(queue))
    api.app.state.provision_store = ProvisioningStore(settings.provision_queue_file)

    payload = {
        "device_name": "Session Node",
        "wifi_ssid": "FarmSSID",
        "start_only": True,
    }
    resp = api.post("/v1/provisioning/session", json=payload, headers=_auth())
    assert resp.status_code == 200
    body = resp.json()
    assert body["status"] == "session_created"
    sessions = api.get("/v1/provisioning/sessions", headers=_auth()).json()["sessions"]
    assert len(sessions) == 1
    assert sessions[0]["status"] in {"session_created", "applying"}
    detail = api.get(f"/v1/provisioning/session/{sessions[0]['session_id']}", headers=_auth())
    assert detail.status_code == 200


def test_provisioning_session_applies_wifi(tmp_path, monkeypatch, api):
    settings = get_settings()
    queue = tmp_path / "provision_queue.json"
    monkeypatch.setattr(settings, "provision_queue_path", str(queue))
    api.app.state.provision_store = ProvisioningStore(settings.provision_queue_file)

    payload = {
        "device_name": "WiFi Node",
        "wifi_ssid": "FarmWiFi",
        "wifi_password": "secret",
        "adoption_token": "adopt-me",
    }
    resp = api.post("/v1/provisioning/session", json=payload, headers=_auth())
    assert resp.status_code == 200
    body = resp.json()
    assert body["status"] in {"in_progress", "provisioned"}
    config = api.get("/v1/config", headers=_auth()).json()
    assert config["wifi_hints"]["ssid"] == "FarmWiFi"
    apply_status = config["wifi_hints"].get("apply_status")
    assert apply_status is not None
    assert apply_status["state"]


def test_provisioning_wifi_apply_returns_quickly_when_slow(api, monkeypatch):
    def slow_runner(ssid, password):
        time.sleep(0.25)
        return {"state": "applied", "message": "ok", "timestamp": time.time(), "method": "test"}

    monkeypatch.setattr(api.app.state, "wifi_apply_runner", slow_runner, raising=False)

    start = time.monotonic()
    resp = api.post(
        "/v1/provisioning/session",
        headers=_auth(),
        json={
            "device_name": "Slow WiFi",
            "wifi_ssid": "FarmWiFi",
            "wifi_password": "secret",
        },
    )
    elapsed = time.monotonic() - start
    assert resp.status_code == 200
    assert elapsed < 0.5
    assert resp.json()["status"] == "in_progress"

    deadline = time.monotonic() + 3.0
    final_state = None
    while time.monotonic() < deadline:
        cfg = api.get("/v1/config", headers=_auth()).json()
        final_state = cfg.get("wifi_hints", {}).get("apply_status", {}).get("state")
        if final_state == "applied":
            break
        time.sleep(0.05)
    assert final_state == "applied"


def test_update_node_clamps_intervals(api):
    resp = api.patch(
        "/v1/node",
        headers=_auth(),
        json={"heartbeat_interval_seconds": 0, "telemetry_interval_seconds": -5},
    )
    assert resp.status_code == 200
    body = resp.json()
    assert body["heartbeat_interval_seconds"] >= 1.0
    assert body["telemetry_interval_seconds"] >= 1.0


def test_apply_config_rejects_invalid_without_partial_mutation(api):
    settings = get_settings()
    original_name = settings.node_name
    original_heartbeat = settings.heartbeat_interval_seconds

    with pytest.raises(ValueError):
        apply_config(
            settings,
            {"node": {"node_name": "New Name", "heartbeat_interval_seconds": "nope"}},
        )

    assert settings.node_name == original_name
    assert settings.heartbeat_interval_seconds == original_heartbeat


def test_firstboot_config_sets_node_name(tmp_path, monkeypatch, api):
    settings = get_settings()
    firstboot = tmp_path / "node-agent-firstboot.json"
    firstboot.write_text(
        json.dumps(
            {
                "node": {
                    "node_id": "pi-firstboot-01",
                    "node_name": "Firstboot Node",
                    "adoption_token": "deadbeefcafebabe",
                }
            }
        )
    )
    monkeypatch.setattr(settings, "firstboot_path", str(firstboot))

    # Re-run the firstboot hook manually
    from app.bootstrap import apply_firstboot
    apply_firstboot(settings)

    resp = api.get("/v1/config", headers=_auth())
    assert resp.status_code == 200
    config = resp.json()
    assert config["node"]["node_name"] == "Firstboot Node"
    assert config["node"]["node_id"] == "pi-firstboot-01"
    assert not firstboot.exists()


def test_healthz_endpoint(api):
    resp = api.get("/healthz")
    assert resp.json()["status"] == "ok"


def test_simulation_get_returns_profile(api):
    resp = api.get("/v1/simulation")
    if build_info.BUILD_FLAVOR == "prod":
        assert resp.status_code == 404
        return
    assert resp.status_code == 200
    body = resp.json()
    assert "enabled" in body
    assert "jitter" in body


def test_simulation_update_rejects_when_disabled(api):
    if build_info.BUILD_FLAVOR == "prod":
        resp = api.put("/v1/simulation", json={"jitter": {"demo-ads": 0.2}})
        assert resp.status_code == 404
        return

    settings = get_settings()
    original_enabled = settings.simulation.enabled
    try:
        settings.simulation.enabled = False
        resp = api.put("/v1/simulation", json={"jitter": {"demo-ads": 0.2}})
        assert resp.status_code == 400
    finally:
        settings.simulation.enabled = original_enabled


def test_simulation_update_applies_and_persists(tmp_path, api):
    if build_info.BUILD_FLAVOR == "prod":
        resp = api.put("/v1/simulation", json={"label": "lab-1"})
        assert resp.status_code == 404
        return

    settings = get_settings()
    original_profile = settings.simulation.model_copy()
    original_config_path = settings.config_path
    original_store = getattr(api.app.state, "config_store", None)
    original_simulator = getattr(api.app.state, "simulator", None)
    original_outputs = list(settings.outputs)
    original_command_listener = getattr(api.app.state, "command_listener", None)
    try:
        settings.simulation.enabled = True
        settings.config_path = str(tmp_path / "node_config.json")
        settings.outputs = []
        api.app.state.config_store = ConfigStore(settings.config_file)
        simulator = SimulatedDevice(settings.simulation, seed_hint=settings.node_id)
        api.app.state.simulator = simulator
        payload = {
            "label": "lab-1",
            "jitter": {"demo-ads": 0.33},
            "stuck_outputs": ["out-relay-1"],
        }
        resp = api.put("/v1/simulation", json=payload)
        assert resp.status_code == 200
        body = resp.json()
        assert body["label"] == "lab-1"
        assert body["jitter"]["demo-ads"] == 0.33
        assert simulator.profile.label == "lab-1"
        assert simulator.profile.jitter.get("demo-ads") == 0.33
        saved = json.loads(settings.config_file.read_text())
        assert saved["simulation"]["label"] == "lab-1"
        assert saved["simulation"]["jitter"]["demo-ads"] == 0.33
    finally:
        settings.simulation = original_profile
        settings.config_path = original_config_path
        settings.outputs = original_outputs
        api.app.state.config_store = original_store
        api.app.state.simulator = original_simulator
        api.app.state.command_listener = original_command_listener
