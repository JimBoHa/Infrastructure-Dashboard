from __future__ import annotations

import shutil
import subprocess
import sys
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = Path(__file__).resolve().parent
SPEC_PATH = ROOT / "apps" / "core-server-rs" / "openapi" / "farm-dashboard.json"

TS_OUT = ROOT / "apps" / "dashboard-web" / "src" / "lib" / "api-client"
PY_OUT = ROOT / "apps" / "node-agent" / "app" / "generated_api"

TS_CONFIG = TOOLS_DIR / "config" / "typescript-fetch.yaml"
PY_CONFIG = TOOLS_DIR / "config" / "python.yaml"


def _run(cmd: list[str]) -> None:
    print("[api-sdk]", " ".join(cmd))
    subprocess.run(cmd, check=True)


def _assert_absent(path: Path) -> None:
    if path.exists():
        raise RuntimeError(
            f"Unexpected generated artifacts present: {path}\n"
            "This repo intentionally keeps generated SDKs minimal: docs/tests are disabled.\n"
            "If you need SDK docs/tests, generate into a temporary directory instead."
        )


def _ensure_generator_cli() -> Path:
    cli = TOOLS_DIR / "node_modules" / ".bin" / "openapi-generator-cli"
    if cli.exists():
        return cli
    raise FileNotFoundError(
        "openapi-generator-cli not found; run 'npm --prefix tools/api-sdk install' first"
    )


def _clean(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)


def _tmp_out(path: Path) -> Path:
    return path.with_name(f"{path.name}.tmp")


def _generate_into(*, out_dir: Path, cmd: list[str]) -> None:
    tmp = _tmp_out(out_dir)
    _clean(tmp)
    cmd = list(cmd)
    for idx, token in enumerate(cmd):
        if token == "-o" and idx + 1 < len(cmd):
            cmd[idx + 1] = str(tmp)
    _run(cmd)
    _clean(out_dir)
    tmp.rename(out_dir)


def _ensure_init(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    init_file = path / "__init__.py"
    if not init_file.exists():
        init_file.write_text("", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate SDKs from the OpenAPI contract.")
    parser.add_argument(
        "--targets",
        default="ts,py",
        help="Comma-separated list of SDK targets to generate: ts, py (default: ts,py)",
    )
    args = parser.parse_args()
    targets = {item.strip().lower() for item in args.targets.split(",") if item.strip()}

    _run([sys.executable, str(TOOLS_DIR / "export_openapi.py")])
    cli = _ensure_generator_cli()

    if "ts" in targets:
        _generate_into(
            out_dir=TS_OUT,
            cmd=[
                str(cli),
                "generate",
                "-i",
                str(SPEC_PATH),
                "-g",
                "typescript-fetch",
                "-o",
                str(TS_OUT),
                "-c",
                str(TS_CONFIG),
                "--global-property=apiDocs=false,modelDocs=false,apiTests=false,modelTests=false",
            ],
        )
        _assert_absent(TS_OUT / "docs")

    if "py" in targets:
        _generate_into(
            out_dir=PY_OUT,
            cmd=[
                str(cli),
                "generate",
                "-i",
                str(SPEC_PATH),
                "-g",
                "python",
                "-o",
                str(PY_OUT),
                "-c",
                str(PY_CONFIG),
                "--global-property=models,modelDocs=false,modelTests=false,apiDocs=false,apiTests=false",
            ],
        )
        _ensure_init(PY_OUT)
        _ensure_init(PY_OUT / "generated_api")
        _ensure_init(PY_OUT / "generated_api" / "models")
        _assert_absent(PY_OUT / "generated_api" / "docs")
        _assert_absent(PY_OUT / "generated_api" / "test")


if __name__ == "__main__":
    main()
