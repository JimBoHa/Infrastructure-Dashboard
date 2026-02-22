from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC_PATH = ROOT / "apps" / "core-server-rs" / "openapi" / "farm-dashboard.json"
EXTRAS_PATH = Path(__file__).resolve().parent / "openapi_extras.json"
RUST_MANIFEST = ROOT / "apps" / "core-server-rs" / "Cargo.toml"
RUST_BINARY = ROOT / "apps" / "core-server-rs" / "target" / "debug" / "core-server-rs"


def _load_extras() -> dict:
    if not EXTRAS_PATH.exists():
        return {}
    return json.loads(EXTRAS_PATH.read_text(encoding="utf-8"))


def _merge_components(spec: dict, extras: dict) -> None:
    if not extras:
        return
    spec.setdefault("components", {})
    extra_components = extras.get("components") or {}
    for key, value in extra_components.items():
        if isinstance(value, dict):
            spec.setdefault("components", {}).setdefault(key, {})
            spec["components"][key].update(value)
        else:
            spec["components"][key] = value


def _run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, check=False)


def _export_from_rust() -> dict:
    build = _run(["cargo", "build", "--manifest-path", str(RUST_MANIFEST), "-q"])
    if build.returncode != 0:
        raise RuntimeError(
            "Rust build failed:\n"
            f"$ cargo build --manifest-path {RUST_MANIFEST} -q\n\n"
            f"STDOUT:\n{build.stdout}\n\nSTDERR:\n{build.stderr}\n"
        )

    if not RUST_BINARY.exists():
        raise RuntimeError(f"Rust binary missing at {RUST_BINARY}")

    export = _run([str(RUST_BINARY), "--print-openapi"])
    if export.returncode != 0:
        raise RuntimeError(
            "Rust OpenAPI export failed:\n"
            f"$ {RUST_BINARY} --print-openapi\n\n"
            f"STDOUT:\n{export.stdout}\n\nSTDERR:\n{export.stderr}\n"
        )
    try:
        return json.loads(export.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            "Rust OpenAPI output was not valid JSON:\n"
            f"{exc}\n\n"
            f"STDOUT (first 2000 chars):\n{export.stdout[:2000]}\n\n"
            f"STDERR:\n{export.stderr}\n"
        ) from exc


def main() -> None:
    spec = _export_from_rust()
    extras = _load_extras()
    _merge_components(spec, extras)

    SPEC_PATH.parent.mkdir(parents=True, exist_ok=True)
    SPEC_PATH.write_text(json.dumps(spec, indent=2, sort_keys=True), encoding="utf-8")
    print(f"[api-sdk] wrote {SPEC_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
