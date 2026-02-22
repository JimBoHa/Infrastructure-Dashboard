from __future__ import annotations

from typing import Dict

from fastapi import APIRouter, Depends, HTTPException, Request, status

from app.auth import require_node_auth
from app.config import Settings, get_settings
from app.hardware import Ads1263HatAnalogReader, Ads1263HatConfig, NullAnalogReader, PulseInputDriver
from app.http_utils import ensure_simulator, mesh_adapter, persist, simulator
from app.schemas import ConfigEnvelope, NodeUpdatePayload, OutputUpdatePayload, SensorUpdatePayload
from app.services.config_store import apply_config, export_config
from app.services.latency_probe import LatencyProbe
from app.services.publisher import ANALOG_SENSOR_TYPES, PULSE_SENSOR_TYPES, TelemetryPublisher, normalize_sensor_type
from app.services.simulator import SimulatedDevice

router = APIRouter(prefix="/v1", dependencies=[Depends(require_node_auth)])

def _redact_secrets(payload: dict) -> dict:
    node = payload.get("node")
    if isinstance(node, dict):
        node.pop("adoption_token", None)
    wifi_hints = payload.get("wifi_hints")
    if isinstance(wifi_hints, dict):
        wifi_hints.pop("password", None)
    return payload


@router.get("/config")
async def config(settings: Settings = Depends(get_settings)) -> Dict:
    return _redact_secrets(export_config(settings))


def _apply_config_envelope(
    request: Request,
    settings: Settings,
    payload: ConfigEnvelope,
    *,
    status_only: bool,
) -> dict:
    try:
        apply_config(settings, payload.model_dump(exclude_none=True))
    except ValueError as exc:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail=str(exc)) from exc
    try:
        _reload_hardware_drivers(request, settings)
    except Exception:
        pass
    sim: SimulatedDevice | None = (
        ensure_simulator(request.app, settings) if settings.simulation.enabled else simulator(request.app)
    )
    if sim and settings.simulation.enabled:
        sim.update_profile(settings.simulation, seed_hint=settings.node_id)
    mesh = mesh_adapter(request.app)
    if mesh:
        mesh.update_simulation(settings.simulation)
    publisher: TelemetryPublisher | None = getattr(request.app.state, "publisher", None)
    if publisher:
        publisher.simulator = sim
    probe: LatencyProbe | None = getattr(request.app.state, "latency_probe", None)
    if probe:
        probe.configure(
            target_host=settings.mqtt_host,
            target_port=settings.mqtt_port,
            interval_seconds=settings.display.latency_sample_seconds,
            window_samples=settings.display.latency_window_samples,
        )
        if settings.display.enabled:
            probe.start()
        else:
            probe.stop()
    persist(request.app, settings)
    if status_only:
        return {"status": "restored"}
    return _redact_secrets(export_config(settings))


def _reload_hardware_drivers(request: Request, settings: Settings) -> None:
    publisher: TelemetryPublisher | None = getattr(request.app.state, "publisher", None)
    if publisher is None:
        return

    analog_inputs = {
        (
            int(sensor.channel),
            int(sensor.negative_channel) if sensor.negative_channel is not None else None,
        )
        for sensor in settings.sensors
        if normalize_sensor_type(sensor.type) in ANALOG_SENSOR_TYPES
    }
    pulse_channels = {
        int(sensor.channel)
        for sensor in settings.sensors
        if normalize_sensor_type(sensor.type) in PULSE_SENSOR_TYPES
    }

    previous_adc = getattr(request.app.state, "adc_reader", None)
    if previous_adc:
        previous_adc.stop()
        request.app.state.adc_reader = None

    previous_pulse = getattr(request.app.state, "pulse_driver", None)
    if previous_pulse:
        previous_pulse.stop()

    pulse_driver = PulseInputDriver(channels=pulse_channels) if pulse_channels else PulseInputDriver()
    if pulse_channels:
        pulse_driver.start()
    request.app.state.pulse_driver = pulse_driver
    publisher.pulse = pulse_driver

    analog_driver = NullAnalogReader()
    adc_reader = None
    if analog_inputs and not settings.ads1263.enabled:
        analog_driver = NullAnalogReader(required=True, reason="ADS1263 backend disabled")
    if settings.ads1263.enabled and analog_inputs:
        adc_cfg = Ads1263HatConfig(**settings.ads1263.model_dump())
        adc_reader = Ads1263HatAnalogReader(adc_cfg, inputs=analog_inputs)
        adc_reader.start()
        analog_driver = adc_reader
    request.app.state.adc_reader = adc_reader
    publisher.analog = analog_driver


@router.put("/config")
async def overwrite_config(
    payload: ConfigEnvelope,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    return _apply_config_envelope(request, settings, payload, status_only=False)


@router.post("/config/restore")
async def restore_config(
    payload: ConfigEnvelope,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    return _apply_config_envelope(request, settings, payload, status_only=True)


@router.post("/config/import")
async def import_config(
    payload: ConfigEnvelope,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    return _apply_config_envelope(request, settings, payload, status_only=True)


@router.patch("/sensors/{sensor_id}")
async def update_sensor(
    sensor_id: str,
    payload: SensorUpdatePayload,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    sensor = next((sensor for sensor in settings.sensors if sensor.sensor_id == sensor_id), None)
    if not sensor:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Sensor not found")
    updated = sensor.model_copy(update=payload.model_dump(exclude_none=True))
    index = settings.sensors.index(sensor)
    settings.sensors[index] = updated
    persist(request.app, settings)
    return updated.model_dump()


@router.patch("/outputs/{output_id}")
async def update_output(
    output_id: str,
    payload: OutputUpdatePayload,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    output = next((out for out in settings.outputs if out.output_id == output_id), None)
    if not output:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Output not found")
    updated = output.model_copy(update=payload.model_dump(exclude_none=True))
    index = settings.outputs.index(output)
    settings.outputs[index] = updated
    sim = simulator(request.app)
    if sim and settings.simulation.enabled and updated.state:
        sim.apply_command(updated.output_id, str(updated.state))
    persist(request.app, settings)
    return updated.model_dump()


@router.patch("/node")
async def update_node(
    payload: NodeUpdatePayload,
    request: Request,
    settings: Settings = Depends(get_settings),
):
    try:
        apply_config(settings, {"node": payload.model_dump(exclude_none=True)})
    except ValueError as exc:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail=str(exc)) from exc
    persist(request.app, settings)
    return {
        "node_name": settings.node_name,
        "heartbeat_interval_seconds": settings.heartbeat_interval_seconds,
        "telemetry_interval_seconds": settings.telemetry_interval_seconds,
        "capabilities": settings.capabilities,
    }
