#!/usr/bin/env python3
"""
Hard gate for Tier-A screenshot review evidence.

This checker fails unless a Tier-A run log includes:
  1) A dedicated "Tier A Screenshot Review (Hard Gate)" section.
  2) Checked REVIEWED screenshot entries that point to existing files.
  3) A minimum number of checked visual checks tied to reviewed screenshots.
  4) A findings section with at least one bullet.
  5) A reviewer declaration line.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


SECTION_HEADING = "## Tier A Screenshot Review (Hard Gate)"
VISUAL_CHECKS_HEADING = "### Visual checks (required)"
FINDINGS_HEADING = "### Findings"
DECLARATION_HEADING = "### Reviewer declaration"

REVIEWED_LINE_RE = re.compile(r"^- \[x\]\s+REVIEWED:\s+`?([^`\s]+)`?\s*$")
VISUAL_CHECK_LINE_RE = re.compile(r"^- \[x\]\s+(PASS|FAIL):\s+(.+)$")
FINDING_LINE_RE = re.compile(r"^- (.+)$")


class GateError(RuntimeError):
    """Raised when one or more gate checks fail."""


def _load_text(path: Path) -> str:
    if not path.exists():
        raise GateError(f"Run log does not exist: {path}")
    try:
        return path.read_text(encoding="utf-8")
    except OSError as err:
        raise GateError(f"Failed to read run log {path}: {err}") from err


def _extract_section(text: str, heading: str) -> str:
    escaped = re.escape(heading)
    start = re.search(rf"^{escaped}\s*$", text, flags=re.MULTILINE)
    if not start:
        raise GateError(f"Missing required section heading: {heading}")
    start_idx = start.end()
    rest = text[start_idx:]
    next_heading = re.search(r"^##\s+", rest, flags=re.MULTILINE)
    if not next_heading:
        return rest
    return rest[: next_heading.start()]


def _extract_subsection(parent_section: str, heading: str) -> str:
    escaped = re.escape(heading)
    start = re.search(rf"^{escaped}\s*$", parent_section, flags=re.MULTILINE)
    if not start:
        raise GateError(f"Missing required subsection heading: {heading}")
    start_idx = start.end()
    rest = parent_section[start_idx:]
    next_sub = re.search(r"^###\s+", rest, flags=re.MULTILINE)
    if not next_sub:
        return rest
    return rest[: next_sub.start()]


def _normalize_rel_path(raw: str) -> Path:
    path = Path(raw.strip())
    if path.is_absolute():
        return path
    return path


def _validate_gate(
    *,
    text: str,
    repo_root: Path,
    min_reviewed: int,
    min_checks: int,
    allow_failing_checks: bool,
) -> tuple[list[Path], int]:
    section = _extract_section(text, SECTION_HEADING)

    reviewed_paths: list[Path] = []
    for raw_line in section.splitlines():
        match = REVIEWED_LINE_RE.match(raw_line.strip())
        if not match:
            continue
        raw_path = match.group(1)
        path = _normalize_rel_path(raw_path)
        if not path.is_absolute():
            path = (repo_root / path).resolve()
        reviewed_paths.append(path)

    if len(reviewed_paths) < min_reviewed:
        raise GateError(
            f"Expected at least {min_reviewed} checked REVIEWED screenshot entries; found {len(reviewed_paths)}."
        )

    missing = [p for p in reviewed_paths if not p.exists()]
    if missing:
        sample = "\n".join(f"- {p}" for p in missing[:10])
        raise GateError(
            "One or more REVIEWED screenshot files do not exist:\n"
            f"{sample}"
        )

    checks_section = _extract_subsection(section, VISUAL_CHECKS_HEADING)
    check_lines: list[tuple[str, str]] = []
    for raw_line in checks_section.splitlines():
        match = VISUAL_CHECK_LINE_RE.match(raw_line.strip())
        if not match:
            continue
        check_lines.append((match.group(1), match.group(2)))

    if len(check_lines) < min_checks:
        raise GateError(
            f"Expected at least {min_checks} checked visual checks; found {len(check_lines)}."
        )

    reviewed_basenames = {p.name for p in reviewed_paths}
    missing_refs = []
    for idx, (_, body) in enumerate(check_lines, start=1):
        if not any(name in body for name in reviewed_basenames):
            missing_refs.append(idx)
    if missing_refs:
        missing_str = ", ".join(str(i) for i in missing_refs)
        raise GateError(
            "Each visual check must reference at least one reviewed screenshot filename. "
            f"Missing reference on check(s): {missing_str}."
        )

    fail_checks = [body for status, body in check_lines if status == "FAIL"]
    if fail_checks and not allow_failing_checks:
        sample = "\n".join(f"- {item}" for item in fail_checks[:10])
        raise GateError(
            "Visual checks include FAIL entries; Tier-A gate requires all checks PASS "
            "(or rerun with --allow-failing-checks for investigative logs):\n"
            f"{sample}"
        )

    findings_section = _extract_subsection(section, FINDINGS_HEADING)
    findings = [
        m.group(1).strip()
        for line in findings_section.splitlines()
        if (m := FINDING_LINE_RE.match(line.strip()))
    ]
    findings = [item for item in findings if item]
    if not findings:
        raise GateError("Findings subsection must include at least one bullet item.")

    declaration_section = _extract_subsection(section, DECLARATION_HEADING)
    if "I viewed each screenshot listed above." not in declaration_section:
        raise GateError(
            "Reviewer declaration is missing required sentence: "
            "'I viewed each screenshot listed above.'"
        )

    return reviewed_paths, len(check_lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate Tier-A screenshot review hard-gate evidence in a run log.")
    parser.add_argument(
        "--run-log",
        required=True,
        help="Path to Tier-A run log markdown file (for example: project_management/runs/RUN-YYYYMMDD-....md).",
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root used to resolve relative screenshot paths (default: current directory).",
    )
    parser.add_argument(
        "--min-reviewed",
        type=int,
        default=1,
        help="Minimum checked REVIEWED screenshot entries required (default: %(default)s).",
    )
    parser.add_argument(
        "--min-checks",
        type=int,
        default=3,
        help="Minimum checked visual checks required (default: %(default)s).",
    )
    parser.add_argument(
        "--allow-failing-checks",
        action="store_true",
        help="Allow FAIL visual checks (use for investigative logs, not release-pass Tier-A logs).",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    run_log_path = Path(args.run_log).expanduser().resolve()
    repo_root = Path(args.repo_root).expanduser().resolve()
    text = _load_text(run_log_path)
    try:
        reviewed_paths, check_count = _validate_gate(
            text=text,
            repo_root=repo_root,
            min_reviewed=max(1, int(args.min_reviewed)),
            min_checks=max(1, int(args.min_checks)),
            allow_failing_checks=bool(args.allow_failing_checks),
        )
    except GateError as err:
        print(f"Tier-A screenshot gate: FAIL\n{err}", file=sys.stderr)
        return 1

    rel_reviewed = []
    for path in reviewed_paths:
        try:
            rel_reviewed.append(str(path.relative_to(repo_root)))
        except ValueError:
            rel_reviewed.append(str(path))

    print("Tier-A screenshot gate: PASS")
    print(f"Run log: {run_log_path}")
    print(f"Reviewed screenshots ({len(reviewed_paths)}):")
    for item in rel_reviewed:
        print(f"- {item}")
    print(f"Visual checks: {check_count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
