#!/usr/bin/env python3
from __future__ import annotations

import os
import sys
from datetime import datetime, timezone
from pathlib import Path


def main() -> int:
    backup_root = Path(os.environ.get("NODE_AGENT_BACKUP_ROOT", "/opt/node-agent/storage/backups"))
    max_age_hours = float(os.environ.get("NODE_AGENT_BACKUP_MAX_AGE_HOURS", "36"))

    if not backup_root.exists():
        print(f"[node-agent] Backup root {backup_root} is missing", file=sys.stderr)
        return 2

    backups = sorted(
        (p for p in backup_root.rglob("*.json") if p.is_file()),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )

    if not backups:
        print(f"[node-agent] No backups found under {backup_root}", file=sys.stderr)
        return 3

    latest = backups[0]
    modified = datetime.fromtimestamp(latest.stat().st_mtime, tz=timezone.utc)
    age_hours = (datetime.now(tz=timezone.utc) - modified).total_seconds() / 3600.0

    if age_hours > max_age_hours:
        print(
            (
                f"[node-agent] Latest backup {latest} is {age_hours:.1f}h old "
                f"(threshold {max_age_hours:.1f}h)"
            ),
            file=sys.stderr,
        )
        return 4

    size_bytes = latest.stat().st_size
    print(
        (
            f"[node-agent] Latest backup {latest.name} age={age_hours:.2f}h "
            f"size={size_bytes}B within threshold {max_age_hours:.1f}h"
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
