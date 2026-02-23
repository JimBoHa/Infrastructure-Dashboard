#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import Request, urlopen

try:
    import test_hygiene
except Exception:  # pragma: no cover
    test_hygiene = None

REPO_ROOT = Path(__file__).resolve().parents[1]
REPORTS_DIR = REPO_ROOT / "reports" / "e2e-web-smoke"
SIM_LAB_RUNNER = REPO_ROOT / "tools" / "sim_lab" / "run.py"
SIM_LAB_STORAGE = REPO_ROOT / "storage" / "sim_lab"
SIM_LAB_HTTP_SERVER = REPO_ROOT / "tools" / "sim_lab" / "http_json_server.py"
SIM_LAB_FIXTURES = REPO_ROOT / "tools" / "sim_lab" / "fixtures"
SIM_LAB_MOSQUITTO_CONF = REPO_ROOT / "tools" / "sim_lab" / "mosquitto.conf"
NODE_AGENT_DIR = REPO_ROOT / "apps" / "node-agent"
LAST_SETUP_STATE = REPO_ROOT / "reports" / "e2e-setup-smoke" / "last_state.json"

DEFAULT_SETUP_CONFIG = Path("/Users/Shared/FarmDashboard/setup/config.json")
DEFAULT_REDIS_PORT = 6379
DEFAULT_CONTROL_PORT = 8100
DEFAULT_SETUP_DAEMON_PORT = 8800

DEFAULT_API_BASE = "http://127.0.0.1:8000"
DEFAULT_WEB_BASE = "http://127.0.0.1:3005"
FORECAST_URL = os.environ.get("FARM_SIM_LAB_FORECAST_URL", "http://127.0.0.1:9103/forecast.json")
RATES_URL = os.environ.get("FARM_SIM_LAB_RATES_URL", "http://127.0.0.1:9104/rates.json")
ADVERTISE_IP = os.environ.get("FARM_SIM_LAB_ADVERTISE_IP", "127.0.0.1")
CONTROL_PORT = int(os.environ.get("FARM_SIM_LAB_CONTROL_PORT", DEFAULT_CONTROL_PORT))

ARTIFACTS_ENV = "FARM_SIM_LAB_ARTIFACTS_DIR"


def timestamp_slug() -> str:
    return time.strftime("%Y-%m-%dT%H-%M-%SZ", time.gmtime())


def collect_hygiene_state() -> tuple[bool, str]:
    if test_hygiene is None:
        return True, ""
    state = test_hygiene.collect_state()
    formatted = test_hygiene.format_state(
        state,
        label_substring=test_hygiene.DEFAULT_LAUNCHD_LABEL_SUBSTRING,
        ps_regex=test_hygiene.DEFAULT_PS_REGEX,
    )
    return state.is_clean(), formatted


def write_hygiene_report(artifacts_dir: Path, *, phase: str, body: str) -> None:
    try:
        report_path = artifacts_dir / f"hygiene-{phase}.txt"
        report_path.write_text(body)
    except Exception:
        return


def enforce_clean_state(artifacts_dir: Path, *, phase: str) -> bool:
    ok, formatted = collect_hygiene_state()
    if ok:
        return True

    if formatted:
        write_hygiene_report(artifacts_dir, phase=phase, body=formatted)

    print(
        f"e2e-web-smoke: {phase} hygiene check failed (orphaned services/processes detected).",
        file=sys.stderr,
    )
    if formatted:
        print(formatted, file=sys.stderr)
    print("Tip: run `make test-clean` and retry.", file=sys.stderr)
    return False


def apply_hygiene_cleanup(artifacts_dir: Path, *, phase: str) -> None:
    if test_hygiene is None:
        return

    state = test_hygiene.collect_state()
    if state.dmg_images:
        test_hygiene.detach_dmg_images(state.dmg_images)
    if state.launchd_jobs:
        test_hygiene.remove_launchd_jobs(state.launchd_jobs)
    if state.processes:
        test_hygiene.terminate_processes(state.processes, timeout_seconds=10.0)

    ok, formatted = collect_hygiene_state()
    if formatted:
        write_hygiene_report(artifacts_dir, phase=phase, body=formatted)
    if not ok:
        print(
            f"e2e-web-smoke: cleanup attempted but machine is still not clean ({phase}).",
            file=sys.stderr,
        )


def resolve_artifacts_dir() -> Path:
    env_path = os.environ.get(ARTIFACTS_ENV)
    if env_path:
        return Path(env_path)
    return REPORTS_DIR / timestamp_slug()


def parse_port(url: str) -> int:
    parsed = urlparse(url)
    if parsed.port:
        return parsed.port
    if parsed.scheme == "https":
        return 443
    return 80


