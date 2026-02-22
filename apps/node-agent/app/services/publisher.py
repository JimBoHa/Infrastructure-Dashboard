"""Telemetry publisher responsible for MQTT comms."""
from __future__ import annotations

import asyncio
import json
import logging
import re
import time
from collections import defaultdict, deque
from datetime import datetime, timezone
from typing import Dict, Iterable, List, Optional

import psutil
from aiomqtt import Client, MqttError

try:
    import paho.mqtt.client as mqtt
except ImportError:  # pragma: no cover - fallback if dependency is missing during linting
    mqtt = None  # type: ignore[assignment]
else:
    if not hasattr(mqtt.Client, "message_retry_set"):
        _mqtt_compat_logger = logging.getLogger(__name__)

        def _noop_message_retry_set(self, *args, **kwargs) -> None:  # type: ignore[no-redef]
            _mqtt_compat_logger.debug(
                "paho-mqtt %s lacks message_retry_set; ignoring compatibility shim",
                getattr(mqtt, "__version__", "unknown"),
            )

        mqtt.Client.message_retry_set = _noop_message_retry_set  # type: ignore[attr-defined]

from app.config import SensorConfig, Settings
from app import build_info
from app.generated_api.generated_api.models.node_status_payload import NodeStatusPayload
from app.observability import attach_request_id, generate_request_id
from app.hardware import AnalogReader, MeshAdapter, MeshSample, PulseInputDriver, RenogyBt2Collector
from app.services.latency_probe import LatencyProbe
from app.services.node_forwarder_client import NodeForwarderClient
from app.services.simulator import SimulatedDevice

logger = logging.getLogger(__name__)
COV_TOLERANCE = 1e-6
SENSOR_TYPE_ALIASES = {
    # Legacy driver labels map to the generic analog driver.
    "ads1263": "analog",
    "ads1115": "analog",
    "temperature": "analog",
    "temp": "analog",
    "moisture": "analog",
    "soil_moisture": "analog",
    "humidity": "analog",
    "pressure": "analog",
    "water_level": "analog",
    "wind": "analog",
    "wind_speed": "analog",
    "wind_direction": "analog",
    "lux": "analog",
    "solar": "analog",
    "irradiance": "analog",
    "chemical_level": "analog",
    "fertilizer": "analog",
    "current": "analog",
    "voltage": "analog",
    "power": "analog",
    "power_kw": "analog",
    "power_w": "analog",
    "power_watts": "analog",
}
ANALOG_SENSOR_TYPES = {
    "analog",
}
PULSE_SENSOR_TYPES = {
    "pulse",
    "gpio_pulse",
    "flow",
    "flow_meter",
    "rain",
    "rain_gauge",
}


def normalize_sensor_type(sensor_type: str | None) -> str:
    if not sensor_type:
        return ""
    cleaned = sensor_type.strip().lower()
    cleaned = cleaned.replace("-", "_").replace(" ", "_")
    cleaned = re.sub(r"[^a-z0-9_]+", "_", cleaned).strip("_")
    return SENSOR_TYPE_ALIASES.get(cleaned, cleaned)


# Backwards compatibility: older modules import the underscored name.
_normalize_sensor_type = normalize_sensor_type


def _preferred_ipv4() -> str:
    import socket

    try:
        hostname = socket.gethostname()
        ip = socket.gethostbyname(hostname)
        if ip.startswith("127."):
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
                sock.connect(("8.8.8.8", 80))
                ip = sock.getsockname()[0]
        return ip
    except OSError:
        return "127.0.0.1"


