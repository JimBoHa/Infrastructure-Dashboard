"""Persist provisioning requests (e.g., BLE handoff) for later processing."""
from __future__ import annotations

import json
import base64
import hashlib
import logging
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List
from uuid import uuid4

from cryptography.fernet import Fernet, InvalidToken
logger = logging.getLogger(__name__)
_ENCRYPTED_PREFIX = "enc:"

@dataclass
class ProvisioningRequest:
    session_id: str
    device_name: str
    pin: str | None
    wifi_ssid: str | None
    wifi_password: str | None
    node_name: str | None
    adoption_token: str | None
    mesh_join_code: str | None
    preferred_protocol: str | None
    requested_at: str
    status: str = "queued"
    message: str | None = None
    last_event_at: str | None = None


class ProvisioningStore:
    """File-backed queue of provisioning requests."""

    def __init__(self, path: Path, *, secret: str | None = None, key_path: Path | None = None):
        self.path = path
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._secret = secret
        self._key_path = key_path or path.with_suffix(".key")
        self._fernet: Fernet | None = None

    def append(
        self,
        *,
        device_name: str,
        pin: str | None,
        wifi_ssid: str | None,
        wifi_password: str | None,
        node_name: str | None,
        adoption_token: str | None,
        mesh_join_code: str | None = None,
        preferred_protocol: str | None = None,
        status: str = "queued",
        message: str | None = None,
    ) -> ProvisioningRequest:
        session_id = uuid4().hex
        record = ProvisioningRequest(
            session_id=session_id,
            device_name=device_name,
            pin=pin,
            wifi_ssid=wifi_ssid,
            wifi_password=wifi_password,
            node_name=node_name,
            adoption_token=adoption_token,
            mesh_join_code=mesh_join_code,
            preferred_protocol=preferred_protocol,
            requested_at=datetime.now(timezone.utc).isoformat(),
            status=status,
            message=message,
        )
        items = self._read_all()
        items.append(record)
        self._write(items)
        return record

    def update_status(
        self,
        session_id: str,
        status: str,
        message: str | None = None,
        updates: Dict[str, object] | None = None,
    ) -> ProvisioningRequest | None:
        items = self._read_all()
        updated: ProvisioningRequest | None = None
        updates = updates or {}
        for idx, record in enumerate(items):
            if record.session_id != session_id:
                continue
            updated = ProvisioningRequest(
                **{
                    **record.__dict__,
                    "status": status,
                    "message": message,
                    "last_event_at": datetime.now(timezone.utc).isoformat(),
                    **updates,
                }
            )
            items[idx] = updated
            break
        if updated:
            self._write(items)
        return updated

    def get(self, session_id: str) -> ProvisioningRequest | None:
        for item in self._read_all():
            if item.session_id == session_id:
                return item
        return None

    def all(self) -> List[ProvisioningRequest]:
        return self._read_all()

    def clear(self) -> None:
        """Remove all queued provisioning requests."""

        self._write([])

    # Internal helpers -----------------------------------------------------
    def _read_all(self) -> List[ProvisioningRequest]:
        if not self.path.exists():
            return []
        try:
            payloads = json.loads(self.path.read_text()) or []
        except json.JSONDecodeError:
            return []
        records: List[ProvisioningRequest] = []
        for item in payloads:
            try:
                defaults = {
                    "status": "queued",
                    "message": None,
                    "last_event_at": None,
                }
                defaults.update(item)
                if defaults.get("wifi_password"):
                    defaults["wifi_password"] = self._decrypt_secret(
                        str(defaults["wifi_password"])
                    )
                records.append(ProvisioningRequest(**defaults))
            except TypeError:
                continue
        return records

    def _write(self, items: List[ProvisioningRequest]) -> None:
        payload: List[Dict[str, object]] = []
        for item in items:
            record = dict(item.__dict__)
            if record.get("wifi_password"):
                record["wifi_password"] = self._encrypt_secret(str(record["wifi_password"]))
            payload.append(record)
        self.path.write_text(json.dumps(payload, indent=2))

    def _get_fernet(self) -> Fernet:
        if self._fernet is None:
            key = self._load_or_create_key()
            self._fernet = Fernet(key)
        return self._fernet

    def _load_or_create_key(self) -> bytes:
        if self._secret:
            return self._normalize_secret(self._secret)
        if self._key_path.exists():
            content = self._key_path.read_text().strip()
            if content:
                return self._normalize_secret(content)
        key = Fernet.generate_key()
        self._key_path.write_text(key.decode("ascii"))
        try:
            self._key_path.chmod(0o600)
        except Exception:
            logger.debug("Unable to set permissions on provisioning key file")
        return key

    def _normalize_secret(self, secret: str) -> bytes:
        raw = secret.strip()
        if not raw:
            raise ValueError("Provisioning secret is empty")
        try:
            decoded = base64.urlsafe_b64decode(raw + "=" * (-len(raw) % 4))
            if len(decoded) == 32:
                return base64.urlsafe_b64encode(decoded)
        except Exception:
            pass
        digest = hashlib.sha256(raw.encode("utf-8")).digest()
        return base64.urlsafe_b64encode(digest)

    def _encrypt_secret(self, value: str) -> str:
        if value.startswith(_ENCRYPTED_PREFIX):
            token = value[len(_ENCRYPTED_PREFIX) :]
            try:
                self._get_fernet().decrypt(token.encode("ascii"))
                return value
            except InvalidToken:
                pass
        token = self._get_fernet().encrypt(value.encode("utf-8")).decode("ascii")
        return f"{_ENCRYPTED_PREFIX}{token}"

    def _decrypt_secret(self, value: str) -> str | None:
        if not value:
            return None
        if not value.startswith(_ENCRYPTED_PREFIX):
            return value
        token = value[len(_ENCRYPTED_PREFIX) :]
        try:
            return self._get_fernet().decrypt(token.encode("ascii")).decode("utf-8")
        except (InvalidToken, ValueError) as exc:
            logger.warning("Unable to decrypt provisioning secret: %s", exc)
            return None
