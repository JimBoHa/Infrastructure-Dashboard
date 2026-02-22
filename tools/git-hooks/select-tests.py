#!/usr/bin/env python3
from __future__ import annotations

import re
import subprocess
import sys

DOC_LOG_REGEX = re.compile(
    r"^(docs/|project_management/|reports/|manual_screenshots_|"
    r"README\.md$|AGENTS\.md$|CODEOWNERS$|guidelines\.md$|"
    r"GAP_ANALYSIS_.*\.md$|critical_project_planning_failures\.md$|"
    r".*\.(md|markdown|mdx|rst|adoc|asciidoc|txt|log|out|err|pdf|"
    r"jpg|jpeg|png|gif|webp|svg|bmp|tiff|tif|ico|heic|heif|avif)$)",
    re.IGNORECASE,
)

WEB_PREFIXES = (
    "apps/dashboard-web/",
)

CORE_PREFIXES = ("infra/", "proto/")
NODE_PREFIXES = ("apps/node-agent/", "apps/telemetry-sidecar/")
FARMCTL_PREFIXES = ("apps/farmctl/",)
RCS_PREFIXES = ("apps/core-server-rs/",)
INTEGRITY_GUARDRAIL_PREFIXES = (
    "apps/core-server-rs/src/routes/",
    "apps/farmctl/src/",
    "apps/node-agent/app/",
)
RCS_OPENAPI_PREFIXES = (
    "apps/core-server-rs/src/routes/",
    "apps/core-server-rs/src/openapi.rs",
    "apps/core-server-rs/openapi/",
    "tools/api-sdk/",
)


def staged_files() -> list[str]:
    result = subprocess.run(
        ["git", "diff", "--cached", "--name-only", "--diff-filter=ACMRD"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def run(cmd: list[str]) -> None:
    print(f"+ {' '.join(cmd)}")
    subprocess.run(cmd, check=True)


def main() -> int:
    paths = staged_files()
    if not paths:
        return 0

    non_doc = [path for path in paths if not DOC_LOG_REGEX.match(path)]
    if not non_doc:
        print("pre-commit: doc/log/image-only change detected; skipping E2E.")
        return 0

    needs_web = False
    needs_core = False
    needs_node = False
    needs_farmctl = False
    needs_rcs = False
    needs_rcs_openapi = False
    needs_integrity_guardrail = False
    unknown = []

    for path in non_doc:
        if path == "apps/dashboard-web/src/lib/api.ts" or any(
            path.startswith(prefix) for prefix in INTEGRITY_GUARDRAIL_PREFIXES
        ):
            needs_integrity_guardrail = True
        if any(path.startswith(prefix) for prefix in RCS_OPENAPI_PREFIXES):
            needs_rcs_openapi = True
        if any(path.startswith(prefix) for prefix in RCS_PREFIXES):
            needs_rcs = True
            continue
        if any(path.startswith(prefix) for prefix in FARMCTL_PREFIXES):
            needs_farmctl = True
            continue
        if any(path.startswith(prefix) for prefix in CORE_PREFIXES):
            needs_core = True
            continue
        if any(path.startswith(prefix) for prefix in NODE_PREFIXES):
            needs_node = True
            continue
        if any(path.startswith(prefix) for prefix in WEB_PREFIXES):
            needs_web = True
            continue
        unknown.append(path)

    if unknown:
        needs_core = True
        needs_node = True
        needs_web = True
        needs_farmctl = True

    if needs_integrity_guardrail:
        run(["make", "ci-integrity-guardrail"])

    if needs_farmctl:
        run(["make", "ci-farmctl-smoke"])

    if needs_rcs_openapi:
        run(["make", "rcs-openapi-coverage"])

    if needs_rcs:
        run(["cargo", "test", "--manifest-path", "apps/core-server-rs/Cargo.toml"])

    # Pre-commit intentionally uses lightweight CI-smoke targets.
    # For dashboard-web changes, include a Next build to catch TS/build-time regressions.
    # DMG/E2E flows are stateful (require an installed temp root) and should be run explicitly:
    #   FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke && make e2e-web-smoke
    if needs_core and needs_node and needs_web:
        run(["make", "ci-smoke"])
    else:
        if needs_core:
            run(["make", "ci-core-smoke"])
        if needs_node:
            run(["make", "ci-node-smoke"])
        if needs_web:
            run(["make", "ci-web-smoke-build"])

    return 0


if __name__ == "__main__":
    sys.exit(main())
