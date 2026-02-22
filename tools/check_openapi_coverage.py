#!/usr/bin/env python3
"""
Check that the Rust core-server router covers the OpenAPI contract.

This is intentionally heuristic (regex + small parser) but aims to be stable
for this repo's routing patterns:
- Rust routes use axum Router::route("/path", get(handler).post(...)) patterns.
- Path params in Rust may be written as ":id" while OpenAPI uses "{id}".
- All routes under apps/core-server-rs/src/routes are nested under "/api"
  except routes/health.rs which is mounted at the root.

Exit codes:
  0 - coverage OK
  2 - coverage mismatch
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator, NamedTuple


class Route(NamedTuple):
    method: str  # GET/POST/PUT/DELETE/PATCH
    path: str    # /api/...


HTTP_METHODS = ("get", "post", "put", "delete", "patch")


def _normalize_rust_path(path: str) -> str:
    path = path.strip()
    if not path.startswith("/"):
        path = "/" + path
    parts = []
    for segment in path.split("/"):
        if segment.startswith(":") and len(segment) > 1:
            parts.append("{" + segment[1:] + "}")
        else:
            parts.append(segment)
    return "/".join(parts).replace("//", "/")


def _iter_openapi_routes(openapi_json: dict) -> set[Route]:
    routes: set[Route] = set()
    for path, methods in (openapi_json.get("paths") or {}).items():
        if not isinstance(methods, dict):
            continue
        for method in methods.keys():
            if method.lower() not in HTTP_METHODS:
                continue
            routes.add(Route(method.upper(), path))
    return routes


@dataclass
class ParsedRouteCall:
    path: str
    expr: str


def _strip_rust_comments(source: str) -> str:
    # Remove // line comments and /* */ block comments (best-effort), while
    # preserving comment markers inside string literals (e.g. "http://...").
    out: list[str] = []
    i = 0
    in_string = False
    in_line_comment = False
    in_block_comment = False
    while i < len(source):
        if in_line_comment:
            if source[i] == "\n":
                in_line_comment = False
                out.append("\n")
            i += 1
            continue
        if in_block_comment:
            if source.startswith("*/", i):
                in_block_comment = False
                i += 2
                continue
            i += 1
            continue
        if in_string:
            ch = source[i]
            out.append(ch)
            if ch == "\\" and i + 1 < len(source):
                out.append(source[i + 1])
                i += 2
                continue
            if ch == '"':
                in_string = False
            i += 1
            continue

        if source.startswith("//", i):
            in_line_comment = True
            i += 2
            continue
        if source.startswith("/*", i):
            in_block_comment = True
            i += 2
            continue
        ch = source[i]
        if ch == '"':
            in_string = True
        out.append(ch)
        i += 1

    return "".join(out)


def _iter_route_calls(source: str) -> Iterator[ParsedRouteCall]:
    source = _strip_rust_comments(source)
    idx = 0
    while True:
        start = source.find(".route(", idx)
        if start == -1:
            return
        i = start + len(".route(")
        # Skip whitespace
        while i < len(source) and source[i].isspace():
            i += 1
        if i >= len(source) or source[i] != '"':
            idx = i
            continue
        # Parse string literal path
        i += 1
        path_chars: list[str] = []
        while i < len(source):
            ch = source[i]
            if ch == "\\":
                if i + 1 < len(source):
                    path_chars.append(source[i + 1])
                    i += 2
                    continue
            if ch == '"':
                i += 1
                break
            path_chars.append(ch)
            i += 1
        path = "".join(path_chars)
        # Find comma after first arg
        while i < len(source) and source[i].isspace():
            i += 1
        if i >= len(source) or source[i] != ",":
            idx = i
            continue
        i += 1
        # Parse the method router expression until the matching closing paren of `.route(...)`.
        depth = 1
        expr_start = i
        in_string = False
        while i < len(source):
            ch = source[i]
            if in_string:
                if ch == "\\":
                    i += 2
                    continue
                if ch == '"':
                    in_string = False
                i += 1
                continue
            if ch == '"':
                in_string = True
                i += 1
                continue
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
                if depth == 0:
                    expr = source[expr_start:i].strip()
                    yield ParsedRouteCall(path=path, expr=expr)
                    i += 1
                    break
            i += 1
        idx = i


def _methods_from_route_expr(expr: str) -> set[str]:
    methods: set[str] = set()
    for match in re.finditer(r"(?<![A-Za-z0-9_])(get|post|put|delete|patch)\s*\(", expr):
        methods.add(match.group(1).upper())
    # Also catch chained forms like `.put(...)`
    for match in re.finditer(r"\.(get|post|put|delete|patch)\s*\(", expr):
        methods.add(match.group(1).upper())
    return methods


def _iter_rust_routes(routes_dir: Path) -> set[Route]:
    routes: set[Route] = set()

    mod_rs = routes_dir / "mod.rs"
    included_modules: set[str] = set()
    if mod_rs.exists():
        source = _strip_rust_comments(mod_rs.read_text(encoding="utf-8", errors="ignore"))
        for match in re.finditer(r"\.merge\(\s*([A-Za-z0-9_]+)::router\(\)\s*\)", source):
            included_modules.add(match.group(1))
        for match in re.finditer(
            r"\.nest\(\s*\"[^\"]+\"\s*,\s*([A-Za-z0-9_]+)::router\(\)\s*\)",
            source,
        ):
            included_modules.add(match.group(1))

    candidates: list[Path] = []
    if included_modules:
        for module in sorted(included_modules):
            path = routes_dir / f"{module}.rs"
            if path.exists():
                candidates.append(path)
    else:
        candidates = [p for p in sorted(routes_dir.glob("*.rs")) if p.name != "mod.rs"]

    for path in candidates:
        source = path.read_text(encoding="utf-8", errors="ignore")
        prefix = "" if path.name == "health.rs" else "/api"
        for call in _iter_route_calls(source):
            methods = _methods_from_route_expr(call.expr)
            if not methods:
                continue
            normalized_path = _normalize_rust_path(call.path)
            for method in methods:
                routes.add(Route(method, f"{prefix}{normalized_path}".replace("//", "/")))
    return routes


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--openapi",
        default="apps/core-server-rs/openapi/farm-dashboard.json",
        help="Path to OpenAPI JSON file",
    )
    parser.add_argument(
        "--routes-dir",
        default="apps/core-server-rs/src/routes",
        help="Rust routes directory (axum Router modules)",
    )
    parser.add_argument(
        "--allow-extra",
        action="store_true",
        help=(
            "Allow router endpoints not present in the OpenAPI contract (legacy behavior). "
            "Default is strict: extra endpoints fail the check."
        ),
    )
    args = parser.parse_args(argv)

    openapi_path = Path(args.openapi)
    routes_dir = Path(args.routes_dir)
    openapi = json.loads(openapi_path.read_text(encoding="utf-8"))

    expected = _iter_openapi_routes(openapi)
    actual = _iter_rust_routes(routes_dir)

    missing = sorted(expected - actual, key=lambda r: (r.path, r.method))
    extra = sorted(actual - expected, key=lambda r: (r.path, r.method))

    allow_extra = bool(args.allow_extra)
    is_ok = not missing and (allow_extra or not extra)
    if is_ok:
        print("OpenAPI coverage OK")
        return 0

    print("OpenAPI coverage mismatch")
    if missing:
        print(f"Missing ({len(missing)}):")
        for route in missing:
            print(f"  {route.method} {route.path}")
    if extra:
        header = "Extra (ignored)" if allow_extra else "Extra"
        print(f"{header} ({len(extra)}):")
        for route in extra[:50]:
            print(f"  {route.method} {route.path}")
        if len(extra) > 50:
            print(f"  ... and {len(extra) - 50} more")
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
