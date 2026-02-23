#!/usr/bin/env python3
"""
Farm Dashboard test hygiene helpers.

Why this exists:
- E2E/installer tests (and failed runs) can leave behind launchd jobs and
  background processes that pollute the dev machine.
- Any subsequent smoke/E2E results are unreliable unless the machine is clean
  before the test starts and clean again after it finishes.

This tool checks for (and can optionally clean up) orphaned Farm services.

Equivalent manual checks:
  launchctl list 2>/dev/null | grep -i farm
  ps aux | grep -E "core-server|telemetry-sidecar|farm|mosquitto|qdrant|setup-daemon" | grep -v grep
  hdiutil info | rg -i FarmDashboard
  launchctl print-disabled gui/$(id -u) | rg com\\.farmdashboard\\.e2e
"""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import re
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Iterable


DEFAULT_LAUNCHD_LABEL_SUBSTRING = "farm"
DEFAULT_LAUNCHD_CLEAN_PREFIXES = ("com.farmdashboard.",)
# Persistent launchd enable/disable override records are state pollution and can make E2E/debugging
# confusing. Historical E2E runs (or accidental `launchctl enable`) can accumulate keys in:
#   /var/db/com.apple.xpc.launchd/disabled.<uid>.plist
# which requires an admin purge to remove. We treat any matching override keys as "NOT CLEAN" so
# test results are only trusted when the machine is fully clean.
DEFAULT_LAUNCHD_OVERRIDE_PREFIXES = ("com.farmdashboard.e2e.",)
# `hdiutil` can keep stale/failed attaches around even without a mountpoint. This pollutes the
# machine state and can cause later E2E runs to fail with `hdiutil: attach failed - Resource
# temporarily unavailable`.
DEFAULT_DMG_SUBSTRINGS = (
    "FarmDashboardInstaller",
    "FarmDashboardController",
    "farm_setup_dmg_",
    "farm_setup_e2e_",
)
# Intentionally avoid a bare "farm" substring match here: dev shells and CI wrappers
# often include the repo path (e.g. .../farm_dashboard/...), which would create
# false-positive "dirty" runs. Keep this focused on known service binaries and
# launchd labels.
DEFAULT_PS_REGEX = (
    r"(\bcore-server\b|\btelemetry-sidecar\b|\bmosquitto\b|\bqdrant\b|\bredis-server\b|\bpostgres\b"
    r"|com\.farmdashboard\.|farm_setup_e2e_|\bfarmctl\b.*\bserve\b)"
)


@dataclass(frozen=True)
class LaunchdJob:
    pid: int | None
    status: int | None
    label: str
    raw_line: str


@dataclass(frozen=True)
class ProcessInfo:
    pid: int
    command: str


@dataclass(frozen=True)
class DmgImage:
    image_path: str
    dev_entry: str | None
    mount_point: str | None


@dataclass(frozen=True)
class HygieneState:
    launchd_jobs: tuple[LaunchdJob, ...]
    processes: tuple[ProcessInfo, ...]
    launchd_override_keys: tuple[str, ...]
    dmg_images: tuple[DmgImage, ...]

    def is_clean(self) -> bool:
        return not self.launchd_jobs and not self.processes and not self.dmg_images


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True)


def run_bytes(cmd: list[str]) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(cmd, capture_output=True)


def parse_launchctl_list(output: str) -> list[LaunchdJob]:
    jobs: list[LaunchdJob] = []
    for line in (output or "").splitlines():
        raw = line.rstrip("\n")
        stripped = raw.strip()
        if not stripped:
            continue
        if stripped.lower().startswith("pid") and "label" in stripped.lower():
            continue
        parts = stripped.split()
        if len(parts) < 3:
            continue
        pid_field, status_field, label = parts[0], parts[1], parts[2]

        pid: int | None
        if pid_field == "-":
            pid = None
        else:
            try:
                pid = int(pid_field)
            except ValueError:
                pid = None

        status: int | None
        try:
            status = int(status_field)
        except ValueError:
            status = None

        jobs.append(LaunchdJob(pid=pid, status=status, label=label, raw_line=stripped))
    return jobs


def list_launchd_jobs(*, label_substring: str) -> list[LaunchdJob]:
    proc = run(["launchctl", "list"])
    if proc.returncode != 0:
        message = (proc.stderr.strip() or proc.stdout.strip() or "").strip()
        if not message or "not permitted" in message.lower():
            print("WARNING: launchctl list failed; treating as empty for hygiene checks.", file=sys.stderr)
            return []
        raise RuntimeError(message or "launchctl list failed")
    substring = (label_substring or "").strip().lower()
    if not substring:
        return []
    jobs = parse_launchctl_list(proc.stdout)
    return [job for job in jobs if substring in job.label.lower()]


