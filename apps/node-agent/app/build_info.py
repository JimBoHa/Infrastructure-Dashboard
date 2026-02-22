from __future__ import annotations

from typing import Literal

# This file is overwritten during packaging so production artifacts bake the build flavor
# into the installed node-agent. Local development defaults to "dev".
BUILD_FLAVOR: Literal["prod", "dev", "test"] = "dev"