class TelemetryPublisher:
    def __init__(
        self,
        settings: Settings,
        analog_driver: AnalogReader,
        pulse_driver: PulseInputDriver,
        mesh_adapter: Optional[MeshAdapter] = None,
        renogy_collector: Optional[RenogyBt2Collector] = None,
        latency_probe: LatencyProbe | None = None,
        mqtt_rtt_probe: LatencyProbe | None = None,
        simulator: SimulatedDevice | None = None,
    ):
        if build_info.BUILD_FLAVOR == "prod" and simulator is not None:
            raise RuntimeError("Simulation is not allowed in production builds")
        self.settings = settings
        self.analog = analog_driver
        self.pulse = pulse_driver
        self.mesh = mesh_adapter
        self.renogy = renogy_collector
        self.simulator = simulator
        if latency_probe is None:
            latency_probe = LatencyProbe(
                probe_kind="icmp",
                target_host=settings.mqtt_host,
                target_port=0,
                interval_seconds=5.0,
                window_samples=360,
            )
            latency_probe.start()
        if mqtt_rtt_probe is None:
            if latency_probe is not None and not isinstance(latency_probe, LatencyProbe):
                mqtt_rtt_probe = latency_probe
            else:
                mqtt_rtt_probe = LatencyProbe(
                    probe_kind="tcp",
                    target_host=settings.mqtt_host,
                    target_port=settings.mqtt_port,
                    interval_seconds=5.0,
                    window_samples=24,
                )
                mqtt_rtt_probe.start()
        self.ping_probe = latency_probe
        self.mqtt_rtt_probe = mqtt_rtt_probe
        self._sampling_task: asyncio.Task | None = None
        self._status_task: asyncio.Task | None = None
        self._rolling_task: asyncio.Task | None = None
        self._stop_event = asyncio.Event()
        self._started = time.monotonic()
        self._loop: Optional[asyncio.AbstractEventLoop] = None  # event loop reference used for mesh callbacks
        self._mesh_buffer: deque[MeshSample] = deque()
        self._mesh_stats: Dict[str, Dict[str, float]] = defaultdict(lambda: {"count": 0, "lqi_total": 0.0})
        self._cov_last_values: Dict[str, float] = {}
        self._pulse_last_counts: Dict[str, int] = {}
        self._rolling_buffers: Dict[str, deque[tuple[float, float]]] = {}
        self._display_latest: Dict[str, Dict[str, object]] = {}
        self._display_history: Dict[str, deque[tuple[str, float]]] = {}
        self.mqtt_connected: bool = False
        self.last_publish_at: Optional[datetime] = None
        self.last_mqtt_error: Optional[str] = None
        self.forwarder: NodeForwarderClient = NodeForwarderClient(settings.node_forwarder_url)
        self._forwarder_queue: deque[Dict[str, object]] = deque()
        self._forwarder_queue_max: int = 10_000
        self.forwarder_dropped_samples: int = 0
        self.last_forwarder_error: Optional[str] = None
        self.last_forwarder_status: Optional[Dict[str, object]] = None
        if self.mesh:
            self.mesh.add_listener(self.ingest_mesh_sample)

    def display_snapshot(self) -> Dict[str, object]:
        """Snapshot data for the optional local display UI (no hardware reads)."""

        return {
            "mqtt_connected": bool(self.mqtt_connected),
            "last_publish_at": self.last_publish_at.isoformat() if self.last_publish_at else None,
            "last_mqtt_error": self.last_mqtt_error,
            "forwarder": {
                "queue_len": len(self._forwarder_queue),
                "dropped_samples": int(self.forwarder_dropped_samples),
                "last_error": self.last_forwarder_error,
                "last_status": self.last_forwarder_status,
            },
            "latest": {key: dict(value) for key, value in self._display_latest.items()},
        }

    def display_history(self, sensor_id: str, *, max_points: int = 400) -> List[Dict[str, object]]:
        buf = self._display_history.get(sensor_id)
        if not buf:
            return []
        points = list(buf)[-max(int(max_points), 1):]
        return [{"timestamp": ts, "value": value} for ts, value in points]

    def start(self) -> None:
        if self._sampling_task and not self._sampling_task.done():
            return
        self._stop_event.clear()
        self._rolling_task = asyncio.create_task(self._rolling_sampler(), name="rolling-sampler")
        self._sampling_task = asyncio.create_task(self._run_sampling(), name="telemetry-sampler")
        self._status_task = asyncio.create_task(self._run_status(), name="status-publisher")

    async def stop(self) -> None:
        self._stop_event.set()
        tasks = [task for task in (self._rolling_task, self._sampling_task, self._status_task) if task]
        for task in tasks:
            try:
                await task
            except asyncio.CancelledError:
                pass
        for probe in (self.ping_probe, self.mqtt_rtt_probe):
            stop_fn = getattr(probe, "stop", None)
            if callable(stop_fn):
                stop_fn()
        try:
            await self.forwarder.aclose()
        except Exception:
            logger.debug("Unable to close node-forwarder client", exc_info=True)

    async def _run_sampling(self) -> None:
        """Always-sample loop that forwards samples to the local node-forwarder."""

        self._loop = asyncio.get_running_loop()
        loop = asyncio.get_running_loop()
        cov_poll_interval = min(self.settings.telemetry_interval_seconds, 1.0)
        next_publish: Dict[str, float] = {sensor.sensor_id: loop.time() for sensor in self.settings.sensors}
        last_flush_error_log = 0.0

        while not self._stop_event.is_set():
            sensor_ids = {sensor.sensor_id for sensor in self.settings.sensors}
            for sensor_id in list(next_publish.keys()):
                if sensor_id in sensor_ids:
                    continue
                next_publish.pop(sensor_id, None)
                self._cov_last_values.pop(sensor_id, None)
                self._pulse_last_counts.pop(sensor_id, None)
                self._rolling_buffers.pop(sensor_id, None)
                self._display_latest.pop(sensor_id, None)
                self._display_history.pop(sensor_id, None)

            now = loop.time()
            await self._collect_samples_due(now, next_publish, cov_poll_interval)
            flushed = await self._flush_forwarder_queue()
            if not flushed:
                # Avoid log spam if the forwarder is down; emit at most once per 30s.
                if now - last_flush_error_log >= 30.0:
                    last_flush_error_log = now
                    if self.last_forwarder_error:
                        logger.warning("node-forwarder push failing: %s", self.last_forwarder_error)

            soonest_sensor = min(next_publish.values()) if next_publish else now + 1.0
            if self._forwarder_queue:
                sleep_for = 0.25
            else:
                sleep_for = max(min(soonest_sensor - loop.time(), 1.0), 0.05)
            try:
                await asyncio.wait_for(self._stop_event.wait(), timeout=sleep_for)
            except asyncio.TimeoutError:
                continue

    async def _collect_samples_due(
        self,
        now: float,
        next_publish: Dict[str, float],
        cov_poll_interval: float,
    ) -> None:
        batch: List[Dict[str, object]] = []
        for sensor in self.settings.sensors:
            due_at = next_publish.get(sensor.sensor_id, now)
            is_due = sensor.interval_seconds == 0 or now >= due_at
            if not is_due:
                continue

            value, quality = self._read_sensor(sensor)
            if value is None:
                next_publish[sensor.sensor_id] = now + (
                    cov_poll_interval if sensor.interval_seconds == 0 else sensor.interval_seconds
                )
                continue

            self._record_display_reading(sensor, float(value), int(quality))

            if sensor.interval_seconds == 0:
                prev = self._cov_last_values.get(sensor.sensor_id)
                if quality == 0 and prev is not None and abs(float(value) - float(prev)) <= COV_TOLERANCE:
                    next_publish[sensor.sensor_id] = now + cov_poll_interval
                    continue

            ts_ms = int(time.time() * 1000)
            batch.append(
                {
                    "sensor_id": sensor.sensor_id,
                    "timestamp_ms": ts_ms,
                    "value": float(value),
                    "quality": int(quality),
                }
            )
            if sensor.interval_seconds == 0 and quality == 0:
                self._cov_last_values[sensor.sensor_id] = float(value)

            if sensor.interval_seconds == 0:
                next_publish[sensor.sensor_id] = now + cov_poll_interval
            else:
                next_publish[sensor.sensor_id] = now + sensor.interval_seconds

        if batch:
            self._enqueue_forwarder_samples(batch)

    def _enqueue_forwarder_samples(self, samples: List[Dict[str, object]]) -> None:
        for sample in samples:
            if len(self._forwarder_queue) >= self._forwarder_queue_max:
                self._forwarder_queue.popleft()
                self.forwarder_dropped_samples += 1
            self._forwarder_queue.append(sample)

    async def _flush_forwarder_queue(self, *, batch_size: int = 200) -> bool:
        if not self._forwarder_queue:
            self.last_forwarder_error = None
            return True

        batch: List[Dict[str, object]] = []
        for _ in range(min(len(self._forwarder_queue), max(batch_size, 1))):
            batch.append(self._forwarder_queue.popleft())

        try:
            await self.forwarder.push_samples(batch)
            self.last_forwarder_error = None
            return True
        except Exception as exc:
            self.last_forwarder_error = str(exc)
            # Requeue the batch at the front preserving order.
            for item in reversed(batch):
                self._forwarder_queue.appendleft(item)
            return False

    async def _run_status(self) -> None:
        reconnect_delay = 5
        while not self._stop_event.is_set():
            try:
                logger.info("Connecting to MQTT broker %s:%s", self.settings.mqtt_host, self.settings.mqtt_port)
                self._loop = asyncio.get_running_loop()
                async with Client(
                    self.settings.mqtt_host,
                    port=self.settings.mqtt_port,
                    username=self.settings.mqtt_username,
                    password=self.settings.mqtt_password,
                ) as client:
                    self.mqtt_connected = True
                    self.last_mqtt_error = None
                    try:
                        await self._loop_run_status(client)
                    finally:
                        self.mqtt_connected = False
            except MqttError as exc:
                self.mqtt_connected = False
                self.last_mqtt_error = str(exc)
                logger.warning("MQTT error %s; retrying", exc)
                await asyncio.sleep(reconnect_delay)
            except RuntimeError as exc:
                # pytest teardown closes the event loop while the publisher is still
                # attempting to connect; avoid noisy tracebacks on shutdown.
                if "cannot schedule new futures after shutdown" in str(exc):
                    logger.info("Telemetry loop stopping: event loop is closing")
                    break
                logger.exception("Unhandled telemetry loop error")
                await asyncio.sleep(reconnect_delay)
            except Exception:
                logger.exception("Unhandled telemetry loop error")
                await asyncio.sleep(reconnect_delay)

    async def _loop_run_status(self, client: Client) -> None:
        heartbeat_interval = self.settings.heartbeat_interval_seconds
        loop = asyncio.get_running_loop()
        next_heartbeat_at = loop.time() + heartbeat_interval
        while not self._stop_event.is_set():
            now = loop.time()
            if now >= next_heartbeat_at:
                await self._publish_heartbeat(client)
                next_heartbeat_at = now + heartbeat_interval
            sleep_for = max(next_heartbeat_at - loop.time(), 0.05)
            try:
                await asyncio.wait_for(self._stop_event.wait(), timeout=sleep_for)
            except asyncio.TimeoutError:
                continue

    async def _publish_sensor_batch(self, forwarder) -> None:
        """Legacy helper used by unit tests; forwards all sensors immediately."""

        loop = asyncio.get_running_loop()
        now = loop.time()
        next_publish: Dict[str, float] = {sensor.sensor_id: now for sensor in self.settings.sensors}
        await self._forward_sensors_due(now, forwarder, next_publish, cov_poll_interval=min(self.settings.telemetry_interval_seconds, 1.0))

    async def _forward_sensors_due(
        self,
        now: float,
        forwarder,
        next_publish: Dict[str, float],
        cov_poll_interval: float,
    ) -> None:
        batch: List[Dict[str, object]] = []
        for sensor in self.settings.sensors:
            due_at = next_publish.get(sensor.sensor_id, now)
            is_due = sensor.interval_seconds == 0 or now >= due_at
            if not is_due:
                continue

            value, quality = self._read_sensor(sensor)
            if value is None:
                next_publish[sensor.sensor_id] = now + (cov_poll_interval if sensor.interval_seconds == 0 else sensor.interval_seconds)
                continue

            self._record_display_reading(sensor, float(value), int(quality))

            if sensor.interval_seconds == 0:
                prev = self._cov_last_values.get(sensor.sensor_id)
                if quality == 0 and prev is not None and abs(float(value) - float(prev)) <= COV_TOLERANCE:
                    next_publish[sensor.sensor_id] = now + cov_poll_interval
                    continue

            batch.append(
                {
                    "sensor_id": sensor.sensor_id,
                    "timestamp_ms": int(time.time() * 1000),
                    "value": float(value),
                    "quality": int(quality),
                }
            )
            if sensor.interval_seconds == 0 and quality == 0:
                self._cov_last_values[sensor.sensor_id] = float(value)

            if sensor.interval_seconds == 0:
                next_publish[sensor.sensor_id] = now + cov_poll_interval
            else:
                next_publish[sensor.sensor_id] = now + sensor.interval_seconds

        if not batch:
            return

        try:
            await forwarder.push_samples(batch)
            self.last_forwarder_error = None
        except Exception as exc:
            self.last_forwarder_error = str(exc)

    def _collect_samples(self, sensor: SensorConfig, samples: int) -> List[float]:
        values: List[float] = []
        for _ in range(samples):
            reading, quality = self._measure_sensor(sensor)
            if reading is not None and quality == 0:
                values.append(float(reading))
        return values

    def _read_sensor(self, sensor: SensorConfig) -> tuple[float | None, int]:
        if sensor.rolling_average_seconds and sensor.rolling_average_seconds > 0:
            buffer = self._rolling_buffers.get(sensor.sensor_id)
            if buffer:
                values = [sample[1] for sample in buffer]
                if values:
                    return sum(values) / len(values), 0

            # Fallback path (startup/tests): sample a bounded number of readings immediately.
            sample_count = min(
                max(int(sensor.rolling_average_seconds * self.settings.rolling_sample_rate_hz), 1),
                100,
            )
            values = self._collect_samples(sensor, sample_count)
            if values:
                return sum(values) / len(values), 0
            return self._measure_sensor(sensor)
        return self._measure_sensor(sensor)

    def _measure_sensor(self, sensor: SensorConfig) -> tuple[float | None, int]:
        type_key = normalize_sensor_type(sensor.type)
        if type_key in ANALOG_SENSOR_TYPES:
            health = getattr(self.analog, "health", None)
            if health is not None and not bool(getattr(health, "ok", True)):
                return None, 4
            reading = self.analog.read_voltage(sensor.channel, sensor.negative_channel)
            if reading is None:
                return None, 4
            volts = float(reading)
            if sensor.current_loop_shunt_ohms is not None or sensor.current_loop_range_m is not None:
                value, quality = self._convert_current_loop_depth(sensor, volts)
                # Fail-closed: do not publish plausible depth values for faulted current-loop sensors.
                if int(quality) != 0:
                    return None, int(quality)
                return float(value), int(quality)
            return self._apply_scaling(sensor, volts), 0
        if type_key in PULSE_SENSOR_TYPES:
            try:
                current = int(self.pulse.read_pulses(sensor.channel))
            except Exception as exc:  # pragma: no cover - defensive
                logger.warning("Pulse read failed for %s (channel %s): %s", sensor.sensor_id, sensor.channel, exc)
                return 0.0, 4

            last = self._pulse_last_counts.get(sensor.sensor_id)
            if last is None:
                delta = current
            else:
                delta = current - last
                if delta < 0:
                    delta = current
            self._pulse_last_counts[sensor.sensor_id] = current
            return self._apply_scaling(sensor, float(delta)), 0
        if type_key == "renogy_bt2":
            external_only = bool(
                self.settings.renogy_bt2.enabled and self.settings.renogy_bt2.mode == "external"
            )
            if self.renogy:
                value = self.renogy.read_metric(sensor.metric)
                if value is not None:
                    return float(value), 0
            if external_only:
                logger.debug("Renogy BT-2 sensor %s awaiting external ingest", sensor.sensor_id)
                return None, 0
            if self.simulator:
                return self.simulator.read_sensor(sensor), 0
            logger.debug("Renogy BT-2 sensor %s missing collector", sensor.sensor_id)
            return None, 0
        if self.simulator:
            return self.simulator.read_sensor(sensor), 0
        logger.warning("Unknown sensor type %s", sensor.type)
        return None, 0

    async def _rolling_sampler(self) -> None:
        """Sample rolling-average sensors at a fixed cadence into bounded buffers."""

        loop = asyncio.get_running_loop()
        sample_rate = max(int(self.settings.rolling_sample_rate_hz or 10), 1)
        interval = 1.0 / float(sample_rate)
        max_window_seconds = 3600.0

        while not self._stop_event.is_set():
            now = loop.time()
            rolling_ids = set()
            for sensor in self.settings.sensors:
                window = float(sensor.rolling_average_seconds or 0.0)
                if window <= 0:
                    continue
                rolling_ids.add(sensor.sensor_id)
                value, quality = self._measure_sensor(sensor)
                if value is None or quality != 0:
                    continue
                buf = self._rolling_buffers.setdefault(sensor.sensor_id, deque())
                buf.append((now, float(value)))
                cutoff = now - window
                while buf and buf[0][0] < cutoff:
                    buf.popleft()
                cap = int(min(window, max_window_seconds) * sample_rate) + 10
                while len(buf) > cap:
                    buf.popleft()

            for sensor_id in list(self._rolling_buffers.keys()):
                if sensor_id not in rolling_ids:
                    self._rolling_buffers.pop(sensor_id, None)

            try:
                await asyncio.wait_for(self._stop_event.wait(), timeout=interval)
            except asyncio.TimeoutError:
                continue

    @staticmethod
    def _apply_scaling(sensor: SensorConfig, value: float) -> float:
        scaled = float(value)
        type_key = normalize_sensor_type(sensor.type)
        if type_key in PULSE_SENSOR_TYPES and sensor.pulses_per_unit and sensor.pulses_per_unit > 0:
            scaled = scaled / sensor.pulses_per_unit

        if type_key in ANALOG_SENSOR_TYPES:
            if sensor.input_min is not None and sensor.input_max is not None and sensor.output_max is not None:
                input_span = sensor.input_max - sensor.input_min
                if input_span != 0:
                    output_min = sensor.output_min or 0.0
                    output_span = sensor.output_max - output_min
                    scaled = output_min + ((scaled - sensor.input_min) / input_span) * output_span

        if sensor.offset:
            scaled += sensor.offset
        if sensor.scale not in (None, 1, 1.0):
            scaled *= float(sensor.scale)
        return scaled

    @staticmethod
    def _convert_current_loop_depth(sensor: SensorConfig, volts: float) -> tuple[float, int]:
        """Convert shunt voltage to depth reading.

        Quality mapping:
        - 0: ok
        - 1: low current fault (open circuit / sensor fault)
        - 2: high current fault (overrange / wiring fault)
        - 3: configuration error (treated as low fault)
        """

        shunt = sensor.current_loop_shunt_ohms
        range_m = sensor.current_loop_range_m
        if not shunt or not range_m or shunt <= 0 or range_m <= 0:
            return 0.0, 3
        span_ma = float(sensor.current_loop_span_ma)
        if span_ma <= 0:
            return 0.0, 3

        current_ma = 1000.0 * float(volts) / float(shunt)
        quality = 0
        if current_ma < float(sensor.current_loop_fault_low_ma):
            quality = 1
        elif current_ma > float(sensor.current_loop_fault_high_ma):
            quality = 2

        depth_m = (current_ma - float(sensor.current_loop_zero_ma)) / span_ma * float(range_m)
        depth_m = max(0.0, min(float(range_m), depth_m))

        unit = (sensor.unit or "m").strip().lower()
        if unit in {"ft", "feet"}:
            value = depth_m * 3.28084
        elif unit in {"in", "inch", "inches"}:
            value = depth_m * 39.37007874
        elif unit in {"cm"}:
            value = depth_m * 100.0
        elif unit in {"mm"}:
            value = depth_m * 1000.0
        else:
            value = depth_m
        if sensor.offset:
            value += float(sensor.offset)
        if sensor.scale not in (None, 1, 1.0):
            value *= float(sensor.scale)
        return float(value), int(quality)

    def ingest_mesh_sample(self, sample: MeshSample | Iterable[MeshSample]) -> None:
        """Accept samples originating from the mesh adapter."""
        if isinstance(sample, MeshSample):
            self._queue_mesh_sample(sample)
            return
        for item in sample:
            if isinstance(item, MeshSample):
                self._queue_mesh_sample(item)

    def _queue_mesh_sample(self, sample: MeshSample) -> None:
        self._update_mesh_summary(sample)
        if self._loop and self._loop.is_running():
            try:
                current = asyncio.get_running_loop()
            except RuntimeError:
                current = None
            if current is not self._loop:
                self._loop.call_soon_threadsafe(self._mesh_buffer.append, sample)
                return
        self._mesh_buffer.append(sample)

    async def _publish_mesh_samples(self, client: Client) -> None:
        if not self._mesh_buffer:
            return
        now = datetime.now(timezone.utc)
        max_age = self.settings.mesh.max_backfill_seconds
        while self._mesh_buffer:
            sample = self._mesh_buffer.popleft()
            age = (now - sample.timestamp).total_seconds()
            if age > max_age:
                logger.debug("Skipping mesh sample %s older than %ss", sample.identifier(), max_age)
                continue
            payload = sample.as_payload()
            payload.update(
                {
                    "source": "mesh",
                    "node_id": self.settings.node_id,
                    "age_seconds": age,
                }
            )
            attach_request_id(payload, request_id=generate_request_id())
            topic = f"iot/{self.settings.node_id}/mesh/{sample.identifier()}/telemetry"
            try:
                await client.publish(topic, json.dumps(payload).encode("utf-8"))
                logger.debug("Published mesh telemetry %s -> %s", sample.identifier(), payload)
            except Exception as exc:
                logger.debug("Dropping mesh sample %s due to publish error: %s", sample.identifier(), exc)

    def _update_mesh_summary(self, sample: MeshSample) -> None:
        summary = self.settings.mesh_summary
        diag = sample.diagnostics
        summary.last_updated = sample.timestamp
        if diag.parent:
            summary.last_parent = diag.parent
        if diag.battery_percent is not None:
            summary.last_battery_percent = diag.battery_percent
        if diag.rssi is not None:
            summary.last_rssi = diag.rssi
        stats = self._mesh_stats[sample.ieee]
        if diag.lqi is not None:
            stats["count"] += 1
            stats["lqi_total"] += float(diag.lqi)
        if diag.link_margin is not None:
            stats["link_margin_total"] = stats.get("link_margin_total", 0.0) + float(diag.link_margin)
        if diag.snr is not None:
            stats["snr_total"] = stats.get("snr_total", 0.0) + float(diag.snr)
        summary.node_count = len(self._mesh_stats)
        total_samples = sum(entry["count"] for entry in self._mesh_stats.values())
        total_lqi = sum(entry["lqi_total"] for entry in self._mesh_stats.values())
        if total_samples:
            summary.average_link_quality = total_lqi / total_samples
            total_link_margin = sum(entry.get("link_margin_total", 0.0) for entry in self._mesh_stats.values())
            total_snr = sum(entry.get("snr_total", 0.0) for entry in self._mesh_stats.values())
            if total_link_margin:
                summary.mesh.health_details["avg_link_margin"] = round(total_link_margin / total_samples, 2)
            if total_snr:
                summary.mesh.health_details["avg_snr"] = round(total_snr / total_samples, 2)
        if summary.health in {"unknown", "starting"}:
            summary.health = "online"

    def _system_metrics_snapshot(self) -> Dict[str, object]:
        cpu_per_core = psutil.cpu_percent(interval=0.0, percpu=True) or []
        if cpu_per_core:
            cpu_percent = sum(cpu_per_core) / len(cpu_per_core)
        else:
            cpu_percent = psutil.cpu_percent(interval=0.0)
        memory = psutil.virtual_memory()
        disk = psutil.disk_usage("/")
        uptime_seconds = int(time.monotonic() - self._started)
        return {
            "cpu_percent": float(cpu_percent),
            "cpu_percent_per_core": [float(value) for value in cpu_per_core],
            "memory_percent": float(memory.percent),
            "memory_used_bytes": int(memory.used),
            "storage_used_bytes": int(disk.used),
            "uptime_seconds": uptime_seconds,
            "ip": _preferred_ipv4(),
        }

    def _latency_metrics(
        self,
    ) -> tuple[
        Optional[float],
        Optional[float],
        Optional[float],
        Optional[float],
        Optional[float],
        Optional[float],
    ]:
        ping_snapshot = self.ping_probe.snapshot()
        mqtt_snapshot = self.mqtt_rtt_probe.snapshot()
        ping_ms = ping_snapshot.last_latency_ms or ping_snapshot.avg_latency_ms
        ping_jitter_ms = ping_snapshot.jitter_ms
        ping_p50_30m_ms = ping_snapshot.p50_latency_ms_30m
        mqtt_rtt_ms = mqtt_snapshot.last_latency_ms or mqtt_snapshot.avg_latency_ms
        mqtt_rtt_jitter_ms = mqtt_snapshot.jitter_ms
        return (
            ping_ms,
            ping_jitter_ms,
            ping_snapshot.uptime_percent_24h,
            ping_p50_30m_ms,
            mqtt_rtt_ms if mqtt_rtt_ms is not None else None,
            mqtt_rtt_jitter_ms,
        )

    def _build_outputs_payload(self) -> Optional[List[Dict[str, object]]]:
        if not self.settings.outputs:
            return None
        simulated = self.simulator.snapshot_outputs() if self.simulator else {}
        payload: List[Dict[str, object]] = []
        for output in self.settings.outputs:
            state = simulated.get(output.output_id) if simulated else None
            if state is None:
                state = output.state or output.default_state or "unknown"
            payload.append(
                {
                    "output_id": output.output_id,
                    "name": output.name,
                    "type": output.type,
                    "state": state,
                }
            )
        return payload

    async def _publish_heartbeat(self, client: Client) -> None:
        status = self.simulator.heartbeat_status() if self.simulator else "online"
        topic = f"iot/{self.settings.node_id}/status"
        metrics = await asyncio.to_thread(self._system_metrics_snapshot)
        (
            ping_ms,
            ping_jitter_ms,
            uptime_percent_24h,
            ping_p50_30m_ms,
            mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms,
        ) = self._latency_metrics()
        forwarder_status = await self.forwarder.get_status()
        self.last_forwarder_status = forwarder_status  # may be None if unavailable
        payload = NodeStatusPayload(
            status=status,
            node_id=self.settings.node_id,
            name=self.settings.node_name,
            ts=datetime.now(timezone.utc),
            uptime_seconds=int(metrics["uptime_seconds"]),
            uptime_percent_24h=uptime_percent_24h,
            cpu_percent=float(metrics["cpu_percent"]),
            cpu_percent_per_core=metrics.get("cpu_percent_per_core"),
            memory_percent=float(metrics["memory_percent"]),
            memory_used_bytes=int(metrics["memory_used_bytes"]),
            storage_used_bytes=int(metrics["storage_used_bytes"]),
            heartbeats=self.settings.heartbeat_interval_seconds,
            mesh=self.settings.mesh_summary.model_dump(),
            outputs=self._build_outputs_payload(),
            request_id=generate_request_id(),
            network_latency_ms=mqtt_broker_rtt_ms,
            network_jitter_ms=mqtt_broker_rtt_jitter_ms,
        )
        payload_data = payload.model_dump(mode="json", exclude_none=True)
        if metrics.get("ip"):
            payload_data["ip"] = metrics["ip"]
        if self.settings.mac_eth:
            payload_data["mac_eth"] = self.settings.mac_eth
        if self.settings.mac_wifi:
            payload_data["mac_wifi"] = self.settings.mac_wifi
        if ping_ms is not None:
            payload_data["ping_ms"] = float(ping_ms)
        if ping_jitter_ms is not None:
            payload_data["ping_jitter_ms"] = float(ping_jitter_ms)
        if ping_p50_30m_ms is not None:
            payload_data["ping_p50_30m_ms"] = float(ping_p50_30m_ms)
        if mqtt_broker_rtt_ms is not None:
            payload_data["mqtt_broker_rtt_ms"] = float(mqtt_broker_rtt_ms)
        if mqtt_broker_rtt_jitter_ms is not None:
            payload_data["mqtt_broker_rtt_jitter_ms"] = float(mqtt_broker_rtt_jitter_ms)

        backend = getattr(self.analog, "backend", None)
        if backend is not None:
            payload_data["analog_backend"] = str(backend)

        health = getattr(self.analog, "health", None)
        if health is not None:
            last_ok_at = getattr(health, "last_ok_at", None)
            payload_data["analog_health"] = {
                "ok": bool(getattr(health, "ok", False)),
                "chip_id": getattr(health, "chip_id", None),
                "last_error": getattr(health, "last_error", None),
                "last_ok_at": last_ok_at.isoformat() if last_ok_at is not None else None,
            }

        payload_data["forwarder"] = {
            "queue_len": len(self._forwarder_queue),
            "dropped_samples": int(self.forwarder_dropped_samples),
            "last_error": self.last_forwarder_error,
            "spool": forwarder_status,
        }

        await client.publish(topic, json.dumps(payload_data).encode("utf-8"))
        self.last_publish_at = datetime.now(timezone.utc)

    def _record_display_reading(self, sensor: SensorConfig, value: float, quality: int) -> None:
        ts = datetime.now(timezone.utc)
        payload: Dict[str, object] = {
            "sensor_id": sensor.sensor_id,
            "name": sensor.name,
            "unit": sensor.unit,
            "value": float(value),
            "quality": int(quality),
            "timestamp": ts.isoformat(),
        }
        self._display_latest[sensor.sensor_id] = payload
        buf = self._display_history.setdefault(sensor.sensor_id, deque())
        buf.append((ts.isoformat(), float(value)))
        while len(buf) > 4000:
            buf.popleft()
