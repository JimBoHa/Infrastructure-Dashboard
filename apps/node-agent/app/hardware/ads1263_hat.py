"""Waveshare High-Precision AD HAT (ADS1263) support.

Pi 5 notes:
- Uses `spidev` for SPI and `gpiozero` (lgpio backend) for GPIO (no `RPi.GPIO`).
- Fail-closed: when hardware is unavailable/unhealthy, reads return `None` and
  telemetry is not published (so the sensor is clearly offline).

Hardware reference: https://www.waveshare.com/wiki/High-Precision_AD_HAT
"""

from __future__ import annotations

import logging
import math
import re
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional

from app.hardware.analog import AnalogHealth, AnalogReader
from app.hardware.background_sampler import BackgroundAnalogSampler

logger = logging.getLogger(__name__)


class Ads1263Error(RuntimeError):
    pass


_SPIDEV_RE = re.compile(r"^spidev(?P<bus>\d+)\.(?P<device>\d+)$")


def _list_spidev_devices() -> list[tuple[int, int, Path]]:
    """Return available spidev nodes as (bus, device, path).

    Notes:
    - On some Pi 5 / Debian stacks SPI0 is exposed as `/dev/spidev10.0` (spi10),
      not `/dev/spidev0.0`.
    """

    if not sys.platform.startswith("linux"):
        return []
    try:
        nodes: list[tuple[int, int, Path]] = []
        for path in Path("/dev").glob("spidev*.*"):
            match = _SPIDEV_RE.match(path.name)
            if not match:
                continue
            nodes.append((int(match.group("bus")), int(match.group("device")), path))
        nodes.sort(key=lambda item: (item[0], item[1]))
        return nodes
    except Exception:  # pragma: no cover - defensive against unusual /dev states
        return []


ADS1263_REG_ID = 0x00
ADS1263_REG_POWER = 0x01
ADS1263_REG_INTERFACE = 0x02
ADS1263_REG_MODE0 = 0x03
ADS1263_REG_MODE1 = 0x04
ADS1263_REG_MODE2 = 0x05
ADS1263_REG_INPMUX = 0x06
ADS1263_REG_REFMUX = 0x0F

ADS1263_CMD_RESET = 0x06
ADS1263_CMD_START1 = 0x08
ADS1263_CMD_STOP1 = 0x0A
ADS1263_CMD_RDATA1 = 0x12
ADS1263_CMD_RREG = 0x20
ADS1263_CMD_WREG = 0x40

ADS1263_DRATE = {
    "ADS1263_100SPS": 0x07,
    "ADS1263_60SPS": 0x06,
    "ADS1263_50SPS": 0x05,
    "ADS1263_20SPS": 0x04,
    "ADS1263_10SPS": 0x02,
}


@dataclass(frozen=True)
class Ads1263HatConfig:
    """Defaults match the Waveshare High-Precision AD HAT pinout (BCM)."""

    enabled: bool = False
    spi_bus: int = 0
    spi_device: int = 0
    spi_mode: int = 0b01
    spi_speed_hz: int = 2_000_000
    rst_bcm: int = 18
    cs_bcm: int = 22
    drdy_bcm: int = 17
    vref_volts: float = 5.0
    gain: int = 1
    data_rate: str = "ADS1263_100SPS"
    scan_interval_seconds: float = 0.25


