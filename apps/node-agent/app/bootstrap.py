from __future__ import annotations

import json
import logging

from app.config import Settings

logger = logging.getLogger(__name__)


def apply_firstboot(settings: Settings) -> None:
    """Apply one-time settings from a first-boot JSON file produced by imaging."""

    path = settings.firstboot_file
    if not path.exists():
        return
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError:
        logger.warning("First-boot config at %s is invalid JSON; skipping", path)
        return
    node_cfg = data.get("node") or {}
    if node_cfg.get("node_id"):
        settings.node_id = str(node_cfg["node_id"])
    if node_cfg.get("node_name"):
        settings.node_name = str(node_cfg["node_name"])
    if node_cfg.get("adoption_token"):
        settings.adoption_token = str(node_cfg["adoption_token"])
    wifi_cfg = data.get("wifi") or {}
    if wifi_cfg.get("ssid"):
        settings.wifi_hints = {
            "ssid": wifi_cfg.get("ssid"),
            "password": wifi_cfg.get("password"),
        }
    try:
        path.unlink()
    except OSError:
        logger.warning("Unable to remove first-boot config at %s", path)

