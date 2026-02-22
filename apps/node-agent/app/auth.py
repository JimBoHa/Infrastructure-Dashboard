from __future__ import annotations

import hmac
from typing import Optional

from fastapi import Depends, HTTPException, Request, status

from app.config import Settings, get_settings


def _extract_bearer_token(request: Request) -> Optional[str]:
    header = request.headers.get("authorization") or request.headers.get("Authorization") or ""
    if not header:
        return None
    if not header.lower().startswith("bearer "):
        return None
    token = header.split(" ", 1)[1].strip()
    return token or None


def _allowed_tokens(settings: Settings) -> list[str]:
    tokens: list[str] = []
    if settings.provisioning_secret:
        value = settings.provisioning_secret.get_secret_value().strip()
        if value:
            tokens.append(value)
    if settings.adoption_token:
        value = str(settings.adoption_token).strip()
        if value:
            tokens.append(value)
    return tokens


def require_node_auth(
    request: Request,
    settings: Settings = Depends(get_settings),
) -> None:
    """Require a bearer token for node-agent configuration/provisioning endpoints.

    This intentionally supports either:
      - NODE_PROVISIONING_SECRET (preferred), or
      - the controller-issued adoption token stored on the node (Settings.adoption_token).
    """

    token = _extract_bearer_token(request)
    if not token:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing bearer token",
        )

    for allowed in _allowed_tokens(settings):
        if hmac.compare_digest(token, allowed):
            return

    raise HTTPException(
        status_code=status.HTTP_403_FORBIDDEN,
        detail="Invalid token",
    )

