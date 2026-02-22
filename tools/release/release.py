#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path


SEMVER_RE = re.compile(
    r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)"
    r"(?:-(?P<prerelease>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


def _run(cmd: list[str]) -> str:
    return subprocess.check_output(cmd, text=True).strip()


def _git_ref_exists(ref: str) -> bool:
    try:
        _run(["git", "rev-parse", "--verify", ref])
        return True
    except subprocess.CalledProcessError:
        return False


def _ensure_ref(ref: str) -> None:
    if _git_ref_exists(ref):
        return
    remote = "origin"
    refspec = ref.split("/", 1)[1] if ref.startswith(f"{remote}/") else ref
    subprocess.run(["git", "fetch", remote, refspec], check=False)


def _git_diff_names(base_ref: str | None) -> set[str]:
    if not base_ref:
        return set()
    _ensure_ref(base_ref)
    if not _git_ref_exists(base_ref):
        return set()
    output = _run(["git", "diff", "--name-only", f"{base_ref}...HEAD"])
    return {line.strip() for line in output.splitlines() if line.strip()}


def _resolve_channel(cli_channel: str | None) -> str:
    if cli_channel:
        return cli_channel
    env_channel = os.getenv("RELEASE_CHANNEL")
    if env_channel:
        return env_channel
    ref = os.getenv("GITHUB_REF", "")
    ref_name = os.getenv("GITHUB_REF_NAME", "")
    event = os.getenv("GITHUB_EVENT_NAME", "")
    tag = ref_name if ref.startswith("refs/tags/") else ""
    if tag:
        if "-alpha" in tag:
            return "alpha"
        if "-beta" in tag:
            return "beta"
        return "stable"
    if event == "pull_request":
        return "alpha"
    if ref == "refs/heads/main":
        return "beta"
    return "alpha"


def _parse_semver(version: str) -> dict[str, str] | None:
    match = SEMVER_RE.match(version.strip())
    if not match:
        return None
    return match.groupdict()


def _load_web_version() -> str:
    data = json.loads(Path("apps/dashboard-web/package.json").read_text())
    return str(data.get("version", "")).strip()


def _validate_version(version: str, channel: str, label: str) -> list[str]:
    errors: list[str] = []
    parsed = _parse_semver(version)
    if not parsed:
        errors.append(f"{label} version '{version}' is not valid semver (MAJOR.MINOR.PATCH).")
        return errors
    prerelease = parsed.get("prerelease")
    if channel == "stable" and prerelease:
        errors.append(f"{label} version '{version}' has prerelease tags but channel is stable.")
    if channel in {"alpha", "beta"} and prerelease and not prerelease.startswith(channel):
        errors.append(
            f"{label} version '{version}' prerelease '{prerelease}' does not match channel '{channel}'."
        )
    return errors


def _validate_bump(changed: set[str], target: str, version_path: str) -> list[str]:
    if not changed:
        return []
    if any(path.startswith(target) and not path.endswith(".md") for path in changed):
        if version_path not in changed:
            return [f"{target} changed but version file '{version_path}' was not updated."]
    return []


def validate(targets: list[str], channel: str, base_ref: str | None) -> int:
    errors: list[str] = []
    versions: dict[str, str] = {}
    supported_targets = {"web"}

    unknown_targets = [target for target in targets if target not in supported_targets]
    if unknown_targets:
        errors.append(
            "Unsupported release targets: "
            + ", ".join(sorted(unknown_targets))
            + ". Supported targets: web."
        )

    if "web" in targets:
        versions["web"] = _load_web_version()

    for label, version in versions.items():
        errors.extend(_validate_version(version, channel, label))

    changed = _git_diff_names(base_ref)
    if "web" in targets:
        errors.extend(
            _validate_bump(changed, "apps/dashboard-web", "apps/dashboard-web/package.json")
        )

    if errors:
        for err in errors:
            print(f"ERROR: {err}")
        return 1

    for label, version in versions.items():
        print(f"{label}: {version} ({channel})")
    return 0


def _git_last_tag() -> str | None:
    try:
        return _run(["git", "describe", "--tags", "--abbrev=0"])
    except subprocess.CalledProcessError:
        return None


def changelog(version: str, channel: str, since: str | None, output: str | None) -> int:
    base = since or _git_last_tag()
    args = ["git", "log", "--pretty=format:%s"]
    if base:
        args.append(f"{base}..HEAD")
    messages = _run(args).splitlines()
    today = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    header = f"## {version} ({channel}) - {today}"
    body = "\n".join(f"- {msg}" for msg in messages if msg.strip()) or "- No changes recorded."
    content = f"{header}\n\n{body}\n"
    if output:
        path = Path(output)
        existing = path.read_text() if path.exists() else ""
        path.write_text(content + "\n" + existing)
        print(f"Wrote changelog to {path}")
    else:
        print(content)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Release tooling (channels, semver, changelog).")
    sub = parser.add_subparsers(dest="command", required=True)

    validate_cmd = sub.add_parser("validate", help="Validate release channel + versioning")
    validate_cmd.add_argument("--channel", default=None, help="Override release channel")
    validate_cmd.add_argument(
        "--targets",
        default="web",
        help="Comma-separated targets: web",
    )
    validate_cmd.add_argument(
        "--base-ref",
        default=os.getenv("GITHUB_BASE_REF") and f"origin/{os.getenv('GITHUB_BASE_REF')}",
        help="Git base ref to diff for version bump checks",
    )

    changelog_cmd = sub.add_parser("changelog", help="Generate changelog from git history")
    changelog_cmd.add_argument("--version", required=True, help="Release version")
    changelog_cmd.add_argument("--channel", default=None, help="Release channel")
    changelog_cmd.add_argument("--since", default=None, help="Git ref/tag to start from")
    changelog_cmd.add_argument("--output", default=None, help="Write to file instead of stdout")

    args = parser.parse_args()

    if args.command == "validate":
        channel = _resolve_channel(args.channel)
        if channel not in {"alpha", "beta", "stable"}:
            print(f"ERROR: Invalid release channel '{channel}'. Use alpha, beta, or stable.")
            return 1
        targets = [item.strip() for item in args.targets.split(",") if item.strip()]
        return validate(targets, channel, args.base_ref)

    if args.command == "changelog":
        channel = _resolve_channel(args.channel)
        return changelog(args.version, channel, args.since, args.output)

    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
