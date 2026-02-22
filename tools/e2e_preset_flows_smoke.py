#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import threading
import time
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen


REPO_ROOT = Path(__file__).resolve().parents[1]
REPORTS_DIR = REPO_ROOT / "reports" / "e2e-preset-flows-smoke"
LAST_SETUP_STATE = REPO_ROOT / "reports" / "e2e-setup-smoke" / "last_state.json"
DEFAULT_SETUP_CONFIG = Path("/Users/Shared/FarmDashboard/setup/config.json")


def timestamp_slug() -> str:
    return time.strftime("%Y-%m-%dT%H-%M-%SZ", time.gmtime())


def resolve_last_state_config() -> Path | None:
    if not LAST_SETUP_STATE.exists():
        return None
    try:
        payload = json.loads(LAST_SETUP_STATE.read_text())
    except json.JSONDecodeError:
        return None
    if not payload.get("preserved"):
        return None
    config_path = payload.get("config_path")
    if not isinstance(config_path, str) or not config_path:
        return None
    candidate = Path(config_path)
    if candidate.exists():
        return candidate
    return None


def resolve_setup_config_path() -> Path:
    env_path = os.environ.get("FARM_SETUP_CONFIG")
    if env_path:
        return Path(env_path)
    state_dir = os.environ.get("FARM_SETUP_STATE_DIR")
    if state_dir:
        candidate = Path(state_dir) / "config.json"
        if candidate.exists():
            return candidate
    last_state = resolve_last_state_config()
    if last_state:
        return last_state
    return DEFAULT_SETUP_CONFIG


def load_setup_config(config_path: Path) -> dict[str, Any]:
    if not config_path.exists():
        return {}
    try:
        return json.loads(config_path.read_text())
    except json.JSONDecodeError:
        return {}


def resolve_api_base(config: dict[str, Any]) -> str:
    port = config.get("core_port")
    if isinstance(port, int) and port > 0:
        return f"http://127.0.0.1:{port}"
    return "http://127.0.0.1:8000"


def resolve_node_agent_port(config: dict[str, Any]) -> int:
    port = config.get("node_agent_port")
    if isinstance(port, int) and port > 0:
        return port
    return 9000


def request_json(
    method: str,
    url: str,
    *,
    json_body: Any | None,
    headers: dict[str, str] | None,
) -> tuple[int, Any]:
    payload: bytes | None = None
    effective_headers = {"Accept": "application/json"}
    if headers:
        effective_headers.update(headers)
    if json_body is not None:
        payload = json.dumps(json_body).encode("utf-8")
        effective_headers["Content-Type"] = "application/json"

    req = Request(url, data=payload, method=method, headers=effective_headers)
    try:
        with urlopen(req, timeout=20) as resp:
            status = int(resp.status)
            body_bytes = resp.read()
    except HTTPError as exc:
        status = int(exc.code)
        body_bytes = exc.read()
    except URLError as exc:
        raise RuntimeError(f"Request failed: {method} {url}: {exc}") from exc

    body_text = body_bytes.decode("utf-8", errors="replace") if body_bytes else ""
    if not body_text:
        return status, None
    try:
        return status, json.loads(body_text)
    except json.JSONDecodeError:
        return status, body_text


def wait_for_health(api_base: str, *, timeout_seconds: int) -> None:
    deadline = time.time() + max(1, timeout_seconds)
    last_error = ""
    while time.time() < deadline:
        try:
            status, _ = request_json("GET", f"{api_base}/healthz", json_body=None, headers=None)
            if status == 200:
                return
            last_error = f"status={status}"
        except Exception as exc:  # noqa: BLE001
            last_error = str(exc)
        time.sleep(0.5)
    raise RuntimeError(f"Core server healthz failed ({last_error})")


@dataclass
class NodeAgentCapture:
    config: dict[str, Any]
    put_bodies: list[dict[str, Any]]