def list_processes(*, ps_regex: str) -> list[ProcessInfo]:
    try:
        proc = run(["ps", "-axo", "pid=,command="])
    except PermissionError:
        print("WARNING: ps not permitted; treating as empty for hygiene checks.", file=sys.stderr)
        return []
    if proc.returncode != 0:
        message = (proc.stderr.strip() or proc.stdout.strip() or "").strip()
        if not message or "not permitted" in message.lower():
            print("WARNING: ps failed; treating as empty for hygiene checks.", file=sys.stderr)
            return []
        raise RuntimeError(message or "ps failed")
    rx = re.compile(ps_regex, re.IGNORECASE)
    pid_self = os.getpid()
    out: list[ProcessInfo] = []
    for line in (proc.stdout or "").splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        parts = stripped.split(maxsplit=1)
        if len(parts) != 2:
            continue
        pid_str, cmd = parts[0], parts[1]
        try:
            pid = int(pid_str)
        except ValueError:
            continue
        if pid == pid_self:
            continue
        if rx.search(cmd):
            out.append(ProcessInfo(pid=pid, command=cmd))
    return out


def list_launchd_override_keys(*, uid: int, prefixes: tuple[str, ...]) -> list[str]:
    disabled_plist = f"/var/db/com.apple.xpc.launchd/disabled.{uid}.plist"
    if not os.path.exists(disabled_plist):
        return []
    proc = run(["plutil", "-convert", "json", "-o", "-", disabled_plist])
    if proc.returncode != 0:
        return []
    try:
        payload = json.loads(proc.stdout or "{}")
    except json.JSONDecodeError:
        return []
    if not isinstance(payload, dict):
        return []
    clean_prefixes = tuple(str(p) for p in prefixes if str(p).strip())
    if not clean_prefixes:
        return []
    matched = [
        key
        for key in payload.keys()
        if isinstance(key, str) and any(key.startswith(prefix) for prefix in clean_prefixes)
    ]
    return sorted(matched)


def list_dmg_images(*, substrings: tuple[str, ...]) -> list[DmgImage]:
    parts = tuple(s for s in (substrings or ()) if str(s).strip())
    if not parts:
        return []

    proc = run_bytes(["hdiutil", "info", "-plist"])
    if proc.returncode != 0 or not proc.stdout:
        return []
    try:
        payload = plistlib.loads(proc.stdout)
    except Exception:
        return []
    images = payload.get("images")
    if not isinstance(images, list):
        return []

    dev_re = re.compile(r"^/dev/disk\\d+$")
    out: list[DmgImage] = []
    for image in images:
        if not isinstance(image, dict):
            continue
        image_path = image.get("image-path")
        if not isinstance(image_path, str):
            continue
        if not any(part in image_path for part in parts):
            continue

        dev_entry = None
        mount_point = None
        entities = image.get("system-entities")
        if isinstance(entities, list):
            dev_candidates = []
            mount_candidates = []
            for entity in entities:
                if not isinstance(entity, dict):
                    continue
                dev = entity.get("dev-entry")
                if isinstance(dev, str):
                    dev_candidates.append(dev)
                mp = entity.get("mount-point")
                if isinstance(mp, str) and mp:
                    mount_candidates.append(mp)
            dev_entry = next((d for d in dev_candidates if dev_re.match(d)), None) or (
                dev_candidates[0] if dev_candidates else None
            )
            mount_point = mount_candidates[0] if mount_candidates else None

        out.append(DmgImage(image_path=image_path, dev_entry=dev_entry, mount_point=mount_point))
    return out


def collect_state(
    *,
    label_substring: str = DEFAULT_LAUNCHD_LABEL_SUBSTRING,
    ps_regex: str = DEFAULT_PS_REGEX,
    override_uid: int | None = None,
    override_prefixes: tuple[str, ...] = DEFAULT_LAUNCHD_OVERRIDE_PREFIXES,
    dmg_substrings: tuple[str, ...] = DEFAULT_DMG_SUBSTRINGS,
) -> HygieneState:
    uid = int(override_uid if override_uid is not None else os.getuid())
    return HygieneState(
        launchd_jobs=tuple(list_launchd_jobs(label_substring=label_substring)),
        processes=tuple(list_processes(ps_regex=ps_regex)),
        launchd_override_keys=tuple(
            list_launchd_override_keys(uid=uid, prefixes=tuple(override_prefixes))
        ),
        dmg_images=tuple(list_dmg_images(substrings=tuple(dmg_substrings))),
    )


