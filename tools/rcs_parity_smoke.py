#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
REPORTS_DIR = REPO_ROOT / "reports" / "rcs-parity-smoke"
CANONICAL_SPEC = REPO_ROOT / "apps" / "core-server-rs" / "openapi" / "farm-dashboard.json"
RUST_MANIFEST = REPO_ROOT / "apps" / "core-server-rs" / "Cargo.toml"
RUST_BINARY = REPO_ROOT / "apps" / "core-server-rs" / "target" / "debug" / "core-server-rs"


def timestamp_slug() -> str:
    return time.strftime("%Y-%m-%dT%H-%M-%SZ", time.gmtime())


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, env=os.environ.copy())


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def main() -> int:
    artifacts_dir = REPORTS_DIR / timestamp_slug()
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    log_path = artifacts_dir / "parity.log"

    if not CANONICAL_SPEC.exists():
        log_path.write_text(f"Missing canonical spec at {CANONICAL_SPEC}\n")
        print("rcs-parity-smoke: FAIL (missing canonical OpenAPI spec)")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    build = run(["cargo", "build", "--manifest-path", str(RUST_MANIFEST), "-q"])
    if build.returncode != 0:
        log_path.write_text(
            f"$ cargo build --manifest-path {RUST_MANIFEST} -q\n\n"
            f"STDOUT:\n{build.stdout}\n\nSTDERR:\n{build.stderr}\n"
        )
        print("rcs-parity-smoke: FAIL (rust build failed)")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    if not RUST_BINARY.exists():
        log_path.write_text(f"Rust binary missing at {RUST_BINARY}\n")
        print("rcs-parity-smoke: FAIL (rust binary missing)")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    rust_spec_proc = run([str(RUST_BINARY), "--print-openapi"])
    if rust_spec_proc.returncode != 0:
        log_path.write_text(
            f"$ {RUST_BINARY} --print-openapi\n\n"
            f"STDOUT:\n{rust_spec_proc.stdout}\n\nSTDERR:\n{rust_spec_proc.stderr}\n"
        )
        print("rcs-parity-smoke: FAIL (rust openapi export failed)")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    canonical = load_json(CANONICAL_SPEC)
    rust = json.loads(rust_spec_proc.stdout)

    ignore_paths = {"/api/openapi.json"}
    canonical_paths: dict = canonical.get("paths") or {}
    rust_paths: dict = rust.get("paths") or {}

    failures: list[str] = []
    for path, methods in sorted(rust_paths.items()):
        if path in ignore_paths:
            continue
        if path not in canonical_paths:
            failures.append(f"canonical spec missing path: {path}")
            continue
        canonical_methods = canonical_paths.get(path) or {}
        rust_methods = methods or {}
        for method_name in sorted(rust_methods.keys()):
            if method_name not in canonical_methods:
                failures.append(f"canonical spec missing {method_name.upper()} {path}")

    if failures:
        log_path.write_text(
            "RCS parity failures:\n"
            + "\n".join(f"- {item}" for item in failures)
            + "\n"
        )
        print(f"rcs-parity-smoke: FAIL ({len(failures)} mismatch(es))")
        print(f"Artifacts: {artifacts_dir}")
        return 1

    log_path.write_text("PASS\n")
    print("rcs-parity-smoke: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