class _Ads1263HatDevice:
    def __init__(self, cfg: Ads1263HatConfig):
        self._cfg = cfg
        try:
            import spidev  # type: ignore
        except Exception as exc:  # pragma: no cover
            raise Ads1263Error("spidev is required for ADS1263 hardware mode") from exc

        try:
            from gpiozero import Button, Device, OutputDevice  # type: ignore
        except Exception as exc:  # pragma: no cover
            raise Ads1263Error("gpiozero is required for ADS1263 hardware mode") from exc

        if sys.platform.startswith("linux"):
            try:
                from gpiozero.pins.lgpio import LGPIOFactory  # type: ignore
            except Exception as exc:  # pragma: no cover - depends on runtime deps
                raise Ads1263Error(
                    "gpiozero LGPIOFactory is required for Pi 5 (install `lgpio` / `python3-lgpio` and a gpiozero build with LGPIO support)"
                ) from exc

            try:
                if not isinstance(Device.pin_factory, LGPIOFactory):
                    Device.pin_factory = LGPIOFactory()
            except Exception as exc:  # pragma: no cover - defensive
                raise Ads1263Error(f"Unable to configure gpiozero LGPIOFactory: {exc}") from exc

        self._spi = spidev.SpiDev()
        self._cs = OutputDevice(cfg.cs_bcm, active_high=True, initial_value=True)
        self._rst = OutputDevice(cfg.rst_bcm, active_high=True, initial_value=True)
        self._drdy = Button(cfg.drdy_bcm, pull_up=True, bounce_time=0)
        self._initialized = False

    def init(self) -> int:
        if self._initialized:
            chip_id = self.read_chip_id()
            return int(chip_id)

        cfg = self._cfg
        requested_bus = int(cfg.spi_bus)
        requested_device = int(cfg.spi_device)
        requested_path = (
            Path(f"/dev/spidev{requested_bus}.{requested_device}") if sys.platform.startswith("linux") else None
        )
        spidev_path = requested_path
        bus = requested_bus
        device = requested_device
        try:
            self._spi.open(bus, device)
        except FileNotFoundError as exc:  # pragma: no cover - depends on kernel/device state
            # Attempt a conservative auto-detection fallback.
            candidates = _list_spidev_devices()
            chosen: tuple[int, int, Path] | None = None
            if candidates:
                same_device = [cand for cand in candidates if int(cand[1]) == device]
                if len(same_device) == 1:
                    chosen = same_device[0]
                elif len(candidates) == 1:
                    chosen = candidates[0]

            if chosen is not None:
                detected_bus, detected_device, detected_path = chosen
                if (detected_bus, detected_device) != (bus, device):
                    try:
                        self._spi.open(int(detected_bus), int(detected_device))
                        bus = int(detected_bus)
                        device = int(detected_device)
                        spidev_path = detected_path
                        logger.info(
                            "ADS1263 SPI device %s missing; using detected %s",
                            requested_path or f"spidev{requested_bus}.{requested_device}",
                            detected_path,
                        )
                    except FileNotFoundError:
                        pass

            if spidev_path != requested_path or (bus, device) != (requested_bus, requested_device):
                # Successful fallback open.
                pass
            else:
                available = ", ".join(str(path) for _b, _d, path in candidates) if candidates else "(none)"
                dev_label = (
                    str(requested_path)
                    if requested_path is not None
                    else f"spi_bus={cfg.spi_bus} spi_device={cfg.spi_device}"
                )
                raise Ads1263Error(
                    "Unable to open ADS1263 SPI device "
                    f"({dev_label}). Available SPI devices: {available}. "
                    "SPI may be disabled; enable SPI (Pi OS: `raspi-config` → Interface Options → SPI, "
                    "or set `dtparam=spi=on` in /boot/firmware/config.txt). "
                    "If your system exposes SPI as /dev/spidev10.0, set ads1263.spi_bus=10."
                ) from exc
        except PermissionError as exc:  # pragma: no cover - depends on service user perms
            dev_label = str(spidev_path) if spidev_path is not None else f"spi_bus={bus} spi_device={device}"
            raise Ads1263Error(
                f"Permission denied opening ADS1263 SPI device ({dev_label}). Ensure the node-agent service user can access spidev (e.g., in the `spi` group)."
            ) from exc
        except Exception as exc:  # pragma: no cover - depends on kernel/device state
            dev_label = str(spidev_path) if spidev_path is not None else f"spi_bus={bus} spi_device={device}"
            raise Ads1263Error(
                f"Unable to open ADS1263 SPI device ({dev_label}): {exc}"
            ) from exc
        self._spi.max_speed_hz = int(cfg.spi_speed_hz)
        self._spi.mode = int(cfg.spi_mode)

        self.reset()
        chip_id = self.read_chip_id()
        if chip_id != 0x01:
            raise Ads1263Error(f"ADS1263 chip id mismatch: expected 0x01, got 0x{chip_id:02x}")

        # Ensure STATUS byte is enabled so reads have stable framing.
        power = self._read_reg(ADS1263_REG_POWER)
        self._write_reg(ADS1263_REG_POWER, int(power) | 0x10)  # external crystal
        interface = self._read_reg(ADS1263_REG_INTERFACE)
        self._write_reg(ADS1263_REG_INTERFACE, int(interface) | 0x04)  # status byte enable

        self._write_cmd(ADS1263_CMD_STOP1)
        self.configure_adc1(gain=cfg.gain, data_rate=cfg.data_rate)
        self._write_cmd(ADS1263_CMD_START1)

        self._initialized = True
        return int(chip_id)

    def close(self) -> None:
        # Always close GPIO devices, even if `init()` never completed.
        #
        # If SPI is disabled/misconfigured, `init()` can raise after GPIO devices
        # were constructed. In that case, failing to close here leaves gpiozero
        # pin reservations behind, causing subsequent config reloads to fail
        # with "pin GPIOXX is already in use".
        try:
            try:
                self._spi.close()
            except Exception:
                pass
        finally:
            cs = getattr(self, "_cs", None)
            if cs is not None:
                try:
                    cs.on()
                except Exception:
                    pass
            for dev in (getattr(self, "_rst", None), cs, getattr(self, "_drdy", None)):
                if dev is None:
                    continue
                try:
                    dev.close()
                except Exception:
                    pass
        self._initialized = False

    def reset(self) -> None:
        # nRESET is active-low.
        self._rst.off()
        time.sleep(0.01)
        self._rst.on()
        time.sleep(0.01)
        self._write_cmd(ADS1263_CMD_RESET)
        time.sleep(0.005)

    def read_chip_id(self) -> int:
        return (self._read_reg(ADS1263_REG_ID) >> 5) & 0x07

    def configure_adc1(self, *, gain: int, data_rate: str) -> None:
        drate = ADS1263_DRATE.get(data_rate, ADS1263_DRATE["ADS1263_100SPS"])

        gain_code = max(min(int(gain), 64), 1)
        gain_shift = 0
        while (1 << gain_shift) < gain_code and gain_shift < 6:
            gain_shift += 1

        # MODE2: PGA bypassed (stable for 0-5V single-ended), with gain/drate fields.
        mode2 = 0x80 | (gain_shift << 4) | drate
        self._write_reg(ADS1263_REG_MODE2, mode2)

        # REFMUX: use VDD/VSS reference (5V rail) per Waveshare demo defaults.
        self._write_reg(ADS1263_REG_REFMUX, 0x24)

        # MODE0: conversion delay (35us).
        self._write_reg(ADS1263_REG_MODE0, 0x03)

        # MODE1: FIR filter (demo default).
        self._write_reg(ADS1263_REG_MODE1, 0x84)

    def read_channel_raw(self, channel: int, negative_channel: int | None = None) -> int:
        channel = int(channel)
        if channel < 0 or channel > 9:
            raise Ads1263Error(f"ADS1263 channel must be 0-9, got {channel}")
        if negative_channel is None:
            self._set_channel_single_ended(channel)
        else:
            neg = int(negative_channel)
            if neg < 0 or neg > 10:
                raise Ads1263Error(f"ADS1263 negative channel must be 0-10 (10=COM), got {neg}")
            self._set_channel_differential(channel, neg)

        self._wait_drdy(
            timeout_seconds=1.0,
            context=f"channel={channel} negative={negative_channel} drdy_bcm={self._cfg.drdy_bcm}",
        )
        return self._read_adc1_raw()

    def _wait_drdy(self, *, timeout_seconds: float, context: str | None = None) -> None:
        deadline = time.monotonic() + max(float(timeout_seconds), 0.01)
        while time.monotonic() < deadline:
            if self._drdy.is_pressed:
                return
            time.sleep(0.0002)
        suffix = f" ({context})" if context else ""
        raise Ads1263Error(f"ADS1263 DRDY timeout{suffix}")

    def _select(self) -> None:
        self._cs.off()

    def _deselect(self) -> None:
        self._cs.on()

    def _write_cmd(self, cmd: int) -> None:
        self._select()
        try:
            self._spi.writebytes([cmd & 0xFF])
        finally:
            self._deselect()

    def _write_reg(self, reg: int, value: int) -> None:
        self._select()
        try:
            self._spi.writebytes([ADS1263_CMD_WREG | (reg & 0x1F), 0x00, value & 0xFF])
        finally:
            self._deselect()

    def _read_reg(self, reg: int) -> int:
        self._select()
        try:
            self._spi.writebytes([ADS1263_CMD_RREG | (reg & 0x1F), 0x00])
            data = self._spi.readbytes(1)
            return int(data[0]) if data else 0
        finally:
            self._deselect()

    def _set_channel_single_ended(self, channel: int) -> None:
        # INPMUX: AINP = channel, AINN = AINCOM (0x0A)
        inpmux = ((channel & 0x0F) << 4) | 0x0A
        self._write_reg(ADS1263_REG_INPMUX, inpmux)

    def _set_channel_differential(self, positive: int, negative: int) -> None:
        # INPMUX: AINP = positive, AINN = negative (0x0A == AINCOM)
        inpmux = ((positive & 0x0F) << 4) | (negative & 0x0F)
        self._write_reg(ADS1263_REG_INPMUX, inpmux)

    def _read_adc1_raw(self) -> int:
        self._select()
        try:
            frame = [ADS1263_CMD_RDATA1] + [0xFF] * 6  # STATUS + 4 data + CRC
            rx = self._spi.xfer2(frame)
        finally:
            self._deselect()

        if not rx or len(rx) < 7:
            raise Ads1263Error("ADS1263 short read")

        raw_u32 = (rx[2] << 24) | (rx[3] << 16) | (rx[4] << 8) | rx[5]
        if raw_u32 & 0x80000000:
            return int(raw_u32 - (1 << 32))
        return int(raw_u32)