def format_state(state: HygieneState, *, label_substring: str, ps_regex: str) -> str:
    lines: list[str] = []
    if state.launchd_jobs:
        lines.append(f"launchd jobs (matching '{label_substring}'): {len(state.launchd_jobs)}")
        for job in state.launchd_jobs:
            lines.append(f"  {job.raw_line}")
    else:
        lines.append("launchd jobs: (none)")

    if state.processes:
        lines.append(f"processes (matching /{ps_regex}/i): {len(state.processes)}")
        for proc in state.processes:
            lines.append(f"  {proc.pid} {proc.command}")
    else:
        lines.append("processes: (none)")

    if state.dmg_images:
        lines.append(f"attached FarmDashboard DMGs: {len(state.dmg_images)}")
        for image in state.dmg_images[:25]:
            suffix = f" ({image.dev_entry})" if image.dev_entry else ""
            lines.append(f"  {image.image_path}{suffix}")
        if len(state.dmg_images) > 25:
            lines.append(f"  ... ({len(state.dmg_images) - 25} more)")
    else:
        lines.append("attached FarmDashboard DMGs: (none)")

    if state.launchd_override_keys:
        lines.append(f"launchd override keys (E2E pollution): {len(state.launchd_override_keys)}")
        for key in state.launchd_override_keys[:50]:
            lines.append(f"  {key}")
        if len(state.launchd_override_keys) > 50:
            lines.append(f"  ... ({len(state.launchd_override_keys) - 50} more)")
        lines.append("")
        lines.append("To purge stale override keys (one-time, requires admin):")
        uid = os.getuid()
        lines.append(
            f"  sudo python3 tools/purge_launchd_overrides.py --uid {uid} --apply --backup"
        )
    else:
        lines.append("launchd override keys: (none)")

    return "\n".join(lines)


def remove_launchd_jobs(
    jobs: Iterable[LaunchdJob],
    *,
    allowed_prefixes: tuple[str, ...] = DEFAULT_LAUNCHD_CLEAN_PREFIXES,
) -> list[str]:
    removed: list[str] = []
    prefixes = tuple(p.strip().lower() for p in allowed_prefixes if str(p).strip())
    for job in jobs:
        if prefixes and not any(job.label.lower().startswith(prefix) for prefix in prefixes):
            continue
        proc = run(["launchctl", "remove", job.label])
        if proc.returncode == 0:
            removed.append(job.label)
        else:
            # `launchctl remove` may fail if the job is already gone; keep going.
            removed.append(job.label)
    return removed


def is_safe_to_terminate(process: ProcessInfo) -> bool:
    cmd = process.command
    lower = cmd.lower()
    if "farm_setup_" in lower:
        return True
    if "/farm_dashboard/" in lower or "\\farm_dashboard\\" in lower:
        return True
    # Production installs use a hyphenated install root.
    if "/usr/local/farm-dashboard/" in lower:
        return True
    if "/users/shared/farmdashboard/" in lower:
        return True
    if "com.farmdashboard" in lower:
        return True
    if re.search(r"\bcore-server\b", lower):
        return True
    if re.search(r"\btelemetry-sidecar\b", lower):
        return True
    if re.search(r"\bmosquitto\b", lower):
        return True
    if re.search(r"\bredis-server\b", lower):
        return True
    if re.search(r"\bpostgres\b", lower):
        return True
    if "farmctl" in lower and " serve " in lower:
        return True
    return False


def terminate_process(pid: int, *, timeout_seconds: float) -> None:
    deadline = time.time() + timeout_seconds
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    except PermissionError:
        return

    while time.time() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.1)

    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        return
    except PermissionError:
        return


def terminate_processes(processes: Iterable[ProcessInfo], *, timeout_seconds: float) -> list[int]:
    terminated: list[int] = []
    for proc in processes:
        if not is_safe_to_terminate(proc):
            continue
        terminate_process(proc.pid, timeout_seconds=timeout_seconds)
        terminated.append(proc.pid)
    return terminated