def parse_host_port(url: str, default_port: int) -> tuple[str, int]:
    parsed = urlparse(url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or default_port
    return host, port


def resolve_setup_config_path() -> Path:
    env_path = os.environ.get("FARM_SETUP_CONFIG")
    if env_path:
        return Path(env_path)
    state_dir = os.environ.get("FARM_SETUP_STATE_DIR")
    if state_dir:
        candidate = Path(state_dir) / "config.json"
        if candidate.exists():
            return candidate
    last_state = resolve_last_state_config()
    if last_state:
        return last_state
    return DEFAULT_SETUP_CONFIG


def resolve_last_state_config() -> Path | None:
    if not LAST_SETUP_STATE.exists():
        return None
    try:
        payload = json.loads(LAST_SETUP_STATE.read_text())
    except json.JSONDecodeError:
        return None
    if not payload.get("preserved"):
        return None
    config_path = payload.get("config_path")
    if not isinstance(config_path, str) or not config_path:
        return None
    candidate = Path(config_path)
    if candidate.exists():
        return candidate
    return None


def load_setup_config() -> tuple[dict, Path]:
    config_path = resolve_setup_config_path()
    if not config_path.exists():
        return {}, config_path
    try:
        return json.loads(config_path.read_text()), config_path
    except json.JSONDecodeError:
        return {}, config_path


def resolve_api_base(config: dict) -> str:
    env_value = os.environ.get("FARM_SIM_LAB_API_BASE")
    if env_value:
        return env_value
    port = config.get("core_port")
    if isinstance(port, int) and port > 0:
        return f"http://127.0.0.1:{port}"
    return DEFAULT_API_BASE


def resolve_web_base(config: dict) -> str:
    env_value = os.environ.get("FARM_SIM_LAB_BASE_URL")
    if env_value:
        return env_value
    port = config.get("core_port")
    if isinstance(port, int) and port > 0:
        return f"http://127.0.0.1:{port}"
    return DEFAULT_WEB_BASE


def resolve_setup_daemon_base(config: dict) -> str:
    value = config.get("setup_port")
    port = value if isinstance(value, int) and value > 0 else DEFAULT_SETUP_DAEMON_PORT
    return f"http://127.0.0.1:{port}"


def maybe_bootout_e2e_app_services(config: dict, config_path: Path) -> None:
    if sys.platform != "darwin":
        return
    profile = str(config.get("profile", "")).strip().lower()
    if profile != "e2e":
        return
    prefix = config.get("launchd_label_prefix")
    if not isinstance(prefix, str) or not prefix.strip():
        return
    if os.environ.get("FARM_E2E_BOOTOUT_APPS", "").strip().lower() in {"0", "false", "no"}:
        return

    uid = os.getuid()
    target = f"gui/{uid}"
    state_dir = Path(os.environ.get("FARM_SETUP_STATE_DIR", str(config_path.parent)))
    launchd_dir = state_dir / "launchd"
    prefix = prefix.rstrip(".")
    for suffix in ["core-server", "telemetry-sidecar"]:
        label = f"{prefix}.{suffix}"
        plist_path = launchd_dir / f"{label}.plist"
        if not plist_path.exists():
            continue
        subprocess.run(
            ["launchctl", "bootout", target, str(plist_path)],
            capture_output=True,
            text=True,
        )


def resolve_install_root(config: dict) -> Path:
    env_root = os.environ.get("FARM_SETUP_INSTALL_ROOT")
    if env_root:
        return Path(env_root)
    install_root = config.get("install_root")
    if isinstance(install_root, str) and install_root:
        return Path(install_root)
    return Path("/usr/local/farm-dashboard")


def resolve_data_root(config: dict) -> Path:
    data_root = config.get("data_root")
    if isinstance(data_root, str) and data_root:
        return Path(data_root)
    return resolve_install_root(config) / "data"


def resolve_logs_root(config: dict) -> Path:
    logs_root = config.get("logs_root")
    if isinstance(logs_root, str) and logs_root:
        return Path(logs_root)
    return resolve_install_root(config) / "logs"


def resolve_backup_root(config: dict) -> Path:
    backup_root = config.get("backup_root")
    if isinstance(backup_root, str) and backup_root:
        return Path(backup_root)
    return resolve_data_root(config) / "storage" / "backups"


def resolve_service_root(config: dict) -> Path:
    return resolve_data_root(config) / "services"


def resolve_mqtt_url(config: dict) -> str:
    env_url = os.environ.get("FARM_SIM_LAB_MQTT_URL")
    if env_url:
        return env_url
    host = config.get("mqtt_host")
    port = config.get("mqtt_port")
    if isinstance(host, str) and host and isinstance(port, int):
        return f"mqtt://{host}:{port}"
    return "mqtt://127.0.0.1:1883"


def resolve_database_url(config: dict) -> str:
    # Prefer installer-generated config for determinism. Use an explicit E2E override
    # knob instead of inheriting CORE_DATABASE_URL from the caller's shell, which can
    # accidentally point at unrelated local dev databases (and even different drivers).
    override = os.environ.get("FARM_E2E_DATABASE_URL") or os.environ.get(
        "FARM_SIM_LAB_DATABASE_URL"
    )
    if override:
        return override
    db_url = config.get("database_url")
    if isinstance(db_url, str) and db_url:
        return db_url
    env_url = os.environ.get("CORE_DATABASE_URL") or os.environ.get("DATABASE_URL")
    if env_url:
        return env_url
    raise RuntimeError(
        "Database URL not configured. Run installer-path setup (so config.json is present) or set CORE_DATABASE_URL."
    )


def parse_database_host_port(db_url: str) -> tuple[str, int] | None:
    normalized = db_url.replace("postgresql+psycopg", "postgresql")
    parsed = urlparse(normalized)
    if not parsed.hostname:
        return None
    port = parsed.port or 5432
    return parsed.hostname, port


def parse_database_credentials(db_url: str) -> tuple[str, str, str]:
    normalized = db_url.replace("postgresql+psycopg", "postgresql")
    parsed = urlparse(normalized)
    user = parsed.username or "postgres"
    password = parsed.password
    if not password:
        raise RuntimeError("Database URL is missing a password; re-run installer or set CORE_DATABASE_URL")
    db_name = (parsed.path or "").lstrip("/") or "iot"
    return user, password, db_name


def resolve_redis_port(config: dict) -> int:
    value = config.get("redis_port")
    if isinstance(value, int) and value > 0:
        return value
    return DEFAULT_REDIS_PORT


def ensure_local_port(host: str, port: int, label: str) -> None:
    if not is_local_host(host):
        return
    if port_open(host, port):
        return
    raise RuntimeError(
        f"{label} not reachable at {host}:{port}. "
        "Start native services via farmctl/launchd or point to a reachable host."
    )


def port_open(host: str, port: int) -> bool:
    try:
        with socket.create_connection((host, port), timeout=1):
            return True
    except OSError:
        return False


def allow_port_collisions() -> bool:
    return os.environ.get("FARM_E2E_ALLOW_PORT_COLLISION", "").strip().lower() in {
        "1",
        "true",
        "yes",
    }


def is_local_host(host: str) -> bool:
    return host in {"127.0.0.1", "localhost", "0.0.0.0"}


def wait_for_port_closed(host: str, port: int, timeout_seconds: int) -> None:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if not port_open(host, port):
            return
        time.sleep(0.25)


def fail_fast_if_port_in_use(host: str, port: int, label: str) -> None:
    if allow_port_collisions():
        return
    if not is_local_host(host):
        return
    if not port_open(host, port):
        return
    wait_for_port_closed(host, port, timeout_seconds=5)
    if port_open(host, port):
        raise RuntimeError(
            f"Port collision: {label} port {host}:{port} is already in use. "
            "Stop the process using it or set FARM_E2E_ALLOW_PORT_COLLISION=1 for debugging."
        )


def postgres_data_dir(config: dict) -> Path:
    return resolve_service_root(config) / "postgres"


def redis_data_dir(config: dict) -> Path:
    return resolve_service_root(config) / "redis"


def mosquitto_data_dir(config: dict) -> Path:
    return resolve_service_root(config) / "mosquitto" / "data"


def find_postgres_binary(config: dict) -> Path | None:
    candidate = resolve_install_root(config) / "native/postgres/bin/postgres"
    if candidate.exists():
        return candidate
    return None


def find_initdb_binary(config: dict) -> Path | None:
    candidate = resolve_install_root(config) / "native/postgres/bin/initdb"
    if candidate.exists():
        return candidate
    return None


def find_psql_binary(config: dict) -> Path | None:
    candidate = resolve_install_root(config) / "native/postgres/bin/psql"
    if candidate.exists():
        return candidate
    which_path = shutil.which("psql")
    if which_path:
        return Path(which_path)
    return None


def find_redis_binary(config: dict) -> Path | None:
    candidate = resolve_install_root(config) / "native/redis/bin/redis-server"
    if candidate.exists():
        return candidate
    which_path = shutil.which("redis-server")
    if which_path:
        return Path(which_path)
    return None


def find_mosquitto_binary(config: dict) -> Path | None:
    env_bin = os.environ.get("FARM_MOSQUITTO_BIN") or os.environ.get("MOSQUITTO_BIN")
    if env_bin:
        candidate = Path(env_bin)
        if candidate.exists():
            return candidate
    install_root = resolve_install_root(config)
    candidates = [
        install_root / "native/mosquitto/bin/mosquitto",
        install_root / "native/mosquitto/sbin/mosquitto",
    ]
    which_path = shutil.which("mosquitto")
    if which_path:
        candidates.append(Path(which_path))
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return None


def mosquitto_config_path(port: int) -> Path:
    if port == 1883 and SIM_LAB_MOSQUITTO_CONF.exists():
        return SIM_LAB_MOSQUITTO_CONF
    SIM_LAB_STORAGE.mkdir(parents=True, exist_ok=True)
    config_path = SIM_LAB_STORAGE / f"mosquitto_{port}.conf"
    config_path.write_text(
        f"listener {port} 0.0.0.0\nallow_anonymous true\npersistence false\n"
    )
    return config_path


def ensure_timescaledb_preload(data_dir: Path) -> None:
    config_path = data_dir / "postgresql.conf"
    if not config_path.exists():
        return
    lines = config_path.read_text().splitlines()
    updated: list[str] = []
    preload_found = False
    telemetry_found = False
    for line in lines:
        trimmed = line.lstrip()
        if trimmed.startswith("shared_preload_libraries"):
            preload_found = True
            if "timescaledb" in trimmed:
                updated.append(line)
            else:
                updated.append("shared_preload_libraries = 'timescaledb'")
            continue
        if trimmed.startswith("timescaledb.telemetry_level"):
            telemetry_found = True
            if "off" in trimmed:
                updated.append(line)
            else:
                updated.append("timescaledb.telemetry_level = 'off'")
            continue
        updated.append(line)
    if not preload_found:
        updated.append("shared_preload_libraries = 'timescaledb'")
    if not telemetry_found:
        updated.append("timescaledb.telemetry_level = 'off'")
    config_path.write_text("\n".join(updated) + "\n")


def ensure_postgres_initialized(data_dir: Path, initdb_bin: Path, *, user: str, password: str) -> None:
    if (data_dir / "PG_VERSION").exists():
        ensure_timescaledb_preload(data_dir)
        return
    data_dir.mkdir(parents=True, exist_ok=True)
    pwfile = tempfile.NamedTemporaryFile("w", delete=False)
    try:
        pwfile.write(password + "\n")
        pwfile.flush()
        pwfile.close()
        subprocess.run(
            [
                str(initdb_bin),
                "-D",
                str(data_dir),
                f"--username={user}",
                "--pwfile",
                pwfile.name,
                "--auth=md5",
            ],
            check=True,
        )
    finally:
        try:
            os.unlink(pwfile.name)
        except FileNotFoundError:
            pass
    ensure_timescaledb_preload(data_dir)


def ensure_database_exists(
    psql_bin: Path,
    *,
    host: str,
    port: int,
    user: str,
    password: str,
    db_name: str,
) -> None:
    env = os.environ.copy()
    if password:
        env["PGPASSWORD"] = password
    check = subprocess.run(
        [
            str(psql_bin),
            "-h",
            host,
            "-p",
            str(port),
            "-U",
            user,
            "-d",
            "postgres",
            "-tAc",
            f"SELECT 1 FROM pg_database WHERE datname='{db_name}'",
        ],
        capture_output=True,
        text=True,
        env=env,
    )
    if check.returncode != 0:
        raise RuntimeError(
            f"psql failed while checking database: {check.stderr.strip() or check.stdout.strip()}"
        )
    if check.stdout.strip() == "1":
        return
    create = subprocess.run(
        [
            str(psql_bin),
            "-h",
            host,
            "-p",
            str(port),
            "-U",
            user,
            "-d",
            "postgres",
            "-c",
            f'CREATE DATABASE "{db_name}"',
        ],
        capture_output=True,
        text=True,
        env=env,
    )
    if create.returncode != 0:
        raise RuntimeError(
            f"psql failed while creating database: {create.stderr.strip() or create.stdout.strip()}"
        )


def ensure_redis_config(config: dict, *, port: int) -> Path:
    redis_root = redis_data_dir(config)
    redis_root.mkdir(parents=True, exist_ok=True)
    config_path = redis_root / "redis.conf"
    if not config_path.exists():
        config_path.write_text(
            f"bind 127.0.0.1\nport {port}\ndir {redis_root}\nprotected-mode yes\n"
        )
    return config_path


def ensure_mosquitto_config(config: dict, *, port: int) -> Path:
    mosquitto_root = resolve_service_root(config) / "mosquitto"
    mosquitto_root.mkdir(parents=True, exist_ok=True)
    data_dir = mosquitto_data_dir(config)
    data_dir.mkdir(parents=True, exist_ok=True)
    logs_root = resolve_logs_root(config)
    logs_root.mkdir(parents=True, exist_ok=True)
    config_path = mosquitto_root / "mosquitto.conf"
    if not config_path.exists():
        config_path.write_text(
            "listener {port} 127.0.0.1\n"
            "persistence true\n"
            "persistence_location {data_dir}\n"
            "log_dest file {log_path}\n"
            "allow_anonymous true\n".format(
                port=port,
                data_dir=data_dir,
                log_path=logs_root / "mosquitto.log",
            )
        )
    return config_path


def maybe_start_postgres(config: dict, *, database_url: str) -> subprocess.Popen | None:
    host_port = parse_database_host_port(database_url)
    if not host_port:
        return None
    host, port = host_port
    if not is_local_host(host) or port_open(host, port):
        return None
    postgres_bin = find_postgres_binary(config)
    initdb_bin = find_initdb_binary(config)
    psql_bin = find_psql_binary(config)
    if not postgres_bin or not initdb_bin or not psql_bin:
        raise RuntimeError("Postgres binaries not found under the install root.")
    user, password, db_name = parse_database_credentials(database_url)
    data_dir = postgres_data_dir(config)
    ensure_postgres_initialized(data_dir, initdb_bin, user=user, password=password)
    proc = spawn_process(
        [
            str(postgres_bin),
            "-D",
            str(data_dir),
            "-p",
            str(port),
            "-h",
            "127.0.0.1",
            "-c",
            "shared_preload_libraries=timescaledb",
            "-c",
            "timescaledb.telemetry_level=off",
        ],
        env=os.environ.copy(),
        label="postgres",
    )
    wait_for_port(host, port, 30)
    ensure_database_exists(
        psql_bin,
        host=host,
        port=port,
        user=user,
        password=password,
        db_name=db_name,
    )
    return proc


def maybe_start_redis(config: dict, *, port: int) -> subprocess.Popen | None:
    if port_open("127.0.0.1", port):
        return None
    redis_bin = find_redis_binary(config)
    if not redis_bin:
        raise RuntimeError("Redis binary not found under the install root.")
    config_path = ensure_redis_config(config, port=port)
    proc = spawn_process([str(redis_bin), str(config_path)], env=os.environ.copy(), label="redis")
    wait_for_port("127.0.0.1", port, 20)
    return proc


def spawn_process(cmd: list[str], *, env: dict[str, str], label: str) -> subprocess.Popen:
    verbose = os.environ.get("FARM_E2E_VERBOSE", "").strip().lower() in {"1", "true", "yes"}
    artifacts_dir = os.environ.get(ARTIFACTS_ENV, "").strip()
    if artifacts_dir and not verbose:
        log_path = Path(artifacts_dir) / f"{label}.log"
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with log_path.open("a", encoding="utf-8") as log:
            log.write(f"$ {' '.join(cmd)}\n")
            log.flush()
            return subprocess.Popen(
                cmd,
                env=env,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )

    stdout = None if verbose else subprocess.DEVNULL
    stderr = None if verbose else subprocess.DEVNULL
    return subprocess.Popen(
        cmd,
        env=env,
        stdout=stdout,
        stderr=stderr,
        start_new_session=True,
    )


def spawn_process_group(
    cmd: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    label: str,
) -> subprocess.Popen:
    verbose = os.environ.get("FARM_E2E_VERBOSE", "").strip().lower() in {"1", "true", "yes"}
    artifacts_dir = os.environ.get(ARTIFACTS_ENV, "").strip()
    if artifacts_dir and not verbose:
        log_path = Path(artifacts_dir) / f"{label}.log"
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with log_path.open("a", encoding="utf-8") as log:
            log.write(f"$ {' '.join(cmd)}\n")
            log.flush()
            return subprocess.Popen(
                cmd,
                cwd=str(cwd),
                env=env,
                stdout=log,
                stderr=log,
                start_new_session=True,
            )

    stdout = None if verbose else subprocess.DEVNULL
    stderr = None if verbose else subprocess.DEVNULL
    return subprocess.Popen(
        cmd,
        cwd=str(cwd),
        env=env,
        stdout=stdout,
        stderr=stderr,
        start_new_session=True,
    )


def stop_process_group(proc: subprocess.Popen, *, timeout: float) -> None:
    if proc.poll() is not None:
        return
    if sys.platform != "win32":
        try:
            os.killpg(proc.pid, signal.SIGINT)
        except ProcessLookupError:
            return
    else:
        proc.send_signal(signal.SIGINT)
    try:
        proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        if sys.platform != "win32":
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                return
        else:
            proc.kill()
        proc.wait(timeout=5)


def resolve_installed_bins(config: dict) -> dict[str, Path] | None:
    install_root = resolve_install_root(config)
    bin_dir = install_root / "bin"
    core_bin = bin_dir / "core-server"
    sidecar_bin = bin_dir / "telemetry-sidecar"
    if core_bin.exists() and sidecar_bin.exists():
        return {"core": core_bin, "sidecar": sidecar_bin}
    return None


def require_installed() -> bool:
    return os.environ.get("FARM_E2E_REQUIRE_INSTALLED", "").strip().lower() in {
        "1",
        "true",
        "yes",
    }


def should_use_installed(config: dict) -> bool:
    if require_installed():
        return True
    override = os.environ.get("FARM_E2E_USE_INSTALLED")
    if override:
        return override.strip().lower() in {"1", "true", "yes"}
    return resolve_installed_bins(config) is not None


def build_core_env(
    base_env: dict[str, str],
    *,
    database_url: str,
    mqtt_host: str,
    mqtt_port: int,
    node_agent_port: int,
    forecast_url: str,
    rates_url: str,
) -> dict[str, str]:
    env = base_env.copy()
    env.setdefault("CORE_DEBUG", "false")
    env.update(
        {
            "CORE_DATABASE_URL": database_url,
            "CORE_MQTT_HOST": mqtt_host,
            "CORE_MQTT_PORT": str(mqtt_port),
            "CORE_DEMO_MODE": "false",
            "CORE_ALLOW_BOOTSTRAP_USER_CREATE": "true",
            "CORE_ENABLE_ANALYTICS_FEEDS": "true",
            "CORE_ENABLE_FORECAST_INGESTION": "true",
            "CORE_ENABLE_INDICATOR_GENERATION": "true",
            "CORE_MQTT_INGEST_ENABLED": "false",
            "CORE_ANALYTICS_FEED_POLL_INTERVAL_SECONDS": "60",
            "CORE_INDICATOR_POLL_INTERVAL_SECONDS": "60",
            "CORE_FORECAST_POLL_INTERVAL_SECONDS": "30",
            "CORE_SCHEDULE_POLL_INTERVAL_SECONDS": "15",
            "CORE_NODE_AGENT_PORT": str(node_agent_port),
        }
    )
    if forecast_url:
        parsed = urlparse(forecast_url)
        env.update(
            {
                "CORE_FORECAST_PROVIDER": "http",
                "CORE_FORECAST_API_BASE_URL": f"{parsed.scheme}://{parsed.netloc}",
                "CORE_FORECAST_API_PATH": parsed.path or "/",
            }
        )
    if rates_url:
        parsed = urlparse(rates_url)
        env.update(
            {
                "CORE_ANALYTICS_RATES__PROVIDER": "http",
                "CORE_ANALYTICS_RATES__API_BASE_URL": f"{parsed.scheme}://{parsed.netloc}",
                "CORE_ANALYTICS_RATES__API_PATH": parsed.path or "/",
            }
        )
    return env


def build_sidecar_env(
    base_env: dict[str, str],
    *,
    database_url: str,
    mqtt_host: str,
    mqtt_port: int,
    config: dict,
) -> dict[str, str]:
    env = base_env.copy()
    env.update(
        {
            "SIDECAR_DATABASE_URL": database_url,
            "SIDECAR_MQTT_HOST": mqtt_host,
            "SIDECAR_MQTT_PORT": str(mqtt_port),
        }
    )
    if config.get("mqtt_username"):
        env["SIDECAR_MQTT_USERNAME"] = str(config["mqtt_username"])
    if config.get("mqtt_password"):
        env["SIDECAR_MQTT_PASSWORD"] = str(config["mqtt_password"])
    return env


def start_installed_services(
    bins: dict[str, Path],
    *,
    base_env: dict[str, str],
    database_url: str,
    mqtt_host: str,
    mqtt_port: int,
    api_port: int,
    node_agent_port: int,
    forecast_url: str,
    rates_url: str,
    config: dict,
    control_port: int,
) -> list[tuple[str, subprocess.Popen]]:
    processes: list[tuple[str, subprocess.Popen]] = []
    setup_daemon_base = resolve_setup_daemon_base(config)
    install_root = resolve_install_root(config)
    core_env = build_core_env(
        base_env,
        database_url=database_url,
        mqtt_host=mqtt_host,
        mqtt_port=mqtt_port,
        node_agent_port=node_agent_port,
        forecast_url=forecast_url,
        rates_url=rates_url,
    )
    core_env["CORE_STATIC_ROOT"] = str(install_root / "static" / "dashboard-web")
    core_env["CORE_SETUP_DAEMON_BASE_URL"] = setup_daemon_base
    core_proc = spawn_process(
        [str(bins["core"]), "--host", "0.0.0.0", "--port", str(api_port)],
        env=core_env,
        label="core-server",
    )
    processes.append(("core-server", core_proc))

    sidecar_env = build_sidecar_env(
        base_env,
        database_url=database_url,
        mqtt_host=mqtt_host,
        mqtt_port=mqtt_port,
        config=config,
    )
    sidecar_proc = spawn_process(
        [str(bins["sidecar"])],
        env=sidecar_env,
        label="telemetry-sidecar",
    )
    processes.append(("telemetry-sidecar", sidecar_proc))
    return processes


def wait_for_http(url: str, timeout_seconds: int) -> None:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        try:
            request = Request(url, method="GET")
            with urlopen(request, timeout=3) as response:
                if response.status < 500:
                    return
        except Exception:
            time.sleep(0.5)
    raise RuntimeError(f"Timed out waiting for {url}")


def wait_for_port(host: str, port: int, timeout_seconds: int) -> None:
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=2):
                return
        except OSError:
            time.sleep(0.5)
    raise RuntimeError(f"Timed out waiting for {host}:{port}")