class NodeAgentHandler(BaseHTTPRequestHandler):
    server_version = "FarmDashboardNodeAgentMock/1.0"

    def _capture(self) -> NodeAgentCapture:
        return self.server.capture  # type: ignore[attr-defined]

    def _send_json(self, status: int, body: Any) -> None:
        encoded = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A003
        # Silence default stdout logging; test harness writes its own logs.
        return

    def do_GET(self) -> None:  # noqa: N802
        if self.path.split("?", 1)[0] != "/v1/config":
            self._send_json(404, {"error": "not found"})
            return
        self._send_json(200, self._capture().config)

    def do_PUT(self) -> None:  # noqa: N802
        if self.path.split("?", 1)[0] != "/v1/config":
            self._send_json(404, {"error": "not found"})
            return
        length = int(self.headers.get("Content-Length", "0") or "0")
        raw = self.rfile.read(length) if length > 0 else b"{}"
        try:
            body = json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError:
            self._send_json(400, {"error": "invalid json"})
            return
        if not isinstance(body, dict):
            self._send_json(400, {"error": "expected object"})
            return
        capture = self._capture()
        capture.config = body
        capture.put_bodies.append(body)
        self._send_json(200, {"status": "ok"})


@dataclass
class NodeAgentMock:
    port: int
    capture: NodeAgentCapture
    server: ThreadingHTTPServer
    thread: threading.Thread

    @classmethod
    def start(cls, port: int) -> NodeAgentMock:
        capture = NodeAgentCapture(config={}, put_bodies=[])
        server = ThreadingHTTPServer(("127.0.0.1", port), NodeAgentHandler)
        server.capture = capture  # type: ignore[attr-defined]
        thread = threading.Thread(target=server.serve_forever, name="node-agent-mock", daemon=True)
        thread.start()
        return cls(port=port, capture=capture, server=server, thread=thread)

    def stop(self) -> None:
        self.server.shutdown()
        self.server.server_close()


def ensure_admin_token(api_base: str) -> str:
    parsed = urlparse(api_base)
    api_port = parsed.port or (443 if parsed.scheme == "https" else 80)
    email = (
        os.environ.get("FARM_E2E_USER_EMAIL")
        or os.environ.get("FARM_E2E_ADMIN_EMAIL")
        or f"e2e-smoke-{api_port}@farm.local"
    ).strip().lower()
    password = (
        os.environ.get("FARM_E2E_USER_PASSWORD")
        or os.environ.get("FARM_E2E_ADMIN_PASSWORD")
        or "SmokeTest!123"
    )
    capabilities = [
        "outputs.command",
        "alerts.ack",
        "schedules.write",
        "config.write",
        "users.manage",
    ]

    status, body = request_json(
        "POST",
        f"{api_base}/api/auth/login",
        json_body={"email": email, "password": password},
        headers=None,
    )
    if status == 200 and isinstance(body, dict) and body.get("token"):
        return str(body["token"])

    status, body = request_json(
        "POST",
        f"{api_base}/api/users",
        json_body={
            "name": "E2E Smoke",
            "email": email,
            "role": "admin",
            "capabilities": capabilities,
            "password": password,
        },
        headers=None,
    )
    if status not in {200, 201, 409, 401, 403}:
        raise RuntimeError(f"Unexpected user create status {status}: {body}")

    status, body = request_json(
        "POST",
        f"{api_base}/api/auth/login",
        json_body={"email": email, "password": password},
        headers=None,
    )
    if status != 200 or not isinstance(body, dict) or not body.get("token"):
        raise RuntimeError(f"Login failed ({status}): {body}")
    return str(body["token"])


