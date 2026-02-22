"""Simulation helpers used by Sim Lab runs."""
from __future__ import annotations

import math
import random
import re
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Dict, Optional

from app.config import SensorConfig, SimulationProfile
from app import build_info


@dataclass
class CommandResult:
    output_id: str
    requested_state: str
    applied_state: str
    stuck: bool = False


RENOGY_METRIC_PROFILE = {
    "pv_power_w": (120.0, 80.0, 6.0, 0.0, None),
    "pv_voltage_v": (18.0, 3.0, 6.0, 0.0, None),
    "pv_current_a": (5.0, 2.0, 6.0, 0.0, None),
    "pv_energy_today_kwh": (2.5, 1.5, 120.0, 0.0, None),
    "pv_energy_total_kwh": (680.0, 20.0, 600.0, 0.0, None),
    "battery_soc_percent": (72.0, 10.0, 30.0, 0.0, 100.0),
    "battery_voltage_v": (12.6, 0.4, 12.0, 11.0, 14.6),
    "battery_current_a": (2.0, 4.0, 8.0, -12.0, 12.0),
    "battery_temp_c": (24.0, 4.0, 14.0, -10.0, 55.0),
    "controller_temp_c": (28.0, 5.0, 14.0, -10.0, 65.0),
    "load_power_w": (50.0, 30.0, 9.0, 0.0, None),
    "load_voltage_v": (12.2, 0.4, 9.0, 10.0, 14.5),
    "load_current_a": (3.0, 1.5, 9.0, 0.0, None),
    "runtime_hours": (8.0, 2.0, 18.0, 0.0, None),
}
SENSOR_TYPE_ALIASES = {
    "power_kw": "power",
    "power_w": "power",
    "power_watts": "power",
}


def _normalize_sensor_type(sensor_type: str | None) -> str:
    if not sensor_type:
        return ""
    cleaned = sensor_type.strip().lower()
    cleaned = cleaned.replace("-", "_").replace(" ", "_")
    cleaned = re.sub(r"[^a-z0-9_]+", "_", cleaned).strip("_")
    return SENSOR_TYPE_ALIASES.get(cleaned, cleaned)