class Ads1263HatAnalogReader(AnalogReader):
    """Non-blocking analog reader backed by a background ADS1263 scan loop."""

    backend: str = "ads1263"

    def __init__(self, cfg: Ads1263HatConfig, *, inputs: Iterable[tuple[int, int | None]]):
        self._cfg = cfg
        self._inputs = sorted({(int(pos), int(neg) if neg is not None else None) for pos, neg in inputs})
        self._device: _Ads1263HatDevice | None = None
        self._sampler: BackgroundAnalogSampler | None = None
        self._health = AnalogHealth(ok=False, last_error="not initialized")

        if not self._inputs:
            self._health = AnalogHealth(ok=True)
            return

        if not cfg.enabled:
            self._health = AnalogHealth(ok=False, last_error="ADS1263 backend disabled")
            return

        try:
            self._device = _Ads1263HatDevice(cfg)
        except Exception as exc:
            self._health = AnalogHealth(ok=False, last_error=str(exc))
            self._device = None

    @property
    def health(self) -> AnalogHealth:
        return self._health

    def start(self) -> None:
        if self._sampler is not None:
            return
        if self._device is None:
            return

        chip: str | None = self._health.chip_id
        try:
            chip_id = self._device.init()
            chip = f"0x{chip_id:02x}"
            if float(self._cfg.vref_volts) <= 0:
                raise Ads1263Error(f"ADS1263 vref_volts must be > 0 (got {self._cfg.vref_volts})")

            pos, neg = self._inputs[0]
            raw = self._device.read_channel_raw(pos, neg)
            volts = (raw / float(0x7FFFFFFF)) * float(self._cfg.vref_volts)
            if not math.isfinite(volts):
                raise Ads1263Error("ADS1263 sample conversion produced non-finite voltage")
            if abs(float(volts)) > float(self._cfg.vref_volts) * 1.2:
                raise Ads1263Error(
                    f"ADS1263 sample out of range during init (got {volts:.4f}V, vref={self._cfg.vref_volts}V)"
                )
            self._health = AnalogHealth(ok=True, chip_id=chip, last_ok_at=datetime.now(timezone.utc))
        except Exception as exc:
            self._health = AnalogHealth(ok=False, chip_id=chip, last_error=str(exc))
            try:
                self._device.close()
            except Exception:
                pass
            return

        self._sampler = BackgroundAnalogSampler(
            read_fn=self._read_channel_voltage,
            inputs=self._inputs,
            interval_seconds=float(self._cfg.scan_interval_seconds),
        )
        self._sampler.start()

    def stop(self) -> None:
        if self._sampler is not None:
            self._sampler.stop()
            self._sampler = None
        if self._device:
            self._device.close()

    def read_voltage(self, channel: int, negative_channel: int | None = None) -> float | None:
        if self._sampler is None:
            return None
        return self._sampler.read_voltage(channel, negative_channel)

    def _read_channel_voltage(self, channel: int, negative_channel: Optional[int]) -> float | None:
        if self._device is None:
            return None
        try:
            raw = self._device.read_channel_raw(channel, negative_channel)
            value = (raw / float(0x7FFFFFFF)) * float(self._cfg.vref_volts)
            if not math.isfinite(float(value)):
                raise Ads1263Error("ADS1263 sample conversion produced non-finite voltage")
            self._health = AnalogHealth(ok=True, chip_id=self._health.chip_id, last_ok_at=datetime.now(timezone.utc))
            return float(value)
        except Exception as exc:
            self._health = AnalogHealth(ok=False, chip_id=self._health.chip_id, last_error=str(exc))
            raise
