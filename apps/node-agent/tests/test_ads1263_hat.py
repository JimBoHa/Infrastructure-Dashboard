from __future__ import annotations

import sys
import types


def _install_fake_ads1263_modules(monkeypatch, *, drdy_pressed: bool = True, chip_id_reg: int = 0x20) -> None:
    class FakeSpiDev:
        def __init__(self) -> None:
            self.max_speed_hz = 0
            self.mode = 0
            self._opened = False
            self._last_reg: int | None = None

        def open(self, _bus: int, _device: int) -> None:
            fail_buses = getattr(sys.modules.get("spidev"), "_fail_buses", set())
            if int(_bus) in set(fail_buses):
                raise FileNotFoundError(f"/dev/spidev{_bus}.{_device} missing")
            self._opened = True

        def close(self) -> None:
            self._opened = False

        def writebytes(self, data) -> None:  # type: ignore[no-untyped-def]
            # Capture last RREG register address.
            if len(data) >= 2 and (int(data[0]) & 0xE0) == 0x20:
                self._last_reg = int(data[0]) & 0x1F

        def readbytes(self, _length: int):  # type: ignore[no-untyped-def]
            if self._last_reg == 0x00:
                return [int(chip_id_reg) & 0xFF]
            return [0x00]

        def xfer2(self, data):  # type: ignore[no-untyped-def]
            if data and int(data[0]) == 0x12:
                # STATUS + 4 data bytes + CRC. Raw=0x40000000 (~0.5 * vref)
                return [0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00]
            return [0x00] * len(data)

    spidev_mod = types.ModuleType("spidev")
    spidev_mod.SpiDev = FakeSpiDev  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "spidev", spidev_mod)

    class FakeLGPIOFactory:
        pass

    class FakeDevice:
        pin_factory = object()

    class FakeOutputDevice:
        def __init__(self, _pin: int, *, active_high: bool = True, initial_value: bool = False):  # noqa: ARG002
            self._value = bool(initial_value)

        def on(self) -> None:
            self._value = True

        def off(self) -> None:
            self._value = False

        def close(self) -> None:
            return

    class FakeButton:
        def __init__(self, _pin: int, *, pull_up: bool = True, bounce_time: float = 0.0):  # noqa: ARG002
            self.is_pressed = bool(drdy_pressed)

        def close(self) -> None:
            return

    gpiozero_mod = types.ModuleType("gpiozero")
    gpiozero_mod.Device = FakeDevice  # type: ignore[attr-defined]
    gpiozero_mod.Button = FakeButton  # type: ignore[attr-defined]
    gpiozero_mod.OutputDevice = FakeOutputDevice  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "gpiozero", gpiozero_mod)
    monkeypatch.setitem(sys.modules, "gpiozero.pins", types.ModuleType("gpiozero.pins"))
    lgpio_mod = types.ModuleType("gpiozero.pins.lgpio")
    lgpio_mod.LGPIOFactory = FakeLGPIOFactory  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "gpiozero.pins.lgpio", lgpio_mod)


def _install_fake_ads1263_modules_with_pin_tracking(
    monkeypatch,
    *,
    pin_registry: set[int],
    spi_open_fails: bool = False,
    drdy_pressed: bool = True,
    chip_id_reg: int = 0x20,
) -> types.ModuleType:
    class FakeSpiDev:
        def __init__(self) -> None:
            self.max_speed_hz = 0
            self.mode = 0
            self._opened = False
            self._last_reg: int | None = None

        def open(self, _bus: int, _device: int) -> None:
            if getattr(sys.modules.get("spidev"), "_open_fails", False):
                raise FileNotFoundError("/dev/spidev0.0 missing")
            fail_buses = getattr(sys.modules.get("spidev"), "_fail_buses", set())
            if int(_bus) in set(fail_buses):
                raise FileNotFoundError(f"/dev/spidev{_bus}.{_device} missing")
            self._opened = True

        def close(self) -> None:
            self._opened = False

        def writebytes(self, data) -> None:  # type: ignore[no-untyped-def]
            # Capture last RREG register address.
            if len(data) >= 2 and (int(data[0]) & 0xE0) == 0x20:
                self._last_reg = int(data[0]) & 0x1F

        def readbytes(self, _length: int):  # type: ignore[no-untyped-def]
            if self._last_reg == 0x00:
                return [int(chip_id_reg) & 0xFF]
            return [0x00]

        def xfer2(self, data):  # type: ignore[no-untyped-def]
            if data and int(data[0]) == 0x12:
                # STATUS + 4 data bytes + CRC. Raw=0x40000000 (~0.5 * vref)
                return [0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00]
            return [0x00] * len(data)

    spidev_mod = types.ModuleType("spidev")
    spidev_mod.SpiDev = FakeSpiDev  # type: ignore[attr-defined]
    spidev_mod._open_fails = bool(spi_open_fails)  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "spidev", spidev_mod)

    class FakeLGPIOFactory:
        pass

    class FakeDevice:
        pin_factory = object()

    class FakeOutputDevice:
        def __init__(self, pin: int, *, active_high: bool = True, initial_value: bool = False):  # noqa: ARG002
            if int(pin) in pin_registry:
                raise RuntimeError(f"pin GPIO{int(pin)} is already in use")
            self._pin = int(pin)
            pin_registry.add(self._pin)
            self._value = bool(initial_value)

        def on(self) -> None:
            self._value = True

        def off(self) -> None:
            self._value = False

        def close(self) -> None:
            pin_registry.discard(self._pin)

    class FakeButton:
        def __init__(self, _pin: int, *, pull_up: bool = True, bounce_time: float = 0.0):  # noqa: ARG002
            self.is_pressed = bool(drdy_pressed)

        def close(self) -> None:
            return

    gpiozero_mod = types.ModuleType("gpiozero")
    gpiozero_mod.Device = FakeDevice  # type: ignore[attr-defined]
    gpiozero_mod.Button = FakeButton  # type: ignore[attr-defined]
    gpiozero_mod.OutputDevice = FakeOutputDevice  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "gpiozero", gpiozero_mod)
    monkeypatch.setitem(sys.modules, "gpiozero.pins", types.ModuleType("gpiozero.pins"))
    lgpio_mod = types.ModuleType("gpiozero.pins.lgpio")
    lgpio_mod.LGPIOFactory = FakeLGPIOFactory  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "gpiozero.pins.lgpio", lgpio_mod)

    return spidev_mod


