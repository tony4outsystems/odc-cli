from __future__ import annotations

import os
from dataclasses import dataclass

from dotenv import load_dotenv

from .errors import OdcApiError


@dataclass(frozen=True)
class Settings:
    tenant_url: str
    client_id: str
    client_secret: str
    scope: str | None = None

    @property
    def tenant_origin(self) -> str:
        return self.tenant_url.rstrip("/")


def load_settings() -> Settings:
    load_dotenv()
    required = ["ODC_TENANT_URL", "ODC_CLIENT_ID", "ODC_CLIENT_SECRET"]
    missing = [name for name in required if not os.environ.get(name)]
    if missing:
        raise OdcApiError(f"Missing required environment variables: {', '.join(missing)}")

    return Settings(
        tenant_url=os.environ["ODC_TENANT_URL"],
        client_id=os.environ["ODC_CLIENT_ID"],
        client_secret=os.environ["ODC_CLIENT_SECRET"],
        scope=os.environ.get("ODC_SCOPE"),
    )
