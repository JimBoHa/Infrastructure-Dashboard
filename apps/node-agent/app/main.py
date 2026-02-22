"""FastAPI application exposing node status, configuration, and discovery metadata."""
from __future__ import annotations

import logging
import time
from contextlib import asynccontextmanager
from typing import Dict

from fastapi import FastAPI

from app import build_info
from app.bootstrap import apply_firstboot
from app.config import get_settings
from app.hardware import Ads1263HatAnalogReader, Ads1263HatConfig, MeshAdapter, NullAnalogReader, PulseInputDriver, RenogyBt2Collector
from app.http_utils import apply_provisioning_request
from app.observability import configure_observability
from app.routers import config as config_router
from app.routers import display as display_router
from app.routers import mesh as mesh_router
from app.routers import provisioning as provisioning_router
from app.routers import renogy as renogy_router
from app.routers import root as root_router
from app.routers import simulation as simulation_router
from app.routers import status as status_router
from app.schemas import BluetoothProvisionRequest
from app.services.ble_provisioning import BLEProvisioningManager, ProvisioningEvent
from app.services.config_store import ConfigStore, apply_config
from app.services.discovery import DiscoveryAdvertiser
from app.services.latency_probe import LatencyProbe
from app.services.output_listener import OutputCommandListener
from app.services.provisioning import ProvisioningStore
from app.services.publisher import ANALOG_SENSOR_TYPES, PULSE_SENSOR_TYPES, TelemetryPublisher, normalize_sensor_type
from app.services.simulator import SimulatedDevice
from app.utils.systemd import SystemdNotifier

logger = logging.getLogger(__name__)

systemd_notifier = SystemdNotifier()


