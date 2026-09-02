from __future__ import annotations

import re
import sys

from .client import OdcClient
from .errors import OdcApiError
from .utils import require_key

GUID_PATTERN = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)


def resolve_asset_key(client: OdcClient, input_key: str) -> str:
    if GUID_PATTERN.match(input_key):
        return input_key

    matches = client.search_asset(input_key)
    if not matches:
        raise OdcApiError(f"No assets found matching '{input_key}'")

    exact_matches = [
        a for a in matches if a.get("name", "").lower() == input_key.lower()
    ]

    if len(exact_matches) == 1:
        asset = exact_matches[0]
        key = asset.get("assetKey")
        return require_key(key, "asset key")
    elif len(exact_matches) > 1:
        print("error: Multiple assets match the name (ambiguous):", file=sys.stderr)
        for asset in exact_matches:
            print(
                f"  - {asset.get('name')} ({asset.get('assetKey')})",
                file=sys.stderr,
            )
        sys.exit(1)
    else:
        print(
            f"error: No exact match for '{input_key}'. Did you mean:",
            file=sys.stderr,
        )
        for asset in matches[:10]:
            print(
                f"  - {asset.get('name')} ({asset.get('assetKey')})",
                file=sys.stderr,
            )
        if len(matches) > 10:
            print(f"  ... and {len(matches) - 10} more", file=sys.stderr)
        sys.exit(1)


def resolve_environment_key(client: OdcClient, input_env: str) -> str:
    if GUID_PATTERN.match(input_env):
        return input_env

    environments = client._request("GET", client.url("portfolios", "/environments"))
    results = environments.get("results") or []

    matches = [
        e
        for e in results
        if input_env.lower() in e.get("name", "").lower()
        or input_env.lower() in e.get("key", "").lower()
    ]

    exact_matches = [e for e in matches if e.get("name", "").lower() == input_env.lower()]

    if len(exact_matches) == 1:
        return require_key(exact_matches[0].get("key"), "environment key")
    elif len(exact_matches) > 1:
        print("error: Multiple environments match the name (ambiguous):", file=sys.stderr)
        for env in exact_matches:
            print(f"  - {env.get('name')} ({env.get('key')})", file=sys.stderr)
        sys.exit(1)
    elif len(matches) == 1:
        return require_key(matches[0].get("key"), "environment key")
    elif not matches:
        print(f"error: No environments found matching '{input_env}'", file=sys.stderr)
        print("Available environments:", file=sys.stderr)
        for env in results:
            print(f"  - {env.get('name')} ({env.get('key')})", file=sys.stderr)
        sys.exit(1)
    else:
        print(f"error: No exact match for '{input_env}'. Did you mean:", file=sys.stderr)
        for env in matches[:10]:
            print(f"  - {env.get('name')} ({env.get('key')})", file=sys.stderr)
        if len(matches) > 10:
            print(f"  ... and {len(matches) - 10} more", file=sys.stderr)
        sys.exit(1)


def resolve_user_key(client: OdcClient, input_user: str) -> str:
    if GUID_PATTERN.match(input_user):
        return input_user

    users = client.query_users(name_or_email_contains=input_user)
    if not users:
        raise OdcApiError(f"No users found matching '{input_user}'")

    exact_email_matches = [u for u in users if u.get("email", "").lower() == input_user.lower()]
    if len(exact_email_matches) == 1:
        user_key = exact_email_matches[0].get("key")
        return require_key(user_key, "user key")

    exact_name_matches = [u for u in users if u.get("name", "").lower() == input_user.lower()]
    if len(exact_name_matches) == 1:
        user_key = exact_name_matches[0].get("key")
        return require_key(user_key, "user key")

    if len(users) == 1:
        user_key = users[0].get("key")
        return require_key(user_key, "user key")

    print(f"error: No exact match for '{input_user}'. Did you mean:", file=sys.stderr)
    for user in users[:10]:
        print(f"  - {user.get('name', 'Unknown')} ({user.get('email', 'Unknown')})", file=sys.stderr)
    if len(users) > 10:
        print(f"  ... and {len(users) - 10} more", file=sys.stderr)
    sys.exit(1)
