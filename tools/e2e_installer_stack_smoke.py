#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import test_hygiene

REPO_ROOT = Path(__file__).resolve().parents[1]
REPORTS_DIR = REPO_ROOT / "reports" / "e2e-installer-stack-smoke"
SETUP_REPORT = REPO_ROOT / "reports" / "e2e-setup-smoke" / "last_state.json"


def stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")


def run_step(
    label: str,
    cmd: list[str],
    *,
    env: dict[str, str],
    artifacts_dir: Path,
) -> None:
    log_path = artifacts_dir / f"{label}_{stamp()}.log"
    with log_path.open("w", encoding="utf-8") as log:
        log.write(f"$ {' '.join(cmd)}\n\n")
        log.flush()
        result = subprocess.run(cmd, stdout=log, stderr=log, text=True, env=env)
    if result.returncode != 0:
        raise RuntimeError(f"{label} failed (see {log_path})")


def load_setup_state() -> dict:
    if not SETUP_REPORT.exists():
        raise RuntimeError(
            f"Missing setup state at {SETUP_REPORT}. Run e2e-setup-smoke with FARM_E2E_KEEP_TEMP=1."
        )
    return json.loads(SETUP_REPORT.read_text())


def uninstall_preserved_install(state: dict, *, artifacts_dir: Path) -> None:
    preserved = bool(state.get("preserved"))
    if not preserved:
        return
    install_root = Path(str(state.get("install_root", "")))
    config_path = Path(str(state.get("config_path", "")))
    state_dir = Path(str(state.get("state_dir", "")))

    farmctl = install_root / "bin" / "farmctl"
    if not farmctl.exists():
        return
    if not config_path.exists():
        return
    env = os.environ.copy()
    if state_dir:
        env["FARM_SETUP_STATE_DIR"] = str(state_dir)

    run_step(
        "cleanup_uninstall",
        [
            str(farmctl),
            "--profile",
            "e2e",
            "uninstall",
            "--config",
            str(config_path),
            "--remove-roots",
            "--yes",
        ],
        env=env,
        artifacts_dir=artifacts_dir,
    )

    temp_root = Path(str(state.get("temp_root", "")))
    if temp_root.exists():
        shutil.rmtree(temp_root, ignore_errors=True)


def quiesce_launchd_services_for_web_smoke(*, artifacts_dir: Path) -> None:
    """
    The setup-smoke path validates launchd install/upgrade/rollback and leaves the
    E2E services running so follow-on smokes can use the preserved temp root.

    The web smoke harness starts installed binaries directly (not via launchd) and
    enforces a clean preflight. Stop any launchd jobs/processes from setup-smoke
    before running the web smoke.
    """

    state = test_hygiene.collect_state(
        label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
        ps_regex=test_hygiene.DEFAULT_PS_REGEX,
    )
    if state.is_clean():
        return

    if state.dmg_images:
        test_hygiene.detach_dmg_images(state.dmg_images)
    if state.launchd_jobs:
        test_hygiene.remove_launchd_jobs(state.launchd_jobs)
    if state.processes:
        test_hygiene.terminate_processes(state.processes, timeout_seconds=10.0)

    post = test_hygiene.collect_state(
        label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
        ps_regex=test_hygiene.DEFAULT_PS_REGEX,
    )
    if not post.is_clean():
        detail = test_hygiene.format_state(
            post,
            label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
            ps_regex=test_hygiene.DEFAULT_PS_REGEX,
        )
        (artifacts_dir / "web-smoke-quiesce.txt").write_text(detail)
        raise RuntimeError(
            "Failed to quiesce launchd services before web smoke (see web-smoke-quiesce.txt)."
        )


def main() -> int:
    artifacts_dir = REPORTS_DIR / stamp()
    artifacts_dir.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["FARM_E2E_KEEP_TEMP"] = "1"

    preflight = test_hygiene.collect_state(
        label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
        ps_regex=test_hygiene.DEFAULT_PS_REGEX,
    )
    if not preflight.is_clean():
        print("e2e-installer-stack-smoke: PRECHECK FAIL (machine not clean)")
        print(
            test_hygiene.format_state(
                preflight,
                label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
                ps_regex=test_hygiene.DEFAULT_PS_REGEX,
            )
        )
        print("Run cleanup first: make test-clean")
        return 2

    exit_code = 1
    error: str | None = None
    try:
        run_step(
            "e2e_setup_smoke",
            [sys.executable, str(REPO_ROOT / "tools" / "e2e_setup_smoke.py")],
            env=env,
            artifacts_dir=artifacts_dir,
        )
        run_step(
            "e2e_installed_health_smoke",
            [sys.executable, str(REPO_ROOT / "tools" / "e2e_installed_health_smoke.py")],
            env={**env, "FARM_E2E_REQUIRE_INSTALLED": "1"},
            artifacts_dir=artifacts_dir,
        )
        run_step(
            "e2e_preset_flows_smoke",
            [sys.executable, str(REPO_ROOT / "tools" / "e2e_preset_flows_smoke.py")],
            env={**env, "FARM_E2E_REQUIRE_INSTALLED": "1"},
            artifacts_dir=artifacts_dir,
        )
        quiesce_launchd_services_for_web_smoke(artifacts_dir=artifacts_dir)
        run_step(
            "e2e_web_smoke",
            [sys.executable, str(REPO_ROOT / "tools" / "e2e_web_smoke.py")],
            env={**env, "FARM_E2E_REQUIRE_INSTALLED": "1"},
            artifacts_dir=artifacts_dir,
        )

        exit_code = 0
    except Exception as exc:
        error = str(exc)
        exit_code = 1
    finally:
        cleanup_ok = True
        try:
            state = load_setup_state()
        except Exception:
            state = {}
        try:
            uninstall_preserved_install(state, artifacts_dir=artifacts_dir)
        except Exception as exc:
            cleanup_ok = False
            if error:
                error = f"{error}; cleanup failed ({exc})"
            else:
                error = f"cleanup failed ({exc})"

        try:
            post = test_hygiene.collect_state(
                label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
                ps_regex=test_hygiene.DEFAULT_PS_REGEX,
            )
            if not post.is_clean():
                cleanup_ok = False
                detail = test_hygiene.format_state(
                    post,
                    label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
                    ps_regex=test_hygiene.DEFAULT_PS_REGEX,
                )
                if error:
                    error = f"{error}; postflight not clean"
                else:
                    error = "postflight not clean"
                print("e2e-installer-stack-smoke: POSTCHECK FAIL (orphaned services/processes remain)")
                print(detail)
        except Exception as exc:
            cleanup_ok = False
            if error:
                error = f"{error}; postflight check failed ({exc})"
            else:
                error = f"postflight check failed ({exc})"

        if not cleanup_ok:
            exit_code = 1

    if exit_code == 0:
        print("e2e-installer-stack-smoke: PASS")
        return 0

    print(f"e2e-installer-stack-smoke: FAIL ({error})")
    print(f"Artifacts: {artifacts_dir}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
