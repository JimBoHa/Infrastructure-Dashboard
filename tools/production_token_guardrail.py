#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

TOKENS = ("stub", "demo", "fallback")
TOKEN_PATTERN = re.compile(r"\b(stub|demo|fallback)\b", re.IGNORECASE)

INCLUDE_GLOBS = (
    "apps/core-server-rs/src/routes/**/*.rs",
    "apps/farmctl/src/**/*.rs",
    "apps/dashboard-web/src/lib/api.ts",
    "apps/node-agent/app/**/*.py",
)

EXCLUDED_PARTS = (
    "/generated_api/",
    "/tests/",
    "/test/",
    "/__pycache__/",
    "/migrations/",
)

ALLOWLIST_PATH = Path("tools/guardrails/production_token_allowlist.json")


@dataclass(frozen=True)
class TokenCount:
    path: str
    token: str
    count: int


def _is_excluded(path: Path) -> bool:
    normalized = path.as_posix()
    return any(part in normalized for part in EXCLUDED_PARTS)


def _scan(root: Path) -> dict[tuple[str, str], int]:
    counts: dict[tuple[str, str], int] = {}
    for pattern in INCLUDE_GLOBS:
        for path in root.glob(pattern):
            if not path.is_file() or _is_excluded(path):
                continue
            rel = path.relative_to(root).as_posix()
            text = path.read_text(encoding="utf-8")
            for match in TOKEN_PATTERN.finditer(text):
                token = match.group(1).lower()
                key = (rel, token)
                counts[key] = counts.get(key, 0) + 1
    return counts


def _load_allowlist(path: Path) -> dict[tuple[str, str], int]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    entries = payload.get("entries", [])
    allowlist: dict[tuple[str, str], int] = {}
    for entry in entries:
        key = (entry["path"], entry["token"].lower())
        allowlist[key] = int(entry["max_count"])
    return allowlist


def _write_allowlist(path: Path, counts: dict[tuple[str, str], int]) -> None:
    entries = [
        {
            "path": rel,
            "token": token,
            "max_count": count,
            "reason": "Baseline allowlisted fallback/stub/demo usage; tighten in follow-up cleanup tasks.",
        }
        for (rel, token), count in sorted(counts.items())
    ]
    payload = {"version": 1, "entries": entries}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"{json.dumps(payload, indent=2)}\n", encoding="utf-8")


def _print_summary(counts: dict[tuple[str, str], int]) -> None:
    by_token = {token: 0 for token in TOKENS}
    for (_, token), count in counts.items():
        by_token[token] = by_token.get(token, 0) + count
    summary = ", ".join(f"{token}={by_token.get(token, 0)}" for token in TOKENS)
    print(f"production-token-guardrail: scanned token counts -> {summary}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Guardrail for production-path usage of stub/demo/fallback tokens."
    )
    parser.add_argument(
        "--write-allowlist",
        action="store_true",
        help="Write/update the allowlist from current repository state.",
    )
    parser.add_argument(
        "--allowlist",
        default=str(ALLOWLIST_PATH),
        help=f"Allowlist file path (default: {ALLOWLIST_PATH})",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    allowlist_path = root / args.allowlist
    counts = _scan(root)
    _print_summary(counts)

    if args.write_allowlist:
        _write_allowlist(allowlist_path, counts)
        print(f"production-token-guardrail: wrote allowlist -> {allowlist_path.relative_to(root)}")
        return 0

    if not allowlist_path.exists():
        print(
            f"production-token-guardrail: missing allowlist at {allowlist_path.relative_to(root)}",
            file=sys.stderr,
        )
        return 1

    allowlist = _load_allowlist(allowlist_path)
    violations: list[str] = []

    for key, count in sorted(counts.items()):
        if key not in allowlist:
            rel, token = key
            violations.append(
                f"new token usage: {rel} token={token} count={count} (not allowlisted)"
            )
            continue
        max_count = allowlist[key]
        if count > max_count:
            rel, token = key
            violations.append(
                f"token count increased: {rel} token={token} current={count} allowlisted={max_count}"
            )

    if violations:
        print("production-token-guardrail: FAIL", file=sys.stderr)
        for violation in violations:
            print(f"  - {violation}", file=sys.stderr)
        return 1

    print("production-token-guardrail: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