@asynccontextmanager
async def lifespan(app: FastAPI):
    settings = get_settings()
    settings.started_at_monotonic = time.monotonic()
    apply_firstboot(settings)
    store = ConfigStore(settings.config_file)
    persisted = store.load()
    if persisted:
        apply_config(settings, persisted)
    systemd_notifier.status("Loading hardware drivers and services")
    simulator = None
    if settings.simulation.enabled:
        if build_info.BUILD_FLAVOR == "prod":
            raise RuntimeError("Simulation is not allowed in production builds")
        simulator = SimulatedDevice(settings.simulation, seed_hint=settings.node_id)
    if simulator and settings.outputs:
        for output in settings.outputs:
            initial_state = output.state or output.default_state or "off"
            simulator.apply_command(output.output_id, str(initial_state))
    mesh_adapter = MeshAdapter(settings)
    renogy_collector = RenogyBt2Collector(settings)
    adc_reader = None
    analog_driver = NullAnalogReader()
    pulse_driver = PulseInputDriver()
    try:
        analog_inputs = {
            (
                int(sensor.channel),
                int(sensor.negative_channel) if sensor.negative_channel is not None else None,
            )
            for sensor in settings.sensors
            if normalize_sensor_type(sensor.type) in ANALOG_SENSOR_TYPES
        }
    except Exception:
        analog_inputs = set()
    try:
        pulse_channels = {
            int(sensor.channel)
            for sensor in settings.sensors
            if normalize_sensor_type(sensor.type) in PULSE_SENSOR_TYPES
        }
    except Exception:
        pulse_channels = set()
    if pulse_channels:
        pulse_driver = PulseInputDriver(channels=pulse_channels)
        pulse_driver.start()
    if analog_inputs and not settings.ads1263.enabled:
        analog_driver = NullAnalogReader(required=True, reason="ADS1263 backend disabled")
    if settings.ads1263.enabled and analog_inputs:
        adc_cfg = Ads1263HatConfig(**settings.ads1263.model_dump())
        adc_reader = Ads1263HatAnalogReader(adc_cfg, inputs=analog_inputs)
        adc_reader.start()
        analog_driver = adc_reader
    latency_probe = LatencyProbe(
        probe_kind="icmp",
        target_host=settings.mqtt_host,
        target_port=0,
        interval_seconds=settings.display.latency_sample_seconds,
        window_samples=max(settings.display.latency_window_samples, 360),
    )
    latency_probe.start()
    publisher = TelemetryPublisher(
        settings,
        analog_driver,
        pulse_driver,
        mesh_adapter=mesh_adapter,
        renogy_collector=renogy_collector,
        latency_probe=latency_probe,
        simulator=simulator,
    )
    provision_store = ProvisioningStore(
        settings.provision_queue_file,
        secret=settings.provisioning_secret.get_secret_value()
        if settings.provisioning_secret
        else None,
    )
    publisher.start()
    command_listener = None
    if settings.outputs and settings.simulation.enabled:
        command_listener = OutputCommandListener(settings, simulator=simulator)
        command_listener.start()
    if mesh_adapter.enabled:
        await mesh_adapter.start()
    renogy_collector.start()
    advertiser = DiscoveryAdvertiser(settings)
    await advertiser.start()

    async def _handle_ble_payload(payload: Dict[str, object]) -> ProvisioningEvent:
        try:
            request = BluetoothProvisionRequest.model_validate(payload)
        except Exception as exc:
            return ProvisioningEvent.now("error", f"invalid_payload: {exc}")
        record = None
        if request.session_id:
            record = provision_store.get(request.session_id)
            if record:
                if not request.wifi_ssid:
                    request.wifi_ssid = record.wifi_ssid
                if not request.wifi_password:
                    request.wifi_password = record.wifi_password
                if not request.adoption_token:
                    request.adoption_token = record.adoption_token
                provision_store.update_status(
                    record.session_id,
                    "received",
                    "ble_payload",
                    updates={
                        "wifi_ssid": request.wifi_ssid or record.wifi_ssid,
                        "wifi_password": request.wifi_password or record.wifi_password,
                        "adoption_token": request.adoption_token or record.adoption_token,
                    },
                )
        if not record:
            record = provision_store.append(
                device_name=request.device_name,
                pin=request.pin,
                wifi_ssid=request.wifi_ssid,
                wifi_password=request.wifi_password,
                node_name=settings.node_name,
                adoption_token=request.adoption_token or settings.adoption_token,
                mesh_join_code=request.mesh_join_code,
                preferred_protocol=request.preferred_protocol,
                status="received",
                message="ble_payload",
            )
        return await apply_provisioning_request(
            app,
            request,
            settings,
            session_id=record.session_id,
        )

    ble_manager = BLEProvisioningManager(settings, _handle_ble_payload)
    await ble_manager.start()

    app.state.publisher = publisher
    app.state.advertiser = advertiser
    app.state.config_store = store
    app.state.provision_store = provision_store
    app.state.ble_provisioning = ble_manager
    app.state.mesh = mesh_adapter
    app.state.renogy = renogy_collector
    app.state.simulator = simulator
    app.state.command_listener = command_listener
    app.state.adc_reader = adc_reader
    app.state.pulse_driver = pulse_driver
    app.state.latency_probe = latency_probe
    app.state.started_at = time.monotonic()
    logger.info("Node agent started for %s", settings.node_id)
    systemd_notifier.ready(f"Node agent ready ({settings.node_id})")
    systemd_notifier.start_watchdog()

    try:
        yield
    finally:
        systemd_notifier.stop_watchdog()
        publisher: TelemetryPublisher | None = getattr(app.state, "publisher", None)
        if publisher:
            await publisher.stop()
        advertiser: DiscoveryAdvertiser | None = getattr(app.state, "advertiser", None)
        if advertiser:
            await advertiser.stop()
        mesh: MeshAdapter | None = getattr(app.state, "mesh", None)
        if mesh:
            await mesh.stop()
        renogy: RenogyBt2Collector | None = getattr(app.state, "renogy", None)
        if renogy:
            await renogy.stop()
        cmd_listener: OutputCommandListener | None = getattr(
            app.state, "command_listener", None
        )
        if cmd_listener:
            await cmd_listener.stop()
        ble_manager: BLEProvisioningManager | None = getattr(
            app.state, "ble_provisioning", None
        )
        if ble_manager:
            await ble_manager.stop()
        adc_reader = getattr(app.state, "adc_reader", None)
        if adc_reader:
            adc_reader.stop()
        pulse_driver = getattr(app.state, "pulse_driver", None)
        if pulse_driver:
            pulse_driver.stop()
        probe: LatencyProbe | None = getattr(app.state, "latency_probe", None)
        if probe:
            probe.stop()
        systemd_notifier.stopping("Node agent shutting down")


settings = get_settings()
app = FastAPI(title="Node Agent", lifespan=lifespan)
configure_observability(
    app,
    service_name=settings.otel_service_name,
    service_version=settings.service_version,
    log_level=settings.log_level,
    otel_enabled=settings.otel_enabled,
    otlp_endpoint=settings.otel_exporter_otlp_endpoint,
    otlp_headers=settings.otel_exporter_otlp_headers,
    otel_sample_ratio=settings.otel_sample_ratio,
)

app.include_router(root_router.router)
app.include_router(status_router.router)
app.include_router(config_router.router)
app.include_router(display_router.router)
if build_info.BUILD_FLAVOR != "prod":
    app.include_router(simulation_router.router)
app.include_router(mesh_router.router)
app.include_router(renogy_router.router)
app.include_router(provisioning_router.router)


if __name__ == "__main__":  # pragma: no cover
    import uvicorn

    uvicorn.run("app.main:app", host="0.0.0.0", port=9000, reload=True)
