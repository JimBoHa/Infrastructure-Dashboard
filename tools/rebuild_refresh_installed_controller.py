#!/usr/bin/env python3
"""
Rebuild a controller bundle DMG from the local repo and refresh the already-installed controller.

This follows the Tier-A runbook:
  docs/runbooks/controller-rebuild-refresh-tier-a.md

Defaults assume a dev machine with:
  - repo at /Users/FarmDashboard/farm_dashboard
  - setup-daemon at http://127.0.0.1:8800
  - core-server at http://127.0.0.1:8000
  - native deps at /usr/local/farm-dashboard/native
  - build outputs under /Users/Shared/FarmDashboardBuilds
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from datetime import datetime, timezone
from dataclasses import dataclass
from pathlib import Path
from subprocess import PIPE, STDOUT, Popen, run
from typing import Any, Iterable
from json import JSONDecodeError
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


@dataclass(frozen=True)
class Urls:
    setup_daemon_base: str
    core_base: str

    @property
    def setup_healthz(self) -> str:
        return self.setup_daemon_base.rstrip("/") + "/healthz"

    @property
    def setup_status(self) -> str:
        return self.setup_daemon_base.rstrip("/") + "/api/status"

    @property
    def setup_config(self) -> str:
        return self.setup_daemon_base.rstrip("/") + "/api/config"

    @property
    def setup_upgrade(self) -> str:
        return self.setup_daemon_base.rstrip("/") + "/api/upgrade"

    @property
    def core_healthz(self) -> str:
        return self.core_base.rstrip("/") + "/healthz"


def _write_stdout_bytes(chunk: bytes) -> None:
    out = getattr(sys.stdout, "buffer", None)
    if out is None:
        sys.stdout.write(chunk.decode("utf-8", errors="replace"))
        sys.stdout.flush()
        return
    out.write(chunk)
    out.flush()


def _read_http_body(
    url: str,
    *,
    method: str = "GET",
    payload: dict[str, Any] | None = None,
    accept: str = "application/json",
    timeout_s: int = 15,
) -> str:
    data = None
    headers = {"Accept": accept}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"
    req = Request(url, data=data, method=method, headers=headers)
    with urlopen(req, timeout=timeout_s) as resp:
        return resp.read().decode("utf-8", errors="replace")


def _http_json(url: str, method: str = "GET", payload: dict[str, Any] | None = None) -> Any:
    body = _read_http_body(url, method=method, payload=payload, accept="application/json")
    if not body.strip():
        return None
    try:
        return json.loads(body)
    except JSONDecodeError as err:
        preview = body.strip().replace("\n", "\\n")
        if len(preview) > 240:
            preview = preview[:240] + "…"
        raise RuntimeError(f"Expected JSON from {url} but got: {preview}") from err


def _http_json_or_text(
    url: str,
    method: str = "GET",
    payload: dict[str, Any] | None = None,
    timeout_s: int = 15,
) -> Any:
    body = _read_http_body(url, method=method, payload=payload, accept="application/json", timeout_s=timeout_s)
    if not body.strip():
        return None
    try:
        return json.loads(body)
    except JSONDecodeError:
        return body


def _http_text(url: str, method: str = "GET") -> str:
    return _read_http_body(url, method=method, payload=None, accept="*/*", timeout_s=10)


def _require_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def _unwrap_result(payload: Any) -> Any:
    # setup-daemon endpoints may wrap payloads as {"ok": true, "result": {...}, "logs": [...]}
    if isinstance(payload, dict):
        result = payload.get("result")
        if isinstance(result, dict):
            return result
    return payload


def _git_clean_or_die(repo: Path, allow_dirty: bool) -> None:
    proc = run(
        ["git", "status", "--porcelain=v1", "-b", "-z"],
        cwd=str(repo),
        capture_output=True,
    )
    if proc.returncode != 0:
        raise RuntimeError("git status failed (is git installed? is this a git repo?)")

    def is_allowed_path(path: str) -> bool:
        # Tier-A allowlist: only repo-root reports/** may be dirty for bundling.
        return path.startswith("reports/")

    dirty: list[str] = []
    fields = proc.stdout.split(b"\0")
    i = 0
    while i < len(fields):
        entry = fields[i]
        i += 1
        if not entry:
            continue
        if entry.startswith(b"## "):
            continue

        # Format (porcelain v1, -z): XY SP path
        if len(entry) < 4 or entry[2:3] != b" ":
            dirty.append(entry.decode("utf-8", errors="replace"))
            continue

        status = entry[:2].decode("utf-8", errors="replace")
        path = entry[3:].decode("utf-8", errors="replace")
        paths = [path]

        # For rename/copy, the next NUL field is the new path.
        if ("R" in status or "C" in status) and i < len(fields) and fields[i]:
            paths.append(fields[i].decode("utf-8", errors="replace"))
            i += 1

        if not all(is_allowed_path(p) for p in paths):
            joined = " -> ".join(paths)
            dirty.append(f"{status} {joined}")

    if dirty and not allow_dirty:
        msg = "\n".join(dirty[:50])
        raise RuntimeError(
            "Refusing to bundle from a dirty worktree (Tier-A hard gate).\n"
            "Only changes under reports/** are allowed.\n\n"
            f"Dirty entries:\n{msg}\n\n"
            "Use --allow-dirty to override (not recommended)."
        )


def _infer_next_version(urls: Urls) -> str:
    status = _unwrap_result(_http_json(urls.setup_status))
    current = None
    if isinstance(status, dict):
        current = status.get("current_version")
    if not isinstance(current, str) or not current.strip():
        raise RuntimeError(f"Could not determine current_version from {urls.setup_status}")

    match = re.fullmatch(r"\s*(\d+)\.(\d+)\.(\d+)\.(\d+)\s*", current)
    if not match:
        # Accept versions with a suffix (e.g. 0.1.9.249-my-branch) by incrementing
        # the leading numeric x.y.z.build portion.
        prefix_match = re.match(r"\s*(\d+)\.(\d+)\.(\d+)\.(\d+)(?:\b|[-+_])", current)
        if prefix_match:
            prefix = ".".join([prefix_match.group(1), prefix_match.group(2), prefix_match.group(3)])
            build = int(prefix_match.group(4)) + 1
            return f"{prefix}.{build}"

        # Final fallback for non-semver-like versions.
        stamp = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
        return f"dev-{stamp}"

    prefix = ".".join([match.group(1), match.group(2), match.group(3)])
    build = int(match.group(4)) + 1
    return f"{prefix}.{build}"


def _tee_process(argv: list[str], cwd: Path, log_path: Path) -> None:
    _require_dir(log_path.parent)
    with log_path.open("wb") as log_file:
        # Avoid stdout/stderr pipe deadlocks by merging streams.
        proc = Popen(argv, cwd=str(cwd), stdout=PIPE, stderr=STDOUT)
        assert proc.stdout is not None
        while True:
            chunk = proc.stdout.read(16 * 1024)
            if not chunk:
                break
            _write_stdout_bytes(chunk)
            log_file.write(chunk)

        rc = proc.wait()
        if rc != 0:
            raise RuntimeError(f"Command failed (exit {rc}). See log: {log_path}")


def _wait_for_healthz(url: str, timeout_s: int) -> None:
    deadline = time.time() + timeout_s
    last_err: str | None = None
    while time.time() < deadline:
        try:
            text = _http_text(url)
            if text.strip():
                return
        except (HTTPError, URLError) as err:
            last_err = str(err)
        time.sleep(1)
    raise RuntimeError(f"Timed out waiting for {url} ({timeout_s}s). Last error: {last_err}")


def _wait_for_version(urls: Urls, *, expected_version: str, timeout_s: int) -> None:
    deadline = time.time() + timeout_s
    last_seen: str | None = None
    while time.time() < deadline:
        try:
            status = _unwrap_result(_http_json(urls.setup_status))
            if isinstance(status, dict):
                current = status.get("current_version")
                if isinstance(current, str):
                    last_seen = current
                    if current == expected_version:
                        return
        except Exception:
            pass
        time.sleep(2)
    raise RuntimeError(
        f"Timed out waiting for setup-daemon current_version={expected_version!r} "
        f"(last seen: {last_seen!r})"
    )


def _prune_old_artifacts(
    *,
    output_dir: Path,
    keep_paths: set[Path],
    keep_latest: int,
    max_age_days: int,
) -> None:
    now = time.time()
    max_age_s = max_age_days * 24 * 60 * 60

    dmg_candidates = sorted(
        output_dir.glob("FarmDashboardController-*.dmg"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    dmg_keep = set(dmg_candidates[: max(0, keep_latest)]) | keep_paths

    pruned = 0
    reclaimed_bytes = 0
    for path in dmg_candidates:
        if path in dmg_keep:
            continue
        try:
            age_s = now - path.stat().st_mtime
            if age_s < max_age_s:
                continue
            size = path.stat().st_size
            path.unlink()
            pruned += 1
            reclaimed_bytes += size
            print(f"Pruned old bundle: {path}")
        except FileNotFoundError:
            continue

    logs_dir = output_dir / "logs"
    if logs_dir.is_dir():
        log_candidates = sorted(
            logs_dir.glob("*.log"),
            key=lambda p: p.stat().st_mtime,
            reverse=True,
        )
        log_keep = set(log_candidates[: max(0, keep_latest)])
        for path in log_candidates:
            if path in log_keep:
                continue
            try:
                age_s = now - path.stat().st_mtime
                if age_s < max_age_s:
                    continue
                size = path.stat().st_size
                path.unlink()
                pruned += 1
                reclaimed_bytes += size
                print(f"Pruned old log: {path}")
            except FileNotFoundError:
                continue

    if pruned:
        mib = reclaimed_bytes / (1024 * 1024)
        print(f"Pruned {pruned} artifact(s), reclaimed ~{mib:.1f} MiB")
    else:
        print("Artifact prune: nothing eligible")


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Rebuild controller bundle from repo and refresh installed controller (Tier A)."
    )
    parser.add_argument(
        "--repo",
        default="/Users/FarmDashboard/farm_dashboard",
        help="Path to farm_dashboard repo (default: %(default)s)",
    )
    parser.add_argument(
        "--version",
        default="",
        help="Controller bundle version string (default: auto-increment from setup-daemon current_version)",
    )
    parser.add_argument(
        "--output-dir",
        default="/Users/Shared/FarmDashboardBuilds",
        help="Directory to write built DMGs (default: %(default)s)",
    )
    parser.add_argument(
        "--native-deps",
        default="/usr/local/farm-dashboard/native",
        help="Native deps path passed to farmctl bundle (default: %(default)s)",
    )
    parser.add_argument(
        "--setup-daemon",
        default="http://127.0.0.1:8800",
        help="Setup daemon base URL (default: %(default)s)",
    )
    parser.add_argument(
        "--core",
        default="http://127.0.0.1:8000",
        help="Core server base URL (default: %(default)s)",
    )
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="Allow bundling from a dirty worktree (NOT recommended; Tier-A gate).",
    )
    parser.add_argument(
        "--skip-health-checks",
        action="store_true",
        help="Skip initial /healthz preflight checks.",
    )
    parser.add_argument(
        "--post-upgrade-health-smoke",
        action="store_true",
        help="Run `make e2e-installed-health-smoke` after upgrade/healthz wait and tee logs.",
    )
    parser.add_argument(
        "--reuse-existing-bundle",
        action="store_true",
        help=(
            "If the target DMG already exists in --output-dir, skip `farmctl bundle` "
            "and reuse it for refresh."
        ),
    )
    parser.add_argument(
        "--farmctl-skip-build",
        action="store_true",
        help="Pass `--skip-build` to `farmctl bundle` to reuse previously built artifacts.",
    )
    parser.add_argument(
        "--upgrade-wait-timeout-s",
        type=int,
        default=900,
        help="Max seconds to wait for installed version to reach --version after upgrade (default: %(default)s)",
    )
    parser.add_argument(
        "--no-prune-artifacts",
        action="store_true",
        help="Disable post-refresh pruning of old DMGs/logs in --output-dir.",
    )
    parser.add_argument(
        "--prune-max-age-days",
        type=int,
        default=7,
        help="Delete artifacts older than this many days (default: %(default)s)",
    )
    parser.add_argument(
        "--prune-keep-latest",
        type=int,
        default=8,
        help="Always keep at least this many most-recent DMGs/logs (default: %(default)s)",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    repo = Path(args.repo).expanduser().resolve()
    if not (repo / "apps" / "farmctl" / "Cargo.toml").exists():
        raise RuntimeError(f"--repo does not look like farm_dashboard: {repo}")

    urls = Urls(setup_daemon_base=args.setup_daemon, core_base=args.core)

    if not args.skip_health_checks:
        _http_text(urls.setup_healthz)
        _http_text(urls.core_healthz)

    pre_status_raw = _http_json_or_text(urls.setup_status)
    pre_status = _unwrap_result(pre_status_raw)
    pre_config = _unwrap_result(_http_json_or_text(urls.setup_config))
    pre_current: str | None = None
    pre_bundle_path: Path | None = None
    if isinstance(pre_status, dict):
        current = pre_status.get("current_version")
        previous = pre_status.get("previous_version")
        pre_current = current if isinstance(current, str) and current.strip() else None
        print(f"Preflight: setup-daemon current_version={current} previous_version={previous}")
    if isinstance(pre_config, dict):
        bundle_path = pre_config.get("bundle_path")
        if isinstance(bundle_path, str) and bundle_path.strip():
            print(f"Preflight: configured bundle_path={bundle_path}")
            pre_bundle_path = Path(bundle_path).expanduser()
            if isinstance(pre_status, dict):
                current = pre_status.get("current_version")
                if isinstance(current, str) and current.strip():
                    print(f"Rollback target (before refresh): version={current} bundle_path={bundle_path}")

    _git_clean_or_die(repo, allow_dirty=bool(args.allow_dirty))

    version = args.version.strip() or _infer_next_version(urls)
    out_dir = Path(args.output_dir).expanduser().resolve()
    logs_dir = out_dir / "logs"
    _require_dir(out_dir)
    _require_dir(logs_dir)

    dmg_path = out_dir / f"FarmDashboardController-{version}.dmg"
    log_path = logs_dir / f"bundle-{version}.log"

    bundle_cmd = [
        "cargo",
        "run",
        "--manifest-path",
        "apps/farmctl/Cargo.toml",
        "--",
        "bundle",
        "--version",
        version,
        "--output",
        str(dmg_path),
        "--native-deps",
        args.native_deps,
    ]
    if args.farmctl_skip_build:
        bundle_cmd.append("--skip-build")

    if args.reuse_existing_bundle and dmg_path.exists():
        print(f"[1/4] Reusing existing controller bundle: {dmg_path}")
        print("      (skipped `farmctl bundle` due to --reuse-existing-bundle)")
    else:
        print(f"[1/4] Building controller bundle: {dmg_path}")
        print(f"      Log: {log_path}")
        _tee_process(bundle_cmd, cwd=repo, log_path=log_path)

    print("[2/4] Pointing setup-daemon at new DMG")
    _http_json_or_text(urls.setup_config, method="POST", payload={"bundle_path": str(dmg_path)})

    print("[3/4] Triggering upgrade (refresh installed controller)")
    try:
        _http_json_or_text(urls.setup_upgrade, method="POST", payload=None, timeout_s=120)
    except TimeoutError:
        # setup-daemon may keep the HTTP request open while farmctl upgrade runs.
        # Continue and verify completion by polling current_version.
        print("Upgrade request timed out after 120s; continuing with version/health polling.")

    _wait_for_version(urls, expected_version=version, timeout_s=max(60, int(args.upgrade_wait_timeout_s)))

    print("[4/4] Waiting for core-server to become healthy")
    _wait_for_healthz(urls.core_healthz, timeout_s=60)

    if args.post_upgrade_health_smoke:
        smoke_log_path = logs_dir / f"installed-health-smoke-{version}.log"
        print("[5/5] Running installed health smoke")
        print(f"      Log: {smoke_log_path}")
        _tee_process(["make", "e2e-installed-health-smoke"], cwd=repo, log_path=smoke_log_path)

    try:
        status = _unwrap_result(_http_json(urls.setup_status))
        if isinstance(status, dict):
            current = status.get("current_version")
            previous = status.get("previous_version")
            print(f"Done. setup-daemon current_version={current} previous_version={previous}")
        else:
            print("Done. (setup-daemon status response not JSON object)")
    except Exception as err:  # noqa: BLE001 - best-effort summary
        print(f"Done. (failed to read setup-daemon status: {err})")

    if not args.no_prune_artifacts:
        keep_paths: set[Path] = {dmg_path.resolve()}
        if pre_bundle_path is not None:
            keep_paths.add(pre_bundle_path.resolve())
        print("Pruning old external artifacts...")
        _prune_old_artifacts(
            output_dir=out_dir,
            keep_paths=keep_paths,
            keep_latest=max(1, int(args.prune_keep_latest)),
            max_age_days=max(1, int(args.prune_max_age_days)),
        )
        if pre_current:
            print(f"Rollback reminder: pre-refresh version was {pre_current}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
