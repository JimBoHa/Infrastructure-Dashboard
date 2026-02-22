#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlparse

import test_hygiene


def run(cmd: list[str], env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, env=env)


def ensure_ok(result: subprocess.CompletedProcess[str], label: str, artifacts_dir: Path) -> None:
    if result.returncode == 0:
        return
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    failure_log = artifacts_dir / f"{label}_{stamp}.log"
    failure_log.write_text(
        f"$ {' '.join(result.args)}\n\nSTDOUT:\n{result.stdout}\n\nSTDERR:\n{result.stderr}\n"
    )
    raise RuntimeError(f"{label} failed (see {failure_log})")


def allocate_port() -> int:
    import socket

    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
    sock.close()
    return port


def wait_for_http(url: str, timeout_seconds: int) -> None:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        try:
            result = subprocess.run(
                ["curl", "-fsS", url],
                capture_output=True,
                text=True,
            )
            if result.returncode == 0:
                return
        except Exception:
            pass
        time.sleep(0.25)
    raise RuntimeError(f"Timed out waiting for {url}")


def parse_database_port(database_url: str) -> int:
    normalized = database_url.replace("postgresql+psycopg", "postgresql")
    parsed = urlparse(normalized)
    if parsed.port:
        return parsed.port
    return 5432


def port_is_free(port: int) -> bool:
    import socket

    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(("127.0.0.1", port))
        return True
    except OSError:
        return False
    finally:
        sock.close()


def wait_for_ports_free(ports: list[int], timeout_seconds: int) -> None:
    deadline = time.time() + timeout_seconds
    ports = sorted({p for p in ports if isinstance(p, int) and p > 0})
    while time.time() < deadline:
        busy = [port for port in ports if not port_is_free(port)]
        if not busy:
            return
        time.sleep(0.25)
    raise RuntimeError(f"Ports did not become free after uninstall: {ports}")


