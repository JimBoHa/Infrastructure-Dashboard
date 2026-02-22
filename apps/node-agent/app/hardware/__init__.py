"""Hardware abstraction layer for the node agent."""
from __future__ import annotations

from .ads1263_hat import Ads1263HatAnalogReader, Ads1263HatConfig
from .analog import AnalogHealth, AnalogReader, NullAnalogReader
from .background_sampler import BackgroundAnalogSampler
from .gpio import PulseInputDriver
from .mesh import MeshAdapter, MeshDiagnostics, MeshSample
from .renogy_bt2 import RenogyBt2Collector, RenogyBt2Snapshot

__all__ = [
    "AnalogHealth",
    "AnalogReader",
    "NullAnalogReader",
    "Ads1263HatAnalogReader",
    "Ads1263HatConfig",
    "BackgroundAnalogSampler",
    "PulseInputDriver",
    "MeshAdapter",
    "MeshDiagnostics",
    "MeshSample",
    "RenogyBt2Collector",
    "RenogyBt2Snapshot",
]
