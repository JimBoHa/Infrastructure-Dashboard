#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, Optional, Sequence

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python <3.11
    try:
        import tomli as tomllib  # type: ignore
    except ModuleNotFoundError:  # pragma: no cover - fallback for environments without tomli installed
        from pip._vendor import tomli as tomllib  # type: ignore


REPO_ROOT = Path(__file__).resolve().parent.parent
NODE_AGENT_ROOT = REPO_ROOT / "apps" / "node-agent"
LOCK_PATH = NODE_AGENT_ROOT / "poetry.lock"

NODE_PLATFORMS = ["manylinux2014_aarch64", "manylinux_2_34_aarch64", "linux_aarch64"]
TARGET_IMPLEMENTATION = "cp"
PIWHEELS_SIMPLE = "https://www.piwheels.org/simple"

@dataclass(frozen=True)
class PythonTarget:
    version: str  # e.g. "3.11"
    tag: str  # e.g. "311" (pip --python-version value)
    abi: str  # e.g. "cp311" (pip --abi value)


PYTHON_TARGETS: Sequence[PythonTarget] = (
    PythonTarget(version="3.11", tag="311", abi="cp311"),
    PythonTarget(version="3.13", tag="313", abi="cp313"),
)

# Poetry lock pins are evaluated for Python 3.11; a small set of packages lag in publishing
# wheels for newer runtimes. Keep the lock stable but override a few pins when targeting newer
# interpreters so offline Pi deployments remain installable.
PIN_OVERRIDES_BY_PYTHON_TAG: dict[str, dict[str, str]] = {
    # zeroconf 0.132.x does not publish cp313 wheels; 0.133.0 does.
    "313": {"zeroconf": "0.133.0", "lgpio": "0.0.0.2"},
}

PIGPIO_VERSION = "1.79-1+rpt1"
PIGPIO_BASE_URL = "https://archive.raspberrypi.org/debian/pool/main/p/pigpio"
PIGPIO_PACKAGES = [
    # Runtime-only set for Pi 5 pulse inputs:
    # - pigpiod provides the daemon + systemd unit.
    # - libpigpio1 provides the shared library used by pigpiod/tools.
    # - libpigpiod-if1/if2 provide the client libraries used by bindings/tools.
    # - pigpio-tools is useful for field debugging.
    #
    # NOTE: Do NOT stage the tiny `pigpio` meta-package; it depends on `*-dev` packages and will
    # fail to configure on clean installs (and is not required for runtime operation).
    "pigpiod",
    "pigpio-tools",
    "libpigpio1",
    "libpigpiod-if1",
    "libpigpiod-if2-1",
]

DEBIAN_POOL = "https://deb.debian.org/debian/pool/main"
PY_DEB_URLS = [
    # No aarch64 wheels on PyPI; ship as debs instead.
    f"{DEBIAN_POOL}/s/spidev/python3-spidev_3.6-1+b7_arm64.deb",
]
SKIP_WHEEL_PINS = {"rpi-gpio", "spidev"}


def main() -> int:
    parser = argparse.ArgumentParser(description="Build offline Pi 5 node deps (vendor site-packages + pigpio debs).")
    parser.add_argument("--vendor-dir", type=Path, required=True, help="Destination vendor dir (site-packages layout).")
    parser.add_argument("--debs-dir", type=Path, required=True, help="Destination debs dir (pigpio debs).")
    parser.add_argument("--cache-root", type=Path, default=REPO_ROOT / "build" / "cache", help="Cache root.")
    args = parser.parse_args()

    args.cache_root.mkdir(parents=True, exist_ok=True)
    build_python_vendor(vendor_dir=args.vendor_dir, cache_root=args.cache_root)
    stage_pigpio_debs(debs_dir=args.debs_dir, cache_root=args.cache_root)
    return 0


