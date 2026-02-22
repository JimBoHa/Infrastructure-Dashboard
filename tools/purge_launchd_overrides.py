#!/usr/bin/env python3
"""
Purge launchd enable/disable override records for farm-dashboard labels.

Why this exists:
- E2E runs should not leave persistent launchd "overrides" behind.
- Older runs (or accidental `launchctl enable`) can accumulate keys in:
    /var/db/com.apple.xpc.launchd/disabled.<uid>.plist
  even when jobs/processes are no longer running.

This tool can list and (optionally) remove matching keys. Removing keys requires
root privileges because the overrides plist is root-owned. Run with `--apply`
and either:
  - run the entire script with `sudo`, or
  - allow it to invoke `sudo cp` to replace the plist.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def run(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, check=check, capture_output=True, text=True)


def plutil_to_json(plist_path: Path) -> dict:
    proc = run(["plutil", "-convert", "json", "-o", "-", str(plist_path)], check=False)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "plutil failed")
    try:
        data = json.loads(proc.stdout or "{}")
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"Failed to parse JSON from plutil: {exc}") from exc
    if not isinstance(data, dict):
        raise RuntimeError("Overrides plist JSON is not an object")
    return data


def write_json_plist(payload: dict, output_path: Path) -> None:
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True))
    run(["plutil", "-convert", "binary1", str(output_path)], check=True)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Purge persistent launchd override keys for Farm Dashboard labels."
    )
    parser.add_argument(
        "--uid",
        type=int,
        default=os.getuid(),
        help="Target UID (default: current user).",
    )
    parser.add_argument(
        "--prefix",
        action="append",
        default=["com.farmdashboard.e2e."],
        help="Label prefix to match (repeatable). Default: com.farmdashboard.e2e.",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply changes (requires root to replace the overrides plist).",
    )
    parser.add_argument(
        "--backup",
        action="store_true",
        help="Create a timestamped backup copy next to the overrides plist (requires root).",
    )
    args = parser.parse_args()

    disabled_plist = Path(f"/var/db/com.apple.xpc.launchd/disabled.{args.uid}.plist")
    if not disabled_plist.exists():
        print(f"(no overrides plist found at {disabled_plist})")
        return 0

    payload = plutil_to_json(disabled_plist)
    prefixes = [p for p in (args.prefix or []) if str(p).strip()]
    if not prefixes:
        raise SystemExit("--prefix must not be empty")

    matched = sorted(
        [
            key
            for key in payload.keys()
            if isinstance(key, str) and any(key.startswith(prefix) for prefix in prefixes)
        ]
    )
    print(f"Overrides plist: {disabled_plist}")
    print(f"Matched keys: {len(matched)}")
    for key in matched[:50]:
        print(f"  {key} = {payload.get(key)}")
    if len(matched) > 50:
        print(f"  ... ({len(matched) - 50} more)")

    if not args.apply:
        return 0

    if not matched:
        print("(nothing to remove)")
        return 0

    new_payload = dict(payload)
    for key in matched:
        new_payload.pop(key, None)

    with tempfile.TemporaryDirectory(prefix="farm_launchd_overrides_") as tmp:
        tmp_path = Path(tmp)
        replacement = tmp_path / f"disabled.{args.uid}.plist"
        write_json_plist(new_payload, replacement)

        if args.backup:
            stamp = time.strftime("%Y%m%d_%H%M%S")
            backup = Path(f"{disabled_plist}.{stamp}.bak")
            print(f"Creating backup: {backup}")
            run(["sudo", "cp", str(disabled_plist), str(backup)], check=True)

        print("Replacing overrides plist (requires admin password)...")
        run(["sudo", "cp", str(replacement), str(disabled_plist)], check=True)
        run(["sudo", "chown", "root:wheel", str(disabled_plist)], check=True)
        run(["sudo", "chmod", "0644", str(disabled_plist)], check=True)

    print("Done. Note: launchd may not reflect changes until the next login/reboot.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