def run(cmd: list[str], *, env: dict[str, str] | None = None, label: str = "command") -> None:
    verbose = os.environ.get("FARM_E2E_VERBOSE", "").strip().lower() in {"1", "true", "yes"}
    artifacts_dir = os.environ.get(ARTIFACTS_ENV, "").strip()
    if artifacts_dir and not verbose:
        log_path = Path(artifacts_dir) / f"{label}.log"
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with log_path.open("a", encoding="utf-8") as log:
            log.write(f"$ {' '.join(cmd)}\n")
            log.flush()
            result = subprocess.run(
                cmd,
                env=env,
                stdout=log,
                stderr=log,
                text=True,
            )
        if result.returncode != 0:
            raise RuntimeError(f"{label} failed (see {log_path})")
        return

    print(f"+ {' '.join(cmd)}")
    subprocess.run(cmd, check=True, env=env)


def copy_installed_logs(config: dict, *, artifacts_dir: Path) -> None:
    logs_root = resolve_logs_root(config)
    if not logs_root.exists():
        return
    dest = artifacts_dir / "service-logs"
    dest.mkdir(parents=True, exist_ok=True)
    for path in sorted(logs_root.glob("*.log")):
        if not path.is_file():
            continue
        try:
            shutil.copy2(path, dest / path.name)
        except Exception:
            continue