def build_python_vendor(*, vendor_dir: Path, cache_root: Path) -> None:
    lock_bytes = LOCK_PATH.read_bytes()
    lock_hash = hashlib.sha256(lock_bytes).hexdigest()
    pip_cache = cache_root / "pip-cache"
    pip_cache.mkdir(parents=True, exist_ok=True)

    for target in PYTHON_TARGETS:
        target_vendor = vendor_dir / f"py{target.tag}"
        overrides = PIN_OVERRIDES_BY_PYTHON_TAG.get(target.tag) or {}
        override_suffix = ""
        if overrides:
            override_key = ",".join(f"{name}=={version}" for name, version in sorted(overrides.items()))
            override_suffix = "-" + hashlib.sha256(override_key.encode("utf-8")).hexdigest()[:12]
        vendor_cache = cache_root / f"node-agent-vendor-pi{target.tag}-{lock_hash}{override_suffix}"
        wheelhouse_cache = cache_root / f"node-agent-wheelhouse-pi{target.tag}-{lock_hash}{override_suffix}"

        if vendor_cache.exists():
            _copytree(vendor_cache, target_vendor)
            continue

        pinned = resolve_lock_pins(lock_bytes, python_version=target.version)
        pinned = apply_pin_overrides(pinned, overrides=overrides)

        wheelhouse_cache.mkdir(parents=True, exist_ok=True)
        download_wheels(
            pinned=pinned,
            wheelhouse=wheelhouse_cache,
            pip_cache=pip_cache,
            python_version_tag=target.tag,
            target_abi=target.abi,
        )

        with tempfile.TemporaryDirectory(prefix=f"node-agent-vendor-{target.tag}-") as tmp:
            temp_vendor = Path(tmp) / "vendor"
            temp_vendor.mkdir(parents=True, exist_ok=True)
            install_wheelhouse(wheelhouse=wheelhouse_cache, vendor_dir=temp_vendor)
            _copytree(temp_vendor, vendor_cache)
            _copytree(vendor_cache, target_vendor)


def resolve_lock_pins(lock_bytes: bytes, *, python_version: str) -> list[str]:
    data = tomllib.loads(lock_bytes.decode("utf-8"))
    packages: list[dict[str, Any]] = list(data.get("package", []))

    marker_cls, specifier_cls = _packaging_types()

    target_env = {
        "python_version": python_version,
        "python_full_version": f"{python_version}.0",
        "platform_system": "Linux",
        "platform_machine": "aarch64",
        "sys_platform": "linux",
        "os_name": "posix",
    }

    pins: list[str] = []
    for pkg in packages:
        groups = set(pkg.get("groups") or [])
        if "main" not in groups:
            continue
        markers_value = pkg.get("markers")
        markers = ""
        if isinstance(markers_value, str):
            markers = markers_value.strip()
        elif isinstance(markers_value, dict):
            markers = str(markers_value.get("main") or "").strip()
        if markers:
            if not marker_cls(markers).evaluate(environment=target_env):
                continue
        python_versions = str(pkg.get("python-versions") or "").strip()
        if python_versions and python_versions != "*":
            if not specifier_cls(python_versions).contains(
                python_version,
                prereleases=True,
            ):
                continue
        name = str(pkg.get("name") or "").strip()
        version = str(pkg.get("version") or "").strip()
        if not name or not version:
            continue
        if name in SKIP_WHEEL_PINS:
            continue
        pins.append(f"{name}=={version}")

    pins.sort()
    return pins


def apply_pin_overrides(pins: list[str], *, overrides: dict[str, str]) -> list[str]:
    if not overrides:
        return pins

    resolved: dict[str, str] = {}
    for pin in pins:
        if "==" not in pin:
            continue
        name, version = pin.split("==", 1)
        resolved[name.strip()] = version.strip()

    for name, version in overrides.items():
        resolved[name] = version

    merged = [f"{name}=={version}" for name, version in resolved.items()]
    merged.sort()
    return merged


