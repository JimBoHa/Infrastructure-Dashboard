import os
import socket
import time
from pathlib import Path

import pytest

from app.utils.systemd import SystemdNotifier


def _prepare_socket(tmp_path: Path) -> tuple[socket.socket, Path]:
    path = Path("/tmp") / f"node-agent-notify-{os.getpid()}-{os.urandom(4).hex()}"
    try:
        path.unlink()
    except FileNotFoundError:
        pass
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    sock.bind(str(path))
    sock.settimeout(1)
    return sock, path


def test_ready_without_socket(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("NOTIFY_SOCKET", raising=False)
    notifier = SystemdNotifier()
    assert notifier.ready("no socket") is False


def test_ready_sends_payload(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    sock, socket_path = _prepare_socket(tmp_path)
    monkeypatch.setenv("NOTIFY_SOCKET", str(socket_path))
    notifier = SystemdNotifier()
    try:
        assert notifier.ready("Node agent ready")
        data = sock.recv(2048)
        decoded = data.decode("utf-8")
        assert "READY=1" in decoded
        assert "STATUS=Node agent ready" in decoded
    finally:
        sock.close()
        socket_path.unlink(missing_ok=True)


def test_watchdog_ping(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    sock, socket_path = _prepare_socket(tmp_path)
    monkeypatch.setenv("NOTIFY_SOCKET", str(socket_path))
    monkeypatch.setenv("WATCHDOG_USEC", str(int(0.5 * 1_000_000)))

    notifier = SystemdNotifier()
    try:
        assert notifier.start_watchdog() is True
        data = sock.recv(2048)
        assert data.decode("utf-8").strip() == "WATCHDOG=1"
    finally:
        notifier.stop_watchdog()
        sock.close()
        socket_path.unlink(missing_ok=True)


def test_watchdog_stops_cleanly(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    sock, socket_path = _prepare_socket(tmp_path)
    monkeypatch.setenv("NOTIFY_SOCKET", str(socket_path))
    monkeypatch.setenv("WATCHDOG_USEC", str(int(0.25 * 1_000_000)))

    notifier = SystemdNotifier()
    try:
        assert notifier.start_watchdog() is True
        # Consume the first ping emitted by the watchdog thread.
        sock.recv(2048)
        notifier.stop_watchdog()
        time.sleep(0.3)
        sock.settimeout(0.2)
        with pytest.raises(socket.timeout):
            sock.recv(2048)
    finally:
        sock.close()
        socket_path.unlink(missing_ok=True)