def start_sim_lab_mocks(
    mqtt_url: str, config: dict, *, use_installed: bool
) -> list[tuple[str, subprocess.Popen]]:
    processes: list[tuple[str, subprocess.Popen]] = []
    mqtt_host, mqtt_port = parse_host_port(mqtt_url, 1883)
    if not port_open(mqtt_host, mqtt_port):
        if not is_local_host(mqtt_host):
            raise RuntimeError(
                f"MQTT broker not reachable at {mqtt_host}:{mqtt_port}. "
                "Start the broker or set FARM_SIM_LAB_MQTT_URL."
            )
        mosquitto_bin = find_mosquitto_binary(config)
        if not mosquitto_bin:
            raise RuntimeError(
                "MQTT broker not running and mosquitto binary not found. "
                "Start the native services or set FARM_MOSQUITTO_BIN."
            )
        if use_installed:
            conf_path = ensure_mosquitto_config(config, port=mqtt_port)
        else:
            conf_path = mosquitto_config_path(mqtt_port)
        proc = spawn_process(
            [str(mosquitto_bin), "-c", str(conf_path)],
            env=os.environ.copy(),
            label="mosquitto",
        )
        processes.append(("mosquitto", proc))

    fixtures = [
        ("ble-mock", 9101, SIM_LAB_FIXTURES / "ble_advertiser.json"),
        ("mesh-mock", 9102, SIM_LAB_FIXTURES / "mesh_coordinator.json"),
        ("forecast-fixture", 9103, SIM_LAB_FIXTURES / "forecast.json"),
        ("rates-fixture", 9104, SIM_LAB_FIXTURES / "rates.json"),
    ]
    for name, port, fixture in fixtures:
        if port_open("127.0.0.1", port):
            continue
        env = os.environ.copy()
        env["PORT"] = str(port)
        env["FIXTURE_FILE"] = str(fixture)
        proc = spawn_process(
            [sys.executable, str(SIM_LAB_HTTP_SERVER)],
            env=env,
            label=name,
        )
        processes.append((name, proc))
    return processes