def download_wheels(
    *,
    pinned: Iterable[str],
    wheelhouse: Path,
    pip_cache: Path,
    python_version_tag: str,
    target_abi: str,
) -> None:
    requirements_file = wheelhouse / "requirements-pins.txt"
    requirements_file.write_text("\n".join(pinned) + "\n", encoding="utf-8")

    cmd = [
        "python3",
        "-m",
        "pip",
        "download",
        "--disable-pip-version-check",
        "--only-binary=:all:",
        "--no-deps",
        "--dest",
        str(wheelhouse),
        "--python-version",
        python_version_tag,
        "--implementation",
        TARGET_IMPLEMENTATION,
        "--abi",
        target_abi,
        "-r",
        str(requirements_file),
    ]
    for platform in NODE_PLATFORMS:
        cmd.extend(["--platform", platform])

    env = dict(os.environ)
    env.update(
        {
            "PIP_DISABLE_PIP_VERSION_CHECK": "1",
            "PIP_CACHE_DIR": str(pip_cache),
        }
    )
    if not env.get("PIP_EXTRA_INDEX_URL"):
        env["PIP_EXTRA_INDEX_URL"] = PIWHEELS_SIMPLE
    proc = subprocess.run(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if proc.returncode != 0:
        output = (proc.stdout or "").strip().splitlines()
        tail = "\n".join(output[-40:])
        raise RuntimeError(f"pip download failed (exit={proc.returncode}). Tail:\n{tail}")


def install_wheelhouse(*, wheelhouse: Path, vendor_dir: Path) -> None:
    for wheel in sorted(wheelhouse.glob("*.whl")):
        install_wheel(wheel=wheel, vendor_dir=vendor_dir)


def install_wheel(*, wheel: Path, vendor_dir: Path) -> None:
    with zipfile.ZipFile(wheel) as zf:
        for member in zf.infolist():
            if member.is_dir():
                continue
            src = member.filename
            dest_rel = _wheel_member_dest(src)
            if dest_rel is None:
                continue
            dest_path = vendor_dir / dest_rel
            dest_path.parent.mkdir(parents=True, exist_ok=True)
            with zf.open(member) as rfh, dest_path.open("wb") as wfh:
                shutil.copyfileobj(rfh, wfh)


def _wheel_member_dest(path: str) -> Optional[str]:
    if ".data/purelib/" in path:
        return path.split(".data/purelib/", 1)[1]
    if ".data/platlib/" in path:
        return path.split(".data/platlib/", 1)[1]
    # Keep dist-info and top-level modules as-is.
    return path


def stage_pigpio_debs(*, debs_dir: Path, cache_root: Path) -> None:
    debs_dir.mkdir(parents=True, exist_ok=True)
    cache_dir = cache_root / "pigpio-debs" / PIGPIO_VERSION
    cache_dir.mkdir(parents=True, exist_ok=True)

    for package in PIGPIO_PACKAGES:
        filename = f"{package}_{PIGPIO_VERSION}_arm64.deb"
        cached = cache_dir / filename
        if not cached.exists():
            url = f"{PIGPIO_BASE_URL}/{filename}"
            with urllib.request.urlopen(url, timeout=60) as resp:
                cached.write_bytes(resp.read())
        shutil.copy2(cached, debs_dir / filename)

    for url in PY_DEB_URLS:
        filename = url.rsplit("/", 1)[-1]
        cached = cache_dir / filename
        if not cached.exists():
            with urllib.request.urlopen(url, timeout=60) as resp:
                cached.write_bytes(resp.read())
        shutil.copy2(cached, debs_dir / filename)


def _copytree(src: Path, dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(src, dest)


def _packaging_types():
    """Return (Marker, SpecifierSet) from packaging or pip's vendored copy."""

    try:
        from packaging.markers import Marker  # type: ignore
        from packaging.specifiers import SpecifierSet  # type: ignore

        return Marker, SpecifierSet
    except Exception:
        from pip._vendor.packaging.markers import Marker  # type: ignore
        from pip._vendor.packaging.specifiers import SpecifierSet  # type: ignore

        return Marker, SpecifierSet


if __name__ == "__main__":
    raise SystemExit(main())
