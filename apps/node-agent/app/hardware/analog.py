"""Analog input abstractions used by the node agent."""
from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Optional, Protocol


@dataclass
class AnalogHealth:
    ok: bool
    chip_id: Optional[str] = None
    last_error: Optional[str] = None
    last_ok_at: Optional[datetime] = None


class AnalogReader(Protocol):
    """Reads an analog voltage for a given input."""

    def read_voltage(self, channel: int, negative_channel: int | None = None) -> float | None:
        ...


class NullAnalogReader:
    """Fail-closed analog reader used when no backend is active."""

    backend: str = "disabled"

    def __init__(self, *, required: bool = False, reason: str | None = None) -> None:
        self._required = bool(required)
        self._reason = reason
        self._health = AnalogHealth(
            ok=not required,
            last_error=reason if required else None,
        )

    @property
    def health(self) -> AnalogHealth:
        return self._health

    def read_voltage(self, channel: int, negative_channel: int | None = None) -> float | None:  # noqa: ARG002
        return None