def auth_headers(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def run_renogy_flow(api_base: str, token: str, *, node_agent_port: int) -> None:
    mock = NodeAgentMock.start(node_agent_port)
    try:
        status, node = request_json(
            "POST",
            f"{api_base}/api/nodes",
            json_body={
                "name": "E2E Renogy Node",
                "ip_last": "127.0.0.1",
                "status": "online",
            },
            headers=auth_headers(token),
        )
        if status not in {200, 201} or not isinstance(node, dict) or not node.get("id"):
            raise RuntimeError(f"Failed to create node ({status}): {node}")
        node_id = str(node["id"])

        status, applied = request_json(
            "POST",
            f"{api_base}/api/nodes/{node_id}/presets/renogy-bt2",
            json_body={
                "bt2_address": "AA:BB:CC:DD:EE:FF",
                "poll_interval_seconds": 30,
                "mode": "external",
            },
            headers=auth_headers(token),
        )
        if status != 200 or not isinstance(applied, dict):
            raise RuntimeError(f"Renogy preset apply failed ({status}): {applied}")

        sensors = applied.get("sensors")
        if not isinstance(sensors, list) or not sensors:
            raise RuntimeError(f"Renogy preset response missing sensors: {applied}")

        if not mock.capture.put_bodies:
            raise RuntimeError("Renogy preset did not PUT /v1/config to the node-agent mock")

        pushed = mock.capture.put_bodies[-1]
        renogy_cfg = pushed.get("renogy_bt2")
        if not isinstance(renogy_cfg, dict):
            raise RuntimeError(f"Expected renogy_bt2 config object, got: {renogy_cfg}")
        if not renogy_cfg.get("enabled"):
            raise RuntimeError("Expected renogy_bt2.enabled=true in pushed config")
        if renogy_cfg.get("mode") != "external":
            raise RuntimeError(f"Expected renogy_bt2.mode=external, got {renogy_cfg.get('mode')}")
        if not renogy_cfg.get("ingest_token"):
            raise RuntimeError("Expected renogy_bt2.ingest_token to be set for external mode")

        status, all_sensors = request_json(
            "GET",
            f"{api_base}/api/sensors",
            json_body=None,
            headers=auth_headers(token),
        )
        if status != 200 or not isinstance(all_sensors, list):
            raise RuntimeError(f"Failed to list sensors ({status}): {all_sensors}")

        renogy_core = [
            entry
            for entry in all_sensors
            if isinstance(entry, dict)
            and entry.get("node_id") == node_id
            and isinstance(entry.get("config"), dict)
            and entry.get("config", {}).get("source") == "renogy_bt2"
        ]
        if len(renogy_core) != len(sensors):
            raise RuntimeError(
                f"Renogy sensors not upserted into core DB: expected {len(sensors)}, got {len(renogy_core)}"
            )
    finally:
        mock.stop()


def run_ws_2902_flow(api_base: str, token: str) -> None:
    status, created = request_json(
        "POST",
        f"{api_base}/api/weather-stations/ws-2902",
        json_body={
            "nickname": "E2E Weather",
            "protocol": "wunderground",
            "interval_seconds": 30,
        },
        headers=auth_headers(token),
    )
    if status != 200 or not isinstance(created, dict) or not created.get("id"):
        raise RuntimeError(f"WS-2902 create failed ({status}): {created}")

    integration_id = str(created["id"])
    ingest_path = str(created.get("ingest_path") or "")
    token_value = str(created.get("token") or "")
    sensors = created.get("sensors")
    if not ingest_path or not token_value or not isinstance(sensors, list) or not sensors:
        raise RuntimeError(f"WS-2902 create response incomplete: {created}")

    now = time.time()
    query = {
        "tempf": "72.5",
        "windspeedmph": "5.4",
        "winddir": "180",
        "dailyrainin": "0.12",
        "uv": "3.1",
        "solarradiation": "455",
        "baromin": "29.92",
        "PASSWORD": "secret",
    }
    status, ingest = request_json(
        "GET",
        f"{api_base}{ingest_path}?{urlencode(query)}",
        json_body=None,
        headers=None,
    )
    if status != 200:
        raise RuntimeError(f"WS-2902 ingest failed ({status}): {ingest}")

    status, ws_status = request_json(
        "GET",
        f"{api_base}/api/weather-stations/ws-2902/{integration_id}",
        json_body=None,
        headers=auth_headers(token),
    )
    if status != 200 or not isinstance(ws_status, dict):
        raise RuntimeError(f"WS-2902 status fetch failed ({status}): {ws_status}")

    if not ws_status.get("last_seen"):
        raise RuntimeError(f"Expected last_seen to be set after ingest: {ws_status}")
    last_payload = ws_status.get("last_payload")
    if not isinstance(last_payload, dict) or last_payload.get("PASSWORD") != "[REDACTED]":
        raise RuntimeError(f"Expected sensitive fields redacted in last_payload: {last_payload}")

    temperature_sensor = None
    for entry in sensors:
        if isinstance(entry, dict) and entry.get("type") == "temperature":
            temperature_sensor = entry
            break
    if temperature_sensor is None:
        temperature_sensor = sensors[0] if isinstance(sensors[0], dict) else None
    if not temperature_sensor or not temperature_sensor.get("sensor_id"):
        raise RuntimeError(f"Unable to resolve a weather station sensor id from: {sensors}")

    start = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now - 60))
    end = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now + 60))
    sensor_id = str(temperature_sensor["sensor_id"])
    metrics_url = f"{api_base}/api/metrics/query?sensor_ids={sensor_id}&start={start}&end={end}&interval=1"
    status, metrics = request_json("GET", metrics_url, json_body=None, headers=None)
    if status != 200 or not isinstance(metrics, dict):
        raise RuntimeError(f"Metrics query failed ({status}): {metrics}")
    series = metrics.get("series")
    if not isinstance(series, list) or not series:
        raise RuntimeError(f"Metrics response missing series: {metrics}")
    points = series[0].get("points") if isinstance(series[0], dict) else None
    if not isinstance(points, list) or not points:
        raise RuntimeError(f"Expected metrics points after ingest, got: {series[0]}")

    status, rotated = request_json(
        "POST",
        f"{api_base}/api/weather-stations/ws-2902/{integration_id}/rotate-token",
        json_body=None,
        headers=auth_headers(token),
    )
    if status != 200 or not isinstance(rotated, dict) or not rotated.get("token"):
        raise RuntimeError(f"WS-2902 rotate-token failed ({status}): {rotated}")

    status, old_ingest = request_json(
        "GET",
        f"{api_base}{ingest_path}?{urlencode(query)}",
        json_body=None,
        headers=None,
    )
    if status != 404:
        raise RuntimeError(
            f"Expected old token to be rejected after rotation (404), got {status}: {old_ingest}"
        )