class SimulatedDevice:
    """Generate repeatable sensor values and track output state for simulations."""

    def __init__(self, profile: SimulationProfile, *, seed_hint: str | None = None):
        if build_info.BUILD_FLAVOR == "prod":
            raise RuntimeError("Simulation is not allowed in production builds")
        self.profile = profile
        self._seed_hint = seed_hint
        seed = self._resolve_seed(profile)
        self.random = random.Random(seed or 1)
        self._started = time.monotonic()
        self._last_spike_at: Dict[str, float] = {}
        self._last_sensor_values: Dict[str, float] = {}
        self._output_state: Dict[str, str] = {}
        self._last_status: str = "online"

    def _resolve_seed(self, profile: SimulationProfile) -> int:
        if profile.seed is not None:
            return int(profile.seed)
        if self._seed_hint:
            return abs(hash(self._seed_hint)) % (2**31)
        return 1

    def update_profile(self, profile: SimulationProfile, *, seed_hint: str | None = None) -> None:
        """Refresh the active simulation profile at runtime."""

        if seed_hint:
            self._seed_hint = seed_hint
        self.profile = profile
        self.random.seed(self._resolve_seed(profile))
        self._last_spike_at.clear()
        self._last_sensor_values.clear()

    def _resolve_time(self, now: Optional[float]) -> float:
        multiplier = self.profile.time_multiplier or 1.0
        if now is None:
            return (time.monotonic() - self._started) * multiplier
        return now * multiplier

    def is_offline(self, now: Optional[float] = None) -> bool:
        now = self._resolve_time(now)
        if not self.profile.enabled:
            return False
        if self.profile.offline:
            return True
        if self.profile.offline_cycle:
            offset = self.profile.offline_cycle.initial_offset_seconds or 0.0
            period = self.profile.offline_cycle.period_seconds
            window = self.profile.offline_cycle.offline_seconds
            if period > 0:
                position = ((now + offset) % period)
                return position <= window
        return False

    def heartbeat_status(self) -> str:
        status = "offline" if self.is_offline() else "online"
        self._last_status = status
        return status

    def read_sensor(self, sensor: SensorConfig, now: Optional[float] = None) -> float | None:
        """Return a deterministic simulated value for a sensor."""

        if not self.profile.enabled:
            return None
        now = self._resolve_time(now)
        if self.is_offline(now):
            return None

        base = self._base_value(sensor, now)
        value = self._apply_variation(sensor.sensor_id, base, now)
        if sensor.interval_seconds == 0:
            # force a small change so COV topics continue to publish
            value += self.random.uniform(-0.05, 0.05)
        value = round(value, 4)
        if sensor.interval_seconds == 0:
            last = self._last_sensor_values.get(sensor.sensor_id)
            if last is not None and value == last:
                value = round(value + 0.01, 4)
            self._last_sensor_values[sensor.sensor_id] = value
        return value

    def apply_command(self, output_id: str, requested_state: str) -> CommandResult:
        stuck = output_id in set(self.profile.stuck_outputs or [])
        prior = self._output_state.get(output_id)
        applied = prior if stuck and prior is not None else requested_state
        self._output_state[output_id] = applied
        return CommandResult(
            output_id=output_id,
            requested_state=requested_state,
            applied_state=applied,
            stuck=stuck,
        )

    def output_state(self, output_id: str) -> Optional[str]:
        return self._output_state.get(output_id)

    def snapshot_outputs(self) -> Dict[str, str]:
        return dict(self._output_state)

    def _base_value(self, sensor: SensorConfig, now: float) -> float:
        """Generate a smooth baseline per sensor type."""

        t = now / 10.0
        type_key = _normalize_sensor_type(sensor.type)
        base_overrides = self.profile.base_overrides or {}
        if sensor.sensor_id in base_overrides:
            return float(base_overrides[sensor.sensor_id])
        if type_key in {"temperature", "temp"}:
            return 18.0 + 6.0 * math.sin(t / 2.5)
        if type_key in {"humidity", "moisture"}:
            return 40.0 + 20.0 * math.sin(t / 3.0 + 0.3)
        if type_key in {"power", "current", "voltage"}:
            return 4.0 + 3.0 * math.sin(t * 1.5)
        if type_key in {"pressure", "water_level"}:
            return 55.0 + 5.0 * math.sin(t / 1.5)
        if type_key in {"wind", "wind_speed"}:
            return max(0.0, 10.0 + 8.0 * math.sin(t * 2.0))
        if type_key in {"flow", "flow_meter"}:
            return max(0.0, 5.0 + 3.0 * math.sin(t * 1.1))
        if type_key in {"lux", "solar", "irradiance"}:
            daylight = max(math.sin((t % (2 * math.pi))), 0.0)
            return 50.0 + 200.0 * daylight
        if type_key == "renogy_bt2":
            return self._renogy_metric_value(sensor.metric, now)
        return 1.0 + math.sin(t)

    def _apply_variation(self, sensor_id: str, value: float, now: float) -> float:
        jitter_map = self.profile.jitter or {}
        spike_map = self.profile.spikes or {}
        if sensor_id in jitter_map:
            sigma = max(float(jitter_map[sensor_id]), 0.0)
            value += self.random.gauss(0, sigma)

        if sensor_id in spike_map:
            cfg = spike_map[sensor_id]
            last = self._last_spike_at.get(sensor_id, 0.0)
            if now - last >= cfg.every_seconds:
                self._last_spike_at[sensor_id] = now
                sign = -1.0 if self.random.random() < 0.5 else 1.0
                jitter = cfg.jitter or 0.0
                value += sign * (cfg.magnitude + self.random.uniform(-jitter, jitter))
        return value

    def _renogy_metric_value(self, metric: Optional[str], now: float) -> float:
        if not metric:
            return 1.0 + math.sin(now / 10.0)
        key = metric.lower()
        spec = RENOGY_METRIC_PROFILE.get(key)
        if not spec:
            return 1.0 + math.sin(now / 10.0)
        base, amplitude, period, clamp_min, clamp_max = spec
        phase = (sum(key.encode("utf-8")) % 10) / 10.0
        value = base + amplitude * math.sin((now / period) + phase)
        if clamp_min is not None:
            value = max(value, clamp_min)
        if clamp_max is not None:
            value = min(value, clamp_max)
        return value