def test_ads1263_hat_happy_path(monkeypatch) -> None:
    _install_fake_ads1263_modules(monkeypatch, drdy_pressed=True, chip_id_reg=0x20)

    from app.hardware.ads1263_hat import Ads1263HatAnalogReader, Ads1263HatConfig

    reader = Ads1263HatAnalogReader(Ads1263HatConfig(enabled=True, scan_interval_seconds=10.0), inputs=[(0, None)])
    reader.start()
    try:
        assert reader.health.ok is True
        assert reader.health.chip_id == "0x01"
        assert reader.backend == "ads1263"
    finally:
        reader.stop()


def test_ads1263_hat_drdy_timeout_sets_health(monkeypatch) -> None:
    _install_fake_ads1263_modules(monkeypatch, drdy_pressed=False, chip_id_reg=0x20)

    import app.hardware.ads1263_hat as ads1263_hat
    from app.hardware.ads1263_hat import Ads1263HatAnalogReader, Ads1263HatConfig

    tick = {"t": 0.0}

    def fake_monotonic() -> float:
        tick["t"] += 1.0
        return float(tick["t"])

    monkeypatch.setattr(ads1263_hat.time, "monotonic", fake_monotonic)
    monkeypatch.setattr(ads1263_hat.time, "sleep", lambda _seconds: None)

    reader = Ads1263HatAnalogReader(Ads1263HatConfig(enabled=True, scan_interval_seconds=10.0), inputs=[(0, None)])
    reader.start()

    assert reader.health.ok is False
    assert reader.health.chip_id == "0x01"
    assert "DRDY timeout" in (reader.health.last_error or "")


def test_ads1263_hat_chip_id_mismatch_sets_health(monkeypatch) -> None:
    _install_fake_ads1263_modules(monkeypatch, drdy_pressed=True, chip_id_reg=0xE0)  # chip id => 0x07

    from app.hardware.ads1263_hat import Ads1263HatAnalogReader, Ads1263HatConfig

    reader = Ads1263HatAnalogReader(Ads1263HatConfig(enabled=True, scan_interval_seconds=10.0), inputs=[(0, None)])
    reader.start()
    assert reader.health.ok is False
    assert "chip id mismatch" in (reader.health.last_error or "")


def test_ads1263_hat_failed_init_releases_gpio_pins(monkeypatch) -> None:
    pin_registry: set[int] = set()
    spidev_mod = _install_fake_ads1263_modules_with_pin_tracking(
        monkeypatch,
        pin_registry=pin_registry,
        spi_open_fails=True,
        drdy_pressed=True,
        chip_id_reg=0x20,
    )

    from app.hardware.ads1263_hat import Ads1263HatAnalogReader, Ads1263HatConfig

    reader = Ads1263HatAnalogReader(Ads1263HatConfig(enabled=True, scan_interval_seconds=10.0), inputs=[(0, None)])
    reader.start()
    assert reader.health.ok is False

    # Failed init should not leak gpiozero pin reservations.
    assert 22 not in pin_registry

    # Simulate the operator fixing SPI (/dev/spidev0.0 becomes available) and
    # re-applying config without restarting the node-agent process.
    spidev_mod._open_fails = False  # type: ignore[attr-defined]
    reader2 = Ads1263HatAnalogReader(Ads1263HatConfig(enabled=True, scan_interval_seconds=10.0), inputs=[(0, None)])
    reader2.start()
    try:
        assert reader2.health.ok is True
    finally:
        reader2.stop()


def test_ads1263_hat_autodetects_spi_bus_on_linux(monkeypatch) -> None:
    pin_registry: set[int] = set()
    spidev_mod = _install_fake_ads1263_modules_with_pin_tracking(
        monkeypatch,
        pin_registry=pin_registry,
        spi_open_fails=False,
        drdy_pressed=True,
        chip_id_reg=0x20,
    )

    import app.hardware.ads1263_hat as ads1263_hat
    from app.hardware.ads1263_hat import Ads1263HatAnalogReader, Ads1263HatConfig

    monkeypatch.setattr(ads1263_hat.sys, "platform", "linux")
    # Simulate a Debian/Pi5 stack exposing SPI as spi10.
    spidev_mod._fail_buses = {0}  # type: ignore[attr-defined]

    monkeypatch.setattr(
        ads1263_hat,
        "_list_spidev_devices",
        lambda: [(10, 0, ads1263_hat.Path("/dev/spidev10.0"))],
    )

    reader = Ads1263HatAnalogReader(Ads1263HatConfig(enabled=True, scan_interval_seconds=10.0), inputs=[(0, None)])
    reader.start()
    try:
        assert reader.health.ok is True
        assert reader.health.chip_id == "0x01"
    finally:
        reader.stop()