def main() -> int:
    artifacts_dir = REPORTS_DIR / timestamp_slug()
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    log_path = artifacts_dir / "preset_flows.log"

    config_path = resolve_setup_config_path()
    config = load_setup_config(config_path)
    api_base = resolve_api_base(config)
    node_agent_port = resolve_node_agent_port(config)

    try:
        wait_for_health(api_base, timeout_seconds=120)
        token = ensure_admin_token(api_base)
        run_renogy_flow(api_base, token, node_agent_port=node_agent_port)
        run_ws_2902_flow(api_base, token)
    except Exception as exc:
        log_path.write_text(
            "\n".join(
                [
                    "e2e-preset-flows-smoke: FAIL",
                    f"- config_path: {config_path}",
                    f"- api_base: {api_base}",
                    f"- node_agent_port: {node_agent_port}",
                    f"- error: {exc}",
                ]
            )
            + "\n"
        )
        print(f"e2e-preset-flows-smoke: FAIL ({exc})")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    log_path.write_text(
        "\n".join(
            [
                "e2e-preset-flows-smoke: PASS",
                f"- config_path: {config_path}",
                f"- api_base: {api_base}",
                f"- node_agent_port: {node_agent_port}",
            ]
        )
        + "\n"
    )
    print("e2e-preset-flows-smoke: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