def stop_sim_lab_mocks(processes: list[tuple[str, subprocess.Popen]]) -> None:
    for name, proc in reversed(processes):
        if proc.poll() is not None:
            continue
        proc.send_signal(signal.SIGINT)
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)


def stop_processes(processes: list[tuple[str, subprocess.Popen]]) -> None:
    for name, proc in reversed(processes):
        if proc.poll() is not None:
            continue
        if sys.platform != "win32":
            try:
                os.killpg(proc.pid, signal.SIGINT)
            except ProcessLookupError:
                continue
        else:
            proc.send_signal(signal.SIGINT)
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            if sys.platform != "win32":
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    continue
            else:
                proc.kill()
            proc.wait(timeout=5)


def cleanup_sim_lab_storage() -> None:
    if not SIM_LAB_STORAGE.exists():
        return
    for path in SIM_LAB_STORAGE.glob("*.json"):
        if not path.is_file():
            continue
        try:
            path.unlink()
        except Exception as exc:
            print(f"e2e-web-smoke: failed to remove {path}: {exc}", file=sys.stderr)


def main() -> int:
    artifacts_dir = resolve_artifacts_dir()
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    os.environ[ARTIFACTS_ENV] = str(artifacts_dir)

    # Tier-A validation (FARM_E2E_REQUIRE_INSTALLED=1) tests against the *already running*
    # installed controller. Skip preflight hygiene check since installed services are expected.
    # Tier-B (default) requires a clean machine before starting.
    if not require_installed() and not enforce_clean_state(artifacts_dir, phase="preflight"):
        return 2

    config, config_path = load_setup_config()
    api_base = resolve_api_base(config)
    web_base = resolve_web_base(config)
    mqtt_url = resolve_mqtt_url(config)
    database_url = resolve_database_url(config)
    redis_port = resolve_redis_port(config)
    api_port = parse_port(api_base)
    web_port = parse_port(web_base)
    mqtt_host, mqtt_port = parse_host_port(mqtt_url, 1883)
    db_host_port = parse_database_host_port(database_url)
    use_installed = should_use_installed(config)
    installed_bins = resolve_installed_bins(config)
    if use_installed and not installed_bins:
        raise RuntimeError(
            "Installed bundle not found. Run make e2e-setup-smoke with FARM_E2E_KEEP_TEMP=1 "
            "and rerun, or set FARM_SETUP_STATE_DIR to the temp install state."
        )

    native_processes: list[tuple[str, subprocess.Popen]] = []
    # Tier-A (require_installed): Use *already running* launchd-managed services directly.
    # Tier-B (use_installed): Start new instances using installed binaries.
    tier_a_mode = require_installed()
    if use_installed and not tier_a_mode:
        maybe_bootout_e2e_app_services(config, config_path)
        fail_fast_if_port_in_use("127.0.0.1", api_port, "core-server")
        postgres_proc = maybe_start_postgres(config, database_url=database_url)
        if postgres_proc:
            native_processes.append(("postgres", postgres_proc))
        redis_proc = maybe_start_redis(config, port=redis_port)
        if redis_proc:
            native_processes.append(("redis", redis_proc))

    if db_host_port:
        ensure_local_port(db_host_port[0], db_host_port[1], "Postgres")
    ensure_local_port("127.0.0.1", redis_port, "Redis")

    env = os.environ.copy()
    if tier_a_mode:
        env["FARM_E2E_REQUIRE_INSTALLED"] = "1"
    env["FARM_SIM_LAB_API_BASE"] = api_base
    env["FARM_SIM_LAB_BASE_URL"] = web_base
    env["FARM_SIM_LAB_FORECAST_URL"] = FORECAST_URL
    env["FARM_SIM_LAB_RATES_URL"] = RATES_URL
    env["FARM_SIM_LAB_MQTT_URL"] = mqtt_url
    env["SIM_LAB_NODE_AUTH_TOKEN"] = env.get("SIM_LAB_NODE_AUTH_TOKEN", "sim-lab-token")
    # Safety: Tier-A mode points at the installed controller which uses a real production database.
    # Do not propagate CORE_DATABASE_URL into the Sim Lab process unless we explicitly intend to run
    # DB-affecting operations (migrations/seed) against an E2E/local database.
    if not tier_a_mode:
        env["CORE_DATABASE_URL"] = database_url
    env["CORE_MQTT_HOST"] = mqtt_host
    env["CORE_MQTT_PORT"] = str(mqtt_port)
    backup_root = resolve_backup_root(config)
    backup_root.mkdir(parents=True, exist_ok=True)
    env["CORE_BACKUP_STORAGE_PATH"] = str(backup_root)
    if config.get("mqtt_username"):
        env["CORE_MQTT_USERNAME"] = str(config["mqtt_username"])
    if config.get("mqtt_password"):
        env["CORE_MQTT_PASSWORD"] = str(config["mqtt_password"])

    runner_cmd = [
        "poetry",
        "run",
        "python",
        str(SIM_LAB_RUNNER),
        "--core-port",
        str(api_port),
        "--web-port",
        str(web_port),
        "--control-port",
        str(CONTROL_PORT),
        "--mqtt-url",
        mqtt_url,
        "--advertise-ip",
        ADVERTISE_IP,
        "--node-port-base",
        "9200",
        "--forecast-fixture-url",
        FORECAST_URL,
        "--rates-fixture-url",
        RATES_URL,
    ]
    if tier_a_mode or use_installed:
        runner_cmd.extend(["--no-core", "--no-sidecar", "--no-web"])
        # Tier-A runs against the installed controller (real DB/state). Ensure Sim Lab never runs
        # migrations or demo seed against CORE_DATABASE_URL.
        if tier_a_mode:
            runner_cmd.extend(["--no-migrations", "--no-seed"])
    else:
        fail_fast_if_port_in_use("127.0.0.1", api_port, "core-server")

    sim_lab_proc = None
    installed_processes: list[tuple[str, subprocess.Popen]] = []
    mock_processes: list[tuple[str, subprocess.Popen]] = []
    error: Exception | None = None
    exit_code = 0
    try:
        mock_processes = start_sim_lab_mocks(mqtt_url, config, use_installed=use_installed)
        wait_for_port(mqtt_host, mqtt_port, 30)
        wait_for_http(FORECAST_URL, 30)
        wait_for_http(RATES_URL, 30)
        print(f"+ {' '.join(runner_cmd)}")
        sim_lab_proc = spawn_process_group(
            runner_cmd, cwd=NODE_AGENT_DIR, env=env, label="sim-lab"
        )
        wait_for_http(f"http://127.0.0.1:{CONTROL_PORT}/healthz", 120)
        if use_installed and installed_bins and not tier_a_mode:
            # Tier-B: Start new core-server/sidecar using installed binaries
            wait_for_port("127.0.0.1", CONTROL_PORT, 60)
            installed_processes = start_installed_services(
                installed_bins,
                base_env=env,
                database_url=database_url,
                mqtt_host=mqtt_host,
                mqtt_port=mqtt_port,
                api_port=api_port,
                node_agent_port=9200,
                forecast_url=FORECAST_URL,
                rates_url=RATES_URL,
                config=config,
                control_port=CONTROL_PORT,
            )
        # Tier-A: Services already running via launchd; just verify they're healthy
        wait_for_http(f"{api_base.rstrip('/')}/healthz", 120)
        wait_for_http(f"{web_base.rstrip('/')}/sim-lab", 120)

        smoke_cmd = [
            "node",
            "apps/dashboard-web/scripts/sim-lab-smoke.mjs",
            "--no-core",
            "--no-web",
            f"--api-base={api_base}",
            f"--base-url={web_base}",
        ]
        run(smoke_cmd, env=env, label="sim-lab-smoke")
        print("e2e-web-smoke: PASS")
        exit_code = 0
    except Exception as exc:
        error = exc
        print(f"e2e-web-smoke: FAIL ({exc})", file=sys.stderr)
        print(f"Artifacts: {artifacts_dir}", file=sys.stderr)
        exit_code = 1
    finally:
        if sim_lab_proc:
            stop_process_group(sim_lab_proc, timeout=30)
        try:
            stop_processes(installed_processes)
        except Exception as exc:
            print(f"e2e-web-smoke: failed to stop installed processes cleanly ({exc}).", file=sys.stderr)
        try:
            stop_processes(native_processes)
        except Exception as exc:
            print(f"e2e-web-smoke: failed to stop native services cleanly ({exc}).", file=sys.stderr)
        try:
            stop_sim_lab_mocks(mock_processes)
        except Exception as exc:
            print(f"e2e-web-smoke: failed to stop sim-lab mocks cleanly ({exc}).", file=sys.stderr)
        cleanup_sim_lab_storage()
        if error and use_installed:
            try:
                copy_installed_logs(config, artifacts_dir=artifacts_dir)
            except Exception:
                pass

        # Skip postflight hygiene cleanup/check in Tier-A mode - we don't own the installed services.
        if not require_installed():
            apply_hygiene_cleanup(artifacts_dir, phase="postflight")
            if not enforce_clean_state(artifacts_dir, phase="postflight"):
                exit_code = 1

    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