def assert_no_launchd_labels(prefix: str) -> None:
    if not prefix.strip():
        return
    result = subprocess.run(
        ["launchctl", "list"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"launchctl list failed: {result.stderr.strip() or result.stdout.strip()}"
        )
    if prefix in result.stdout:
        raise RuntimeError(f"launchd jobs still present for prefix: {prefix}")


def read_launchd_overrides(prefix: str) -> list[str]:
    if not prefix.strip():
        return []
    uid = os.getuid()
    disabled_plist = Path(f"/var/db/com.apple.xpc.launchd/disabled.{uid}.plist")
    if not disabled_plist.exists():
        return []
    result = subprocess.run(
        ["plutil", "-convert", "json", "-o", "-", str(disabled_plist)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return []
    try:
        payload = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        return []
    if not isinstance(payload, dict):
        return []
    return sorted([key for key in payload.keys() if isinstance(key, str) and key.startswith(prefix)])


def read_state(state_path: Path) -> dict:
    if not state_path.exists():
        return {}
    return json.loads(state_path.read_text())


def write_last_state(
    report_path: Path,
    *,
    temp_root: Path,
    install_root: Path,
    state_dir: Path,
    config_path: Path,
    preserved: bool,
    extra: dict[str, object] | None = None,
) -> None:
    report_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "temp_root": str(temp_root),
        "install_root": str(install_root),
        "state_dir": str(state_dir),
        "config_path": str(config_path),
        "preserved": preserved,
    }
    if extra:
        payload.update(extra)
    report_path.write_text(json.dumps(payload, indent=2))


def assert_native_installed(install_root: Path) -> None:
    required = [
        install_root / "native/postgres/bin/postgres",
        install_root / "native/postgres/bin/initdb",
        install_root / "native/redis/bin/redis-server",
        install_root / "native/qdrant/bin/qdrant",
    ]
    missing = [str(path) for path in required if not path.exists()]
    mosquitto_bin = install_root / "native/mosquitto/bin/mosquitto"
    mosquitto_sbin = install_root / "native/mosquitto/sbin/mosquitto"
    if not mosquitto_bin.exists() and not mosquitto_sbin.exists():
        missing.append(f"{mosquitto_bin} or {mosquitto_sbin}")
    if missing:
        raise RuntimeError(f"Native deps missing after install: {', '.join(missing)}")


class DmgMount:
    def __init__(self, dmg_path: Path):
        self.dmg_path = dmg_path
        self.mount_dir = Path(tempfile.mkdtemp(prefix="farm_setup_dmg_"))
        last_error: str | None = None
        delay_seconds = 0.25
        for attempt in range(1, 6):
            result = subprocess.run(
                [
                    "hdiutil",
                    "attach",
                    str(dmg_path),
                    "-nobrowse",
                    "-readonly",
                    "-mountpoint",
                    str(self.mount_dir),
                ],
                capture_output=True,
                text=True,
            )
            if result.returncode == 0:
                last_error = None
                break
            last_error = (result.stderr.strip() or result.stdout.strip() or "unknown error").strip()
            subprocess.run(
                ["hdiutil", "detach", str(self.mount_dir), "-quiet"],
                capture_output=True,
                text=True,
                check=False,
            )
            if attempt < 5:
                time.sleep(delay_seconds)
                delay_seconds = min(delay_seconds * 2.0, 2.0)

        if last_error:
            raise RuntimeError(f"Failed to mount DMG {dmg_path}: {last_error}")

    def __enter__(self) -> "DmgMount":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        subprocess.run(["hdiutil", "detach", str(self.mount_dir), "-quiet"], check=False)
        shutil.rmtree(self.mount_dir, ignore_errors=True)


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    artifacts_dir = repo_root / "reports" / "e2e-setup-smoke"
    keep_temp = os.environ.get("FARM_E2E_KEEP_TEMP", "").strip().lower() in {
        "1",
        "true",
        "yes",
    }

    preflight = test_hygiene.collect_state(
        label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
        ps_regex=test_hygiene.DEFAULT_PS_REGEX,
    )
    if not preflight.is_clean():
        print("e2e-setup-smoke: PRECHECK FAIL (machine not clean)")
        print(
            test_hygiene.format_state(
                preflight,
                label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
                ps_regex=test_hygiene.DEFAULT_PS_REGEX,
            )
        )
        print("Run cleanup first: make test-clean")
        return 2

    temp_root = Path(tempfile.mkdtemp(prefix="farm_setup_e2e_"))
    install_root = temp_root / "install"
    data_root = temp_root / "data"
    logs_root = temp_root / "logs"
    backup_root = temp_root / "backups"
    state_dir = temp_root / "state"
    config_path = state_dir / "config.json"

    env = os.environ.copy()
    env.update(
        {
            "FARM_SETUP_STATE_DIR": str(state_dir),
        }
    )

    build = run(
        ["cargo", "build", "--release", "--manifest-path", "apps/farmctl/Cargo.toml"],
        env,
    )
    success = False
    error: str | None = None
    postflight_clean = True
    quarantined_installer_dmg = False
    try:
        ensure_ok(build, "farmctl_build", artifacts_dir)
        farmctl = repo_root / "apps/farmctl/target/release/farmctl"
        if not farmctl.exists():
            raise RuntimeError("farmctl binary missing after build")

        native_deps_env = os.environ.get("FARM_E2E_NATIVE_DEPS")
        if native_deps_env:
            native_deps = Path(native_deps_env)
            if not native_deps.exists():
                raise RuntimeError(f"FARM_E2E_NATIVE_DEPS not found at {native_deps}")
        else:
            native_deps = temp_root / "native-deps"
            native_build = run(
                [str(farmctl), "native-deps", "--output", str(native_deps)],
                env,
            )
            ensure_ok(native_build, "native_deps", artifacts_dir)

        bundle_v1 = temp_root / "FarmDashboardController-0.0.0-test.dmg"
        bundle_one = run(
            [
                str(farmctl),
                "bundle",
                "--version",
                "0.0.0-test",
                "--output",
                str(bundle_v1),
                "--native-deps",
                str(native_deps),
            ],
            env,
        )
        ensure_ok(bundle_one, "bundle_v1", artifacts_dir)

        installer_dmg = temp_root / "FarmDashboardInstaller-0.0.0-test.dmg"
        installer = run(
            [
                str(farmctl),
                "installer",
                "--version",
                "0.0.0-test",
                "--bundle",
                str(bundle_v1),
                "--output",
                str(installer_dmg),
                "--skip-build",
                "--farmctl-binary",
                str(farmctl),
            ],
            env,
        )
        ensure_ok(installer, "installer_dmg", artifacts_dir)

        if os.environ.get("FARM_E2E_QUARANTINE_INSTALLER_DMG", "").strip().lower() in {
            "1",
            "true",
            "yes",
        }:
            quarantine_value = f"0081;{int(time.time())};FarmDashboardInstaller;{installer_dmg.name}"
            quarantine = run(
                [
                    "xattr",
                    "-w",
                    "com.apple.quarantine",
                    quarantine_value,
                    str(installer_dmg),
                ],
                env,
            )
            ensure_ok(quarantine, "quarantine_installer_dmg", artifacts_dir)
            quarantined_installer_dmg = True

        # Wizard-driven install/upgrade/rollback against a clean temp root using the E2E profile.
        with DmgMount(installer_dmg) as mount:
            installer_root = mount.mount_dir
            installer_app = installer_root / "Farm Dashboard Installer.app"
            bundle_in_installer = (
                installer_app
                / "Contents/Resources/FarmDashboardController-0.0.0-test.dmg"
            )
            embedded_farmctl = installer_app / "Contents/Resources/farmctl"

            if not installer_app.exists():
                raise RuntimeError("Installer app missing in DMG")
            if list(installer_root.glob("FarmDashboardController-*.dmg")):
                raise RuntimeError(
                    "Controller bundle DMG was visible at the installer DMG root (expected embedded inside the installer app)"
                )
            if not bundle_in_installer.exists():
                raise RuntimeError("Controller bundle missing in installer DMG")
            if not embedded_farmctl.exists():
                raise RuntimeError("Embedded farmctl missing in installer app")

            bundle_v2 = temp_root / "FarmDashboardController-0.0.1-test.dmg"
            bundle_two = run(
                [
                    str(farmctl),
                    "bundle",
                    "--version",
                    "0.0.1-test",
                    "--output",
                    str(bundle_v2),
                    "--native-deps",
                    str(native_deps),
                ],
                env,
            )
            ensure_ok(bundle_two, "bundle_v2", artifacts_dir)

            setup_port = allocate_port()
            serve_proc = subprocess.Popen(
                [
                    str(embedded_farmctl),
                    "--profile",
                    "e2e",
                    "serve",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    str(setup_port),
                    "--config",
                    str(config_path),
                    "--no-auto-open",
                ],
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            try:
                wait_for_http(f"http://127.0.0.1:{setup_port}/healthz", 30)

                preflight = subprocess.run(
                    ["curl", "-fsS", f"http://127.0.0.1:{setup_port}/api/preflight"],
                    env=env,
                    capture_output=True,
                    text=True,
                )
                ensure_ok(preflight, "preflight_api", artifacts_dir)
                checks = json.loads(preflight.stdout or "{}").get("checks") or []
                warns = [check for check in checks if check.get("status") == "warn"]
                if warns:
                    sample = ", ".join(check.get("id", "?") for check in warns[:6])
                    raise RuntimeError(
                        f"Expected no preflight warnings on a clean E2E run; found {len(warns)} warn checks (sample: {sample})"
                    )

                wizard_script = (
                    repo_root / "apps/dashboard-web/scripts/setup-wizard-smoke.mjs"
                )
                wizard = subprocess.run(
                    [
                        "node",
                        str(wizard_script),
                        f"--base-url=http://127.0.0.1:{setup_port}",
                        f"--install-root={install_root}",
                        f"--data-root={data_root}",
                        f"--backup-root={backup_root}",
                        f"--logs-root={logs_root}",
                        f"--upgrade-bundle-path={bundle_v2}",
                        "--expected-install-version=0.0.0-test",
                        "--expected-upgrade-version=0.0.1-test",
                    ],
                    env=env,
                    capture_output=True,
                    text=True,
                )
                ensure_ok(wizard, "wizard_flow", artifacts_dir)
            finally:
                serve_proc.terminate()
                try:
                    serve_proc.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    serve_proc.kill()

        # Validate install results on disk.
        state = read_state(install_root / "state.json")
        if state.get("current_version") != "0.0.0-test":
            raise RuntimeError("install did not record expected version")
        assert_native_installed(install_root)

        installed_farmctl = install_root / "bin/farmctl"
        if not installed_farmctl.exists():
            raise RuntimeError("Installed farmctl missing after install")

        config_payload = read_state(config_path)
        launchd_prefix = str(config_payload.get("launchd_label_prefix") or "").strip()
        ports_to_free = [
            config_payload.get("core_port"),
            config_payload.get("mqtt_port"),
            config_payload.get("redis_port"),
        ]
        db_url = str(config_payload.get("database_url") or "")
        if db_url:
            ports_to_free.append(parse_database_port(db_url))
        ports_to_free = [int(value) for value in ports_to_free if isinstance(value, int)]

        # Validate uninstall/reset so repeated installs are safe.
        uninstall = run(
            [
                str(installed_farmctl),
                "--profile",
                "e2e",
                "uninstall",
                "--config",
                str(config_path),
                "--remove-roots",
                "--yes",
            ],
            env,
        )
        ensure_ok(uninstall, "uninstall", artifacts_dir)
        if install_root.exists() or data_root.exists() or logs_root.exists() or state_dir.exists():
            raise RuntimeError("uninstall did not remove expected roots")
        assert_no_launchd_labels(launchd_prefix)
        overrides = read_launchd_overrides(launchd_prefix)
        if overrides:
            uid = os.getuid()
            raise RuntimeError(
                "launchd enable/disable overrides persisted for this E2E label prefix "
                f"(state pollution). Sample keys: {', '.join(overrides[:6])}\n"
                f"To purge stale overrides (one-time, requires admin): sudo python3 tools/purge_launchd_overrides.py --uid {uid} --apply --backup"
            )
        wait_for_ports_free(ports_to_free, 60)

        # Optional: re-install (headless) so follow-up E2E flows can run against a preserved install.
        if keep_temp:
            state_dir.mkdir(parents=True, exist_ok=True)
            payload = {
                "install_root": str(install_root),
                "data_root": str(data_root),
                "logs_root": str(logs_root),
                "backup_root": str(backup_root),
                "backup_retention_days": 30,
                "profile": "e2e",
            }
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(json.dumps(payload, indent=2))
            reinstall = run(
                [
                    str(farmctl),
                    "--profile",
                    "e2e",
                    "install",
                    "--bundle",
                    str(bundle_v1),
                    "--config",
                    str(config_path),
                ],
                env,
            )
            ensure_ok(reinstall, "reinstall", artifacts_dir)
            state = read_state(install_root / "state.json")
            if state.get("current_version") != "0.0.0-test":
                raise RuntimeError("reinstall did not record expected version")
            assert_native_installed(install_root)

        success = True
    except Exception as exc:
        error = str(exc)
    finally:
        cleanup_log: Path | None = None
        if not keep_temp and not success:
            # Best-effort: stop services so failed runs do not pollute the machine.
            farmctl = repo_root / "apps/farmctl/target/release/farmctl"
            if farmctl.exists() and config_path.exists():
                cleanup = run(
                    [
                        str(farmctl),
                        "--profile",
                        "e2e",
                        "uninstall",
                        "--config",
                        str(config_path),
                        "--yes",
                    ],
                    env,
                )
                if cleanup.returncode != 0:
                    artifacts_dir.mkdir(parents=True, exist_ok=True)
                    stamp = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
                    cleanup_log = artifacts_dir / f"cleanup_uninstall_{stamp}.log"
                    cleanup_log.write_text(
                        f"$ {' '.join(cleanup.args)}\n\nSTDOUT:\n{cleanup.stdout}\n\nSTDERR:\n{cleanup.stderr}\n"
                    )
                    if error:
                        error = f"{error}; cleanup uninstall failed (see {cleanup_log})"
                    else:
                        error = f"cleanup uninstall failed (see {cleanup_log})"

        # Postflight hygiene check when we expect a clean machine.
        if not keep_temp:
            try:
                post = test_hygiene.collect_state(
                    label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
                    ps_regex=test_hygiene.DEFAULT_PS_REGEX,
                )
                if not post.is_clean():
                    postflight_clean = False
                    detail = test_hygiene.format_state(
                        post,
                        label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
                        ps_regex=test_hygiene.DEFAULT_PS_REGEX,
                    )
                    if error:
                        error = f"{error}; postflight not clean"
                    else:
                        error = "postflight not clean"
                    print("e2e-setup-smoke: POSTCHECK FAIL (orphaned services/processes remain)")
                    print(detail)
            except Exception as exc:
                postflight_clean = False
                if error:
                    error = f"{error}; postflight check failed ({exc})"
                else:
                    error = f"postflight check failed ({exc})"

        final_success = success and (keep_temp or postflight_clean)

        try:
            write_last_state(
                artifacts_dir / "last_state.json",
                temp_root=temp_root,
                install_root=install_root,
                state_dir=state_dir,
                config_path=config_path,
                preserved=keep_temp or not final_success,
                extra={
                    "timestamp_utc": datetime.now(timezone.utc).isoformat(),
                    "success": final_success,
                    "postflight_clean": postflight_clean,
                    "quarantine_installer_dmg": quarantined_installer_dmg,
                },
            )
        except Exception:
            pass

        if final_success and not keep_temp:
            shutil.rmtree(temp_root, ignore_errors=True)
        else:
            print(f"Temp root preserved at {temp_root}")

        success = final_success

    if success:
        print("e2e-setup-smoke: PASS")
        return 0

    print(f"e2e-setup-smoke: FAIL ({error})")
    if artifacts_dir.exists():
        print(f"Artifacts: {artifacts_dir}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
