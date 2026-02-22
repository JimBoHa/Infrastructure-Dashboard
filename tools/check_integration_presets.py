#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from types import ModuleType
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
PRESETS_PATH = REPO_ROOT / "shared" / "presets" / "integrations.json"


def load_module(name: str, path: Path) -> ModuleType:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load module {name} from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # type: ignore[attr-defined]
    return module


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def build_renogy_defs_from_file(presets: dict[str, Any]) -> list[tuple[str, str, str, str]]:
    renogy = require_dict(presets.get("renogy_bt2"), "renogy_bt2")
    sensors = renogy.get("sensors")
    if not isinstance(sensors, list):
        raise ValueError("renogy_bt2.sensors must be a list")
    out: list[tuple[str, str, str, str]] = []
    for sensor in sensors:
        sensor_obj = require_dict(sensor, "renogy_bt2 sensor")
        metric = str(sensor_obj.get("metric") or "").strip()
        name = str(sensor_obj.get("name") or "").strip()
        core_type = str(sensor_obj.get("core_type") or "").strip()
        unit = str(sensor_obj.get("unit") or "").strip()
        if not metric or not name or not core_type:
            raise ValueError("renogy_bt2 sensor entries require metric/name/core_type")
        out.append((metric, name, core_type, unit))
    if not out:
        raise ValueError("renogy_bt2.sensors must not be empty")
    return out


def main() -> int:
    if not PRESETS_PATH.exists():
        print(f"preset-drift: missing {PRESETS_PATH}", file=sys.stderr)
        return 1

    presets = require_dict(json.loads(PRESETS_PATH.read_text(encoding="utf-8")), "integrations.json")
    canonical = build_renogy_defs_from_file(presets)

    tool_path = REPO_ROOT / "tools" / "renogy_node_deploy.py"
    tool = load_module("renogy_node_deploy", tool_path)
    if not hasattr(tool, "load_renogy_sensor_defs"):
        print("preset-drift: tools/renogy_node_deploy.py missing load_renogy_sensor_defs()", file=sys.stderr)
        return 1

    tool_defs = tool.load_renogy_sensor_defs()
    if tool_defs != canonical:
        print("preset-drift: Renogy preset definitions diverged between integrations.json and renogy_node_deploy.py", file=sys.stderr)
        print(f"preset-drift: canonical={canonical}", file=sys.stderr)
        print(f"preset-drift: tool={tool_defs}", file=sys.stderr)
        return 1

    print("preset-drift: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