def detach_dmg_images(images: Iterable[DmgImage]) -> list[str]:
    removed: list[str] = []
    for image in images:
        if not image.dev_entry:
            continue
        proc = run(["hdiutil", "detach", image.dev_entry, "-force"])
        if proc.returncode == 0:
            removed.append(image.dev_entry)
        else:
            # `hdiutil detach` may fail if the device is already gone; keep going.
            removed.append(image.dev_entry)
    return removed


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check (and optionally clean) orphaned Farm Dashboard services before/after tests."
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Attempt to remove matching launchd jobs and terminate matching processes.",
    )
    parser.add_argument(
        "--label-substring",
        default=DEFAULT_LAUNCHD_LABEL_SUBSTRING,
        help=f"Substring match for launchd labels (default: {DEFAULT_LAUNCHD_LABEL_SUBSTRING!r}).",
    )
    parser.add_argument(
        "--ps-regex",
        default=DEFAULT_PS_REGEX,
        help=f"Regex for matching suspicious processes (default: {DEFAULT_PS_REGEX!r}).",
    )
    parser.add_argument(
        "--kill-timeout-seconds",
        type=float,
        default=5.0,
        help="Seconds to wait after SIGTERM before SIGKILL.",
    )
    parser.add_argument(
        "--override-prefix",
        action="append",
        default=list(DEFAULT_LAUNCHD_OVERRIDE_PREFIXES),
        help=(
            "Launchd override key prefix to detect (repeatable). "
            "Default: com.farmdashboard.e2e."
        ),
    )
    parser.add_argument(
        "--override-uid",
        type=int,
        default=os.getuid(),
        help="UID used for disabled.<uid>.plist override checks (default: current user).",
    )
    parser.add_argument(
        "--strict-overrides",
        action="store_true",
        help=(
            "Treat matching launchd override keys as a hard failure. "
            "Recommended for E2E when you want a fully pristine machine state."
        ),
    )
    args = parser.parse_args()

    print("Farm Dashboard test hygiene")
    print("Manual checks:")
    print("  launchctl list 2>/dev/null | grep -i farm")
    print('  ps aux | grep -E "core-server|telemetry-sidecar|farm|mosquitto|qdrant|setup-daemon" | grep -v grep')
    print("  hdiutil info | rg -i FarmDashboard")
    print("")

    state = collect_state(
        label_substring=args.label_substring,
        ps_regex=args.ps_regex,
        override_uid=args.override_uid,
        override_prefixes=tuple(args.override_prefix),
    )
    if state.is_clean() and not (args.strict_overrides and state.launchd_override_keys):
        if state.launchd_override_keys:
            print("Status: CLEAN (but launchd override keys present; recommended one-time purge)")
            print(format_state(state, label_substring=args.label_substring, ps_regex=args.ps_regex))
        else:
            print("Status: CLEAN")
        return 0

    print("Status: NOT CLEAN")
    print(format_state(state, label_substring=args.label_substring, ps_regex=args.ps_regex))

    if not args.apply:
        return 2

    if state.dmg_images:
        removed = detach_dmg_images(state.dmg_images)
        if removed:
            print("")
            print(f"Detached DMG devices (best-effort): {len(removed)}")
            for dev in removed:
                print(f"  {dev}")

    if state.launchd_jobs:
        removed = remove_launchd_jobs(state.launchd_jobs)
        if removed:
            print("")
            print(f"Removed launchd jobs (best-effort): {len(removed)}")
            for label in removed:
                print(f"  {label}")

    if state.processes:
        terminated = terminate_processes(state.processes, timeout_seconds=args.kill_timeout_seconds)
        if terminated:
            print("")
            print(f"Terminated processes (best-effort): {len(terminated)}")
            for pid in terminated:
                print(f"  {pid}")

    print("")
    post = collect_state(
        label_substring=args.label_substring,
        ps_regex=args.ps_regex,
        override_uid=args.override_uid,
        override_prefixes=tuple(args.override_prefix),
    )
    if post.is_clean() and not (args.strict_overrides and post.launchd_override_keys):
        if post.launchd_override_keys:
            print(
                "Status after cleanup: CLEAN (but launchd override keys present; recommended one-time purge)"
            )
            print(format_state(post, label_substring=args.label_substring, ps_regex=args.ps_regex))
        else:
            print("Status after cleanup: CLEAN")
        return 0

    print("Status after cleanup: NOT CLEAN")
    print(format_state(post, label_substring=args.label_substring, ps_regex=args.ps_regex))
    return 3


if __name__ == "__main__":
    raise SystemExit(main())
