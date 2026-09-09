from __future__ import annotations

import json
import threading
from typing import Any

from .errors import OdcApiError

_PRINT_LOCK = threading.Lock()


def print_json(payload: Any) -> None:
    with _PRINT_LOCK:
        print(json.dumps(payload, indent=2, sort_keys=True))


def compact_dict(payload: dict[str, Any], fields: list[str]) -> dict[str, Any]:
    return {field: payload.get(field) for field in fields if payload.get(field) is not None}


def require_key(value: str | None, label: str) -> str:
    if not value:
        raise OdcApiError(f"{label} is required")
    return value
