from __future__ import annotations

import os
import sys
import importlib.util
import types
from pathlib import Path

import pytest

from app import build_info
from app.config import get_settings

REPO_ROOT = Path(__file__).resolve().parents[3]
PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

build_info.BUILD_FLAVOR = os.environ.get("NODE_TEST_BUILD_FLAVOR", "test")


def _install_dbus_shims() -> bool:
    """Prevent dbus-next imports from failing on unsupported runtimes."""

    missing_dbus = importlib.util.find_spec("dbus_next") is None
    shim_needed = sys.version_info >= (3, 14) or missing_dbus
    if not shim_needed:
        return False

    service_mod = types.ModuleType("dbus_next.service")
    service_mod.ServiceInterface = object
    service_mod.method = lambda *args, **kwargs: (lambda fn: fn)
    service_mod.dbus_property = service_mod.method
    sys.modules.setdefault("dbus_next.service", service_mod)

    constants_mod = types.ModuleType("dbus_next.constants")

    class _DummyAccess:
        READ = "read"

    constants_mod.PropertyAccess = _DummyAccess  # type: ignore[attr-defined]
    sys.modules.setdefault("dbus_next.constants", constants_mod)

    aio_mod = types.ModuleType("dbus_next.aio")
    aio_mod.MessageBus = object
    sys.modules.setdefault("dbus_next.aio", aio_mod)

    root_mod = sys.modules.setdefault("dbus_next", types.ModuleType("dbus_next"))
    if not getattr(root_mod, "Variant", None):
        root_mod.Variant = object
    return True


_DBUS_SHIMMED = _install_dbus_shims()

if _DBUS_SHIMMED or sys.version_info >= (3, 14):
    try:
        import app.services.ble_provisioning as ble

        ble.DBUS_AVAILABLE = False

        async def _skip_start(self):  # type: ignore[override]
            self.available = False
            return None

        async def _skip_stop(self):  # type: ignore[override]
            self.available = False
            return None

        ble.BLEProvisioningManager.start = _skip_start
        ble.BLEProvisioningManager.stop = _skip_stop
    except Exception:
        pass


@pytest.fixture(autouse=True)
def reset_settings_env(monkeypatch, tmp_path):
    config_path = tmp_path / "config.json"
    monkeypatch.setenv("NODE_CONFIG_PATH", str(config_path))
    get_settings.cache_clear()
    yield
    get_settings.cache_clear()
