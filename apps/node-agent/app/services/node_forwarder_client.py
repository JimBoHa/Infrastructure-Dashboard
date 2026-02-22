from __future__ import annotations

import logging
from typing import Any, Dict, List, Optional

import httpx

logger = logging.getLogger(__name__)


class NodeForwarderClient:
    """Best-effort HTTP client for the local Rust node-forwarder."""

    def __init__(
        self,
        base_url: str,
        *,
        timeout_seconds: float = 2.0,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        timeout = httpx.Timeout(timeout_seconds)
        self._client = httpx.AsyncClient(timeout=timeout)

    async def aclose(self) -> None:
        await self._client.aclose()

    async def push_samples(self, samples: List[Dict[str, Any]]) -> int:
        if not samples:
            return 0
        resp = await self._client.post(f"{self.base_url}/v1/samples", json={"samples": samples})
        resp.raise_for_status()
        data = resp.json()
        try:
            return int(data.get("accepted") or 0)
        except Exception:
            return 0

    async def get_status(self) -> Optional[Dict[str, Any]]:
        try:
            resp = await self._client.get(f"{self.base_url}/v1/status")
            resp.raise_for_status()
            data = resp.json()
            if isinstance(data, dict):
                return data
        except Exception as exc:
            logger.debug("node-forwarder status unavailable: %s", exc)
        return None

