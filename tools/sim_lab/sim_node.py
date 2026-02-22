#!/usr/bin/env python3
from __future__ import annotations

import json
import math
import os
import random
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import List

import paho.mqtt.client as mqtt


@dataclass(frozen=True)
class SensorProfile:
    sensor_id: str
    base: float
    amplitude: float
    period: float
    phase: float


@dataclass
class SimNode:
    node_id: str
    sensors: List[SensorProfile]


def _env_int(key: str, default: int) -> int:
    value = os.getenv(key)
    if not value:
        return default
    try:
        return int(value)
    except ValueError:
        return default


def _env_float(key: str, default: float) -> float:
    value = os.getenv(key)
    if not value:
        return default
    try:
        return float(value)
    except ValueError:
        return default


def _env_list(key: str) -> List[str]:
    value = os.getenv(key, "")
    items = [entry.strip() for entry in value.split(",") if entry.strip()]
    return items


def _timestamp() -> str:
    return datetime.now(timezone.utc).isoformat()


def _build_nodes(seed: int) -> List[SimNode]:
    rng = random.Random(seed)
    node_ids = _env_list("NODE_SIM_NODE_IDS")
    if not node_ids:
        count = max(_env_int("NODE_SIM_NODE_COUNT", 2), 1)
        node_ids = [f"sim-node-{idx + 1}" for idx in range(count)]

    sensor_labels = _env_list("NODE_SIM_SENSOR_LABELS")
    if not sensor_labels:
        sensor_labels = ["temperature_c", "moisture_pct", "pressure_kpa"]

    nodes: List[SimNode] = []
    for node_id in node_ids:
        sensors: List[SensorProfile] = []
        for label in sensor_labels:
            sensor_id = f"{node_id}-{label}"
            base = 10.0 + rng.random() * 20.0
            amplitude = 0.4 + rng.random() * 2.0
            period = 30.0 + rng.random() * 90.0
            phase = rng.random() * math.tau
            sensors.append(
                SensorProfile(
                    sensor_id=sensor_id,
                    base=base,
                    amplitude=amplitude,
                    period=period,
                    phase=phase,
                )
            )
        nodes.append(SimNode(node_id=node_id, sensors=sensors))
    return nodes


def _publish_status(client: mqtt.Client, topic_prefix: str, node_id: str) -> None:
    topic = f"{topic_prefix}/{node_id}/status"
    client.publish(topic, "online", qos=0, retain=False)


def _publish_telemetry(
    client: mqtt.Client,
    topic_prefix: str,
    node: SimNode,
    elapsed: float,
) -> None:
    for sensor in node.sensors:
        value = sensor.base + sensor.amplitude * math.sin((elapsed / sensor.period) + sensor.phase)
        payload = {
            "value": round(value, 3),
            "timestamp": _timestamp(),
            "quality": 0,
        }
        topic = f"{topic_prefix}/{node.node_id}/{sensor.sensor_id}/telemetry"
        client.publish(topic, json.dumps(payload), qos=0, retain=False)


def main() -> None:
    mqtt_host = os.getenv("MQTT_HOST", "127.0.0.1")
    mqtt_port = _env_int("MQTT_PORT", 1883)
    mqtt_keepalive = _env_int("MQTT_KEEPALIVE", 60)
    topic_prefix = os.getenv("MQTT_TOPIC_PREFIX", "iot")
    heartbeat_interval = _env_float("NODE_SIM_HEARTBEAT_INTERVAL", 5.0)
    telemetry_interval = _env_float("NODE_SIM_TELEMETRY_INTERVAL", 5.0)
    seed = _env_int("NODE_SIM_SEED", 42)

    nodes = _build_nodes(seed)
    client = mqtt.Client(client_id=f"sim-lab-node-{seed}")

    def _on_connect(_client, _userdata, _flags, rc):
        if rc == 0:
            print(f"[sim-node] connected to MQTT at {mqtt_host}:{mqtt_port}")
        else:
            print(f"[sim-node] MQTT connection failed: {rc}")

    def _on_disconnect(_client, _userdata, rc):
        if rc != 0:
            print("[sim-node] MQTT disconnected unexpectedly")

    client.on_connect = _on_connect
    client.on_disconnect = _on_disconnect

    while True:
        try:
            client.connect(mqtt_host, mqtt_port, mqtt_keepalive)
            break
        except Exception as exc:
            print(f"[sim-node] MQTT connect failed: {exc}")
            time.sleep(2)

    client.loop_start()
    start = time.monotonic()
    last_heartbeat = 0.0
    last_telemetry = 0.0

    try:
        while True:
            now = time.monotonic()
            elapsed = now - start
            if elapsed - last_heartbeat >= heartbeat_interval:
                for node in nodes:
                    _publish_status(client, topic_prefix, node.node_id)
                last_heartbeat = elapsed
            if elapsed - last_telemetry >= telemetry_interval:
                for node in nodes:
                    _publish_telemetry(client, topic_prefix, node, elapsed)
                last_telemetry = elapsed
            time.sleep(0.5)
    except KeyboardInterrupt:
        print("[sim-node] shutting down")
    finally:
        client.loop_stop()
        client.disconnect()


if __name__ == "__main__":
    main()
