from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx
from dotenv import load_dotenv


BUILD_TERMINAL_STATUSES = {"Finished", "FinishedWithErrors", "Deleted", "ToBeDeleted"}
OPERATION_TERMINAL_STATUSES = {"Finished", "FinishedWithError"}
API_BASE_PATHS = {
    "asset-repository": "/api/asset-repository/v1",
    "builds": "/api/builds/v1",
    "dependency-management": "/api/dependency-management/v1",
    "deployments": "/api/deployments/v1",
    "portfolios": "/api/portfolios/v2",
    "identity": "/api/identity/v1",
}
GUID_PATTERN = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)


class OdcApiError(RuntimeError):
    pass


@dataclass(frozen=True)
class Settings:
    tenant_url: str
    client_id: str
    client_secret: str
    asset_key: str
    environment_key: str
    scope: str | None = None

    @property
    def tenant_origin(self) -> str:
        return self.tenant_url.rstrip("/")


def load_settings() -> Settings:
    load_dotenv()
    required = [
        "ODC_TENANT_URL",
        "ODC_CLIENT_ID",
        "ODC_CLIENT_SECRET",
        "ODC_ENVIRONMENT_KEY",
    ]
    missing = [name for name in required if not os.environ.get(name)]
    if missing:
        raise OdcApiError(f"Missing required environment variables: {', '.join(missing)}")

    return Settings(
        tenant_url=os.environ["ODC_TENANT_URL"],
        client_id=os.environ["ODC_CLIENT_ID"],
        client_secret=os.environ["ODC_CLIENT_SECRET"],
        asset_key=os.environ.get("ODC_ASSET_KEY", ""),
        environment_key=os.environ["ODC_ENVIRONMENT_KEY"],
        scope=os.environ.get("ODC_SCOPE"),
    )


class OdcClient:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.http = httpx.Client(timeout=httpx.Timeout(60.0))
        self._token: str | None = None
        self._discovery: dict[str, Any] | None = None

    def close(self) -> None:
        self.http.close()

    def __enter__(self) -> OdcClient:
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def discover(self) -> dict[str, Any]:
        if self._discovery is None:
            url = f"{self.settings.tenant_origin}/identity/.well-known/openid-configuration"
            self._discovery = self._request("GET", url, auth=False)
        return self._discovery

    def token(self) -> str:
        if self._token is None:
            discovery = self.discover()
            token_endpoint = discovery.get("token_endpoint")
            if not token_endpoint:
                raise OdcApiError("Discovery document does not include token_endpoint")

            data = {
                "grant_type": "client_credentials",
                "client_id": self.settings.client_id,
                "client_secret": self.settings.client_secret,
            }
            if self.settings.scope:
                data["scope"] = self.settings.scope

            response = self._request("POST", token_endpoint, data=data, auth=False)
            access_token = response.get("access_token")
            if not access_token:
                raise OdcApiError("Token response did not include access_token")
            self._token = access_token
        return self._token

    def latest_revision(self, asset_key: str) -> int:
        revision = self._request(
            "GET",
            self.url("asset-repository", f"/assets/{asset_key}/latest-revision"),
        )
        revision_number = revision.get("revision")
        if not isinstance(revision_number, int):
            raise OdcApiError(f"Latest revision response did not include an integer revision: {revision}")
        return revision_number

    def get_asset(self, asset_key: str) -> dict[str, Any]:
        return self._request("GET", self.url("asset-repository", f"/assets/{asset_key}"))

    def get_environment(self, environment_key: str) -> dict[str, Any]:
        environments = self._request("GET", self.url("portfolios", "/environments"))
        for environment in environments.get("results") or []:
            if environment.get("key") == environment_key:
                return environment
        raise OdcApiError(f"Environment key was not found or is not visible: {environment_key}")

    def list_assets(self) -> list[dict[str, Any]]:
        response = self._request("GET", self.url("asset-repository", "/assets"))
        return response.get("results") or []

    def search_asset(self, query: str) -> list[dict[str, Any]]:
        results = self.list_assets()
        query_lower = query.lower()
        return [
            asset
            for asset in results
            if query_lower in asset.get("name", "").lower()
            or query_lower in asset.get("assetKey", "").lower()
        ]

    def start_build(self, asset_key: str, revision: int, build_type: str) -> dict[str, Any]:
        return self._request(
            "POST",
            self.url("builds", "/build-operations"),
            json_data={
                "assetKey": asset_key,
                "assetRevision": revision,
                "buildType": build_type,
            },
        )

    def get_build(self, build_key: str) -> dict[str, Any]:
        return self._request("GET", self.url("builds", f"/build-operations/{build_key}"))

    def publish(self, asset_key: str, revision: int, environment_key: str | None) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "operation": "Publish",
            "assetKey": asset_key,
            "revision": revision,
        }
        if environment_key:
            payload["environmentKey"] = environment_key
        return self._request("POST", self.url("deployments", "/publish-operations"), json_data=payload)

    def get_publish(self, operation_key: str) -> dict[str, Any]:
        return self._request("GET", self.url("deployments", f"/publish-operations/{operation_key}"))

    def deploy(self, asset_key: str, revision: int, build_key: str, environment_key: str) -> dict[str, Any]:
        return self._request(
            "POST",
            self.url("deployments", "/deployment-operations"),
            json_data={
                "operation": "Deploy",
                "assetKey": asset_key,
                "buildKey": build_key,
                "revision": revision,
                "environmentKey": environment_key,
            },
        )

    def get_deployment(self, operation_key: str) -> dict[str, Any]:
        return self._request("GET", self.url("deployments", f"/deployment-operations/{operation_key}"))

    def producer_graph(
        self,
        asset_key: str,
        revision: int,
        *,
        environment_key: str | None = None,
        max_depth: int = 0,
        producer_type_filter: str = "Deployable",
    ) -> dict[str, Any]:
        params: dict[str, Any] = {
            "maxDepth": max_depth,
            "producerTypeFilter": producer_type_filter,
            "sort": "name",
        }
        if environment_key:
            params["environmentKey"] = environment_key
        return self._request(
            "GET",
            self.url(
                "dependency-management",
                f"/assets/{asset_key}/revisions/{revision}/producer-graph",
            ),
            params=params,
        )

    def get_user(self, user_key: str) -> dict[str, Any]:
        return self._request("GET", self.url("identity", f"/users/{user_key}"))

    def update_user(
        self,
        user_key: str,
        *,
        name: str | None = None,
        is_active: bool | None = None,
        photo_url: str | None = None,
    ) -> None:
        payload: dict[str, Any] = {}
        if name is not None:
            payload["name"] = name
        if is_active is not None:
            payload["isActive"] = is_active
        if photo_url is not None:
            payload["photoUrl"] = photo_url
        self._request("PATCH", self.url("identity", f"/users/{user_key}"), json_data=payload)

    def query_users(self, *, name_or_email_contains: str | None = None, limit: int = 100) -> list[dict[str, Any]]:
        params: dict[str, Any] = {"limit": limit}
        if name_or_email_contains:
            params["nameOrEmailContains"] = name_or_email_contains
        response = self._request("GET", self.url("identity", "/users"), params=params)
        return response.get("results") or []

    def url(self, api: str, path: str) -> str:
        base_path = API_BASE_PATHS[api]
        return f"{self.settings.tenant_origin}{base_path}{path}"

    def _request(
        self,
        method: str,
        url: str,
        *,
        auth: bool = True,
        json_data: dict[str, Any] | None = None,
        data: dict[str, Any] | None = None,
        params: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        headers = {"Accept": "application/json"}
        if auth:
            headers["Authorization"] = f"Bearer {self.token()}"

        response = self.http.request(
            method,
            url,
            headers=headers,
            json=json_data,
            data=data,
            params=params,
        )
        if response.status_code >= 400:
            raise OdcApiError(format_error(response))
        if not response.content:
            return {}
        return response.json()


def format_error(response: httpx.Response) -> str:
    try:
        payload = response.json()
    except json.JSONDecodeError:
        payload = response.text
    return f"{response.request.method} {response.url} failed with {response.status_code}: {payload}"


def wait_for(
    label: str,
    fetch: Any,
    terminal_statuses: set[str],
    *,
    interval_seconds: float,
    timeout_seconds: float,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_seconds
    last_status: str | None = None

    while True:
        details = fetch()
        status = details.get("status")
        if status != last_status:
            with _PRINT_LOCK:
                print(f"{label}: {status}")
            last_status = status
        if status in terminal_statuses:
            return details
        if time.monotonic() >= deadline:
            raise OdcApiError(f"Timed out waiting for {label}; last response: {details}")
        time.sleep(interval_seconds)


_PRINT_LOCK = threading.Lock()


def print_json(payload: Any) -> None:
    with _PRINT_LOCK:
        print(json.dumps(payload, indent=2, sort_keys=True))


def compact_dict(payload: dict[str, Any], fields: list[str]) -> dict[str, Any]:
    return {field: payload.get(field) for field in fields if payload.get(field) is not None}


def mermaid_node_id(asset_key: str, revision: int | None) -> str:
    raw = f"{asset_key}:{revision if revision is not None else ''}"
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"asset_{digest}"


def mermaid_label(asset: dict[str, Any]) -> str:
    name = asset.get("name") or asset.get("key") or "Unknown asset"
    details = []
    if asset.get("revision") is not None:
        details.append(f"rev {asset['revision']}")
    if asset.get("type"):
        details.append(str(asset["type"]))
    label = str(name)
    if details:
        label = f"{label}\n{' / '.join(details)}"
    return label.replace("\\", "\\\\").replace('"', '\\"')


def render_producer_graph_mermaid(root: dict[str, Any], producers: list[dict[str, Any]]) -> str:
    nodes: dict[str, str] = {}
    edges: set[tuple[str, str]] = set()

    def add_node(asset: dict[str, Any]) -> str:
        key = str(asset.get("key") or "unknown")
        revision = asset.get("revision")
        node_id = mermaid_node_id(key, revision if isinstance(revision, int) else None)
        nodes[node_id] = mermaid_label(asset)
        return node_id

    def visit(parent: dict[str, Any], children: list[dict[str, Any]]) -> None:
        parent_id = add_node(parent)
        for child in children:
            child_id = add_node(child)
            edges.add((parent_id, child_id))
            visit(child, child.get("producers") or [])

    visit(root, producers)

    lines = [
        "---",
        "title: Producer dependency graph",
        "---",
        "flowchart LR",
    ]
    for node_id in sorted(nodes):
        lines.append(f'    {node_id}["{nodes[node_id]}"]')
    for parent_id, child_id in sorted(edges):
        lines.append(f"    {parent_id} --> {child_id}")
    return "\n".join(lines) + "\n"


def default_mermaid_output_path(asset_key: str, revision: int) -> Path:
    safe_asset_key = "".join(
        char if char.isalnum() or char in ("-", "_") else "_"
        for char in asset_key
    )
    return Path(f"producer-graph-{safe_asset_key}-rev-{revision}.mmd")


def print_preflight_summary(asset: dict[str, Any], environment: dict[str, Any], revision: int) -> None:
    print_json(
        {
            "preflight": {
                "asset": compact_dict(
                    asset,
                    [
                        "assetKey",
                        "name",
                        "assetType",
                        "revision",
                        "tag",
                        "portfolioKey",
                        "createdAt",
                        "createdBy",
                    ],
                ),
                "environment": compact_dict(
                    environment,
                    [
                        "key",
                        "name",
                        "purpose",
                        "defaultDomain",
                        "region",
                        "hosting",
                        "status",
                        "portfolioKey",
                    ],
                ),
                "selectedRevision": revision,
            }
        }
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


def print_dependency_summary(client: OdcClient, asset_key: str, revision: int, environment_key: str) -> None:
    graph = client.producer_graph(asset_key, revision, environment_key=environment_key)
    producers = graph.get("results") or []
    with _PRINT_LOCK:
        print(f"Dependencies ({len(producers)}):")
        for producer in producers:
            name = producer.get("name", "Unknown")
            producer_type = producer.get("type", "Unknown")
            status = producer.get("status", "Unknown")
            print(f"  - {name} ({producer_type}) - {status}")


def preflight(client: OdcClient, asset_key: str, environment_key: str, revision: int | None) -> int:
    asset = client.get_asset(asset_key)
    environment = client.get_environment(environment_key)
    resolved_revision = revision if revision is not None else asset.get("revision")
    if not isinstance(resolved_revision, int):
        resolved_revision = client.latest_revision(asset_key)
    print_preflight_summary(asset, environment, resolved_revision)
    return resolved_revision


def add_common_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--asset-key", default=None, help="Asset name or key. Defaults to ODC_ASSET_KEY.")
    parser.add_argument(
        "--environment-key",
        default=None,
        help="Environment name or key. Defaults to ODC_ENVIRONMENT_KEY.",
    )
    parser.add_argument("--revision", type=int, default=None)
    parser.add_argument("--poll-interval", type=float, default=10.0)
    parser.add_argument("--timeout", type=float, default=1800.0)


def require_key(value: str | None, label: str) -> str:
    if not value:
        raise OdcApiError(f"{label} is required")
    return value


def handle_discover(client: OdcClient, _args: argparse.Namespace) -> None:
    discovery = client.discover()
    print_json(
        {
            "issuer": discovery.get("issuer"),
            "token_endpoint": discovery.get("token_endpoint"),
            "scopes_supported": discovery.get("scopes_supported"),
        }
    )


def handle_latest_revision(client: OdcClient, args: argparse.Namespace) -> None:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    print(client.latest_revision(resolved_key))


def handle_validate(client: OdcClient, args: argparse.Namespace) -> None:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    environment_key = args.environment_key or client.settings.environment_key
    preflight(client, resolved_key, environment_key, args.revision)


def handle_build(client: OdcClient, args: argparse.Namespace) -> dict[str, Any]:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    environment_key = resolve_environment_key(client, args.environment_key or client.settings.environment_key)
    revision = preflight(client, resolved_key, environment_key, args.revision)
    response = client.start_build(resolved_key, revision, args.build_type)
    print_json(response)
    build_key = require_key(response.get("buildKey"), "buildKey")
    if args.no_wait:
        return response

    details = wait_for(
        f"build {build_key}",
        lambda: client.get_build(build_key),
        BUILD_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    print_json(details)
    if details.get("status") != "Finished":
        raise OdcApiError(f"Build did not finish successfully: {details.get('status')}")
    return details


def handle_publish(client: OdcClient, args: argparse.Namespace) -> dict[str, Any]:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    environment_key = resolve_environment_key(client, args.environment_key or client.settings.environment_key)
    revision = preflight(client, resolved_key, environment_key, args.revision)
    response = client.publish(resolved_key, revision, environment_key)
    print_json(response)
    operation_key = require_key(response.get("key"), "publish operation key")
    if args.no_wait:
        return response

    details = wait_for(
        f"publish {operation_key}",
        lambda: client.get_publish(operation_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    print_json(details)
    if details.get("status") != "Finished":
        raise OdcApiError(f"Publish did not finish successfully: {details.get('status')}")
    return details


def handle_deploy(client: OdcClient, args: argparse.Namespace) -> dict[str, Any]:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    environment_key = resolve_environment_key(client, args.environment_key or client.settings.environment_key)
    revision = preflight(client, resolved_key, environment_key, args.revision)
    build_key = require_key(args.build_key, "--build-key")
    response = client.deploy(resolved_key, revision, build_key, environment_key)
    print_json(response)
    operation_key = require_key(response.get("key"), "deployment operation key")
    if args.no_wait:
        return response

    details = wait_for(
        f"deployment {operation_key}",
        lambda: client.get_deployment(operation_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    print_json(details)
    if details.get("status") != "Finished":
        raise OdcApiError(f"Deployment did not finish successfully: {details.get('status')}")
    return details


def handle_producer_graph(client: OdcClient, args: argparse.Namespace) -> None:
    asset_key = args.asset_key_arg or args.asset_key or client.settings.asset_key
    asset_key = require_key(asset_key, "asset key (positional argument, --asset-key, or ODC_ASSET_KEY)")
    resolved_key = resolve_asset_key(client, asset_key)
    revision = args.revision if args.revision is not None else client.latest_revision(resolved_key)
    producer_type_filter = "All" if args.all_producers else args.producer_type_filter
    asset = client.get_asset(resolved_key)
    root = {
        "key": resolved_key,
        "name": asset.get("name"),
        "revision": revision,
        "type": asset.get("assetType") or asset.get("type"),
    }
    graph = client.producer_graph(
        resolved_key,
        revision,
        environment_key=args.environment_key,
        max_depth=args.max_depth,
        producer_type_filter=producer_type_filter,
    )
    producers = graph.get("results") or []
    output_path = Path(args.output) if args.output else default_mermaid_output_path(resolved_key, revision)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(render_producer_graph_mermaid(root, producers), encoding="utf-8")
    print_json(
        {
            "assetKey": asset_key,
            "producerTypeFilter": producer_type_filter,
            "revision": revision,
            "topLevelProducerCount": len(producers),
            "output": str(output_path),
        }
    )


def run_all_for_asset(
    client: OdcClient,
    asset_key: str,
    environment_key: str,
    revision: int | None,
    build_type: str,
    poll_interval: float,
    timeout_seconds: float,
) -> dict[str, Any]:
    resolved_key = resolve_asset_key(client, asset_key)
    resolved_environment_key = resolve_environment_key(client, environment_key)
    resolved_revision = preflight(client, resolved_key, resolved_environment_key, revision)
    print_dependency_summary(client, resolved_key, resolved_revision, resolved_environment_key)

    label_prefix = f"[{asset_key}] "

    build_response = client.start_build(resolved_key, resolved_revision, build_type)
    print_json({"build_started": build_response})
    build_key = require_key(build_response.get("buildKey"), "buildKey")
    build_details = wait_for(
        f"{label_prefix}build {build_key}",
        lambda: client.get_build(build_key),
        BUILD_TERMINAL_STATUSES,
        interval_seconds=poll_interval,
        timeout_seconds=timeout_seconds,
    )
    if build_details.get("status") != "Finished":
        raise OdcApiError(f"Build did not finish successfully: {build_details.get('status')}")

    deploy_response = client.deploy(resolved_key, resolved_revision, build_key, resolved_environment_key)
    print_json({"deploy_started": deploy_response})
    deploy_key = require_key(deploy_response.get("key"), "deployment operation key")
    deploy_details = wait_for(
        f"{label_prefix}deployment {deploy_key}",
        lambda: client.get_deployment(deploy_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=poll_interval,
        timeout_seconds=timeout_seconds,
    )
    if deploy_details.get("status") != "Finished":
        raise OdcApiError(f"Deployment did not finish successfully: {deploy_details.get('status')}")

    result = {
        "assetKey": resolved_key,
        "environmentKey": resolved_environment_key,
        "revision": resolved_revision,
        "build": build_details,
        "deployment": deploy_details,
    }
    print_json(result)
    return result


def handle_run_all(client: OdcClient, args: argparse.Namespace) -> None:
    asset_key = args.asset_key or client.settings.asset_key
    environment_key = args.environment_key or client.settings.environment_key
    run_all_for_asset(
        client,
        asset_key,
        environment_key,
        args.revision,
        args.build_type,
        args.poll_interval,
        args.timeout,
    )


def read_apps_file(path: str) -> list[str]:
    apps_path = Path(path)
    if not apps_path.is_file():
        raise OdcApiError(f"Apps file not found: {path}")
    lines = apps_path.read_text(encoding="utf-8").splitlines()
    apps = [line.strip() for line in lines]
    apps = [app for app in apps if app and not app.startswith("#")]
    if not apps:
        raise OdcApiError(f"Apps file is empty: {path}")
    return apps


def _deploy_one(
    client: OdcClient,
    asset_key: str,
    environment_key: str,
    revision: int | None,
    build_type: str,
    poll_interval: float,
    timeout_seconds: float,
) -> dict[str, Any]:
    with _PRINT_LOCK:
        print(f"\n=== Deploying '{asset_key}' ===")
    try:
        result = run_all_for_asset(
            client,
            asset_key,
            environment_key,
            revision,
            build_type,
            poll_interval,
            timeout_seconds,
        )
        return {"app": asset_key, "status": "success", "result": result}
    except (OdcApiError, httpx.HTTPError) as exc:
        with _PRINT_LOCK:
            print(f"error: [{asset_key}] {exc}", file=sys.stderr)
        return {"app": asset_key, "status": "failed", "error": str(exc)}


def handle_batch_deploy(client: OdcClient, args: argparse.Namespace) -> None:
    apps = read_apps_file(args.apps_file)
    environment_key = args.environment_key or client.settings.environment_key
    max_parallel = max(1, args.max_parallel)

    # Force token acquisition once up front so concurrent workers don't race on it.
    client.token()

    summary: list[dict[str, Any]] = []
    if max_parallel == 1 and not args.continue_on_error:
        for asset_key in apps:
            entry = _deploy_one(
                client,
                asset_key,
                environment_key,
                args.revision,
                args.build_type,
                args.poll_interval,
                args.timeout,
            )
            summary.append(entry)
            if entry["status"] == "failed":
                break
    else:
        with ThreadPoolExecutor(max_workers=max_parallel) as executor:
            futures = {
                executor.submit(
                    _deploy_one,
                    client,
                    asset_key,
                    environment_key,
                    args.revision,
                    args.build_type,
                    args.poll_interval,
                    args.timeout,
                ): asset_key
                for asset_key in apps
            }
            for future in as_completed(futures):
                summary.append(future.result())

    print("\n=== Batch deploy summary ===")
    print_json(summary)

    if any(entry["status"] == "failed" for entry in summary):
        raise OdcApiError("One or more apps failed to deploy; see summary above.")


def handle_get_user(client: OdcClient, args: argparse.Namespace) -> None:
    user_identifier = require_key(args.user_key, "user key or email")
    user_key = resolve_user_key(client, user_identifier)
    user = client.get_user(user_key)
    print_json(user)


def handle_update_user(client: OdcClient, args: argparse.Namespace) -> None:
    user_identifier = require_key(args.user_key, "user key or email")
    user_key = resolve_user_key(client, user_identifier)
    update_kwargs = {}
    if args.name is not None:
        update_kwargs["name"] = args.name
    if args.is_active is not None:
        update_kwargs["is_active"] = args.is_active
    if args.photo_url is not None:
        update_kwargs["photo_url"] = args.photo_url

    if not update_kwargs:
        raise OdcApiError("At least one field must be specified for update (--name, --is-active, or --photo-url)")

    client.update_user(user_key, **update_kwargs)
    print_json({"status": "success", "message": f"User {user_key} updated successfully"})


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Small ODC API client for build, publish, and deploy tests.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    discover = subparsers.add_parser("discover", help="Fetch OIDC discovery metadata.")
    discover.set_defaults(handler=handle_discover)

    latest_revision = subparsers.add_parser("latest-revision", help="Print the latest asset revision.")
    latest_revision.add_argument("--asset-key", default=None)
    latest_revision.set_defaults(handler=handle_latest_revision)

    validate = subparsers.add_parser("validate", help="Validate the configured asset and environment.")
    add_common_args(validate)
    validate.set_defaults(handler=handle_validate)

    build = subparsers.add_parser("build", help="Start a build operation.")
    add_common_args(build)
    build.add_argument("--build-type", choices=["Debug", "Release"], default="Release")
    build.add_argument("--no-wait", action="store_true")
    build.set_defaults(handler=handle_build)

    publish = subparsers.add_parser("publish", help="Start a publish operation.")
    add_common_args(publish)
    publish.add_argument("--no-wait", action="store_true")
    publish.set_defaults(handler=handle_publish)

    deploy = subparsers.add_parser("deploy", help="Start a deploy operation for an existing build.")
    add_common_args(deploy)
    deploy.add_argument("--build-key", required=True)
    deploy.add_argument("--no-wait", action="store_true")
    deploy.set_defaults(handler=handle_deploy)

    producer_graph = subparsers.add_parser(
        "producer-graph",
        help="Generate a Mermaid graph of all asset producers.",
    )
    producer_graph.add_argument("asset_key_arg", nargs="?", help="Asset key. Defaults to ODC_ASSET_KEY.")
    producer_graph.add_argument("--asset-key", default=None)
    producer_graph.add_argument("--revision", type=int, default=None)
    producer_graph.add_argument("--environment-key", default=None)
    producer_graph.add_argument(
        "--max-depth",
        type=int,
        default=0,
        help="Maximum producer depth. 0 means infinite.",
    )
    producer_graph.add_argument(
        "--producer-type-filter",
        choices=["Deployable", "Libraries", "All"],
        default="Deployable",
        help="Producer type filter to send to the API. Defaults to Deployable.",
    )
    producer_graph.add_argument(
        "--all-producers",
        action="store_true",
        help="Shortcut for --producer-type-filter All.",
    )
    producer_graph.add_argument(
        "--output",
        default=None,
        help="Mermaid output path. Defaults to producer-graph-<asset>-rev-<revision>.mmd.",
    )
    producer_graph.set_defaults(handler=handle_producer_graph)

    run_all = subparsers.add_parser("run-all", help="Build, then deploy the configured asset.")
    add_common_args(run_all)
    run_all.add_argument("--build-type", choices=["Debug", "Release"], default="Release")
    run_all.set_defaults(handler=handle_run_all)

    batch_deploy = subparsers.add_parser(
        "batch-deploy",
        help="Build, then deploy every app listed in a text file (one app name or key per line).",
    )
    batch_deploy.add_argument(
        "apps_file",
        help="Path to a text file with one app name/key per line. Blank lines and lines starting with # are ignored.",
    )
    batch_deploy.add_argument(
        "--environment-key",
        default=None,
        help="Environment name or key. Defaults to ODC_ENVIRONMENT_KEY.",
    )
    batch_deploy.add_argument("--revision", type=int, default=None)
    batch_deploy.add_argument("--poll-interval", type=float, default=10.0)
    batch_deploy.add_argument("--timeout", type=float, default=1800.0)
    batch_deploy.add_argument("--build-type", choices=["Debug", "Release"], default="Release")
    batch_deploy.add_argument(
        "--max-parallel",
        type=int,
        default=5,
        help="Maximum number of apps to build/deploy concurrently. Defaults to 5.",
    )
    batch_deploy.add_argument(
        "--continue-on-error",
        action="store_true",
        help=(
            "Keep deploying remaining apps if one fails instead of stopping. "
            "Only fully honored when --max-parallel is 1; with concurrency, "
            "in-flight apps are not cancelled on a failure either way."
        ),
    )
    batch_deploy.set_defaults(handler=handle_batch_deploy)

    get_user = subparsers.add_parser("get-user", help="Retrieve user information.")
    get_user.add_argument("user_key", help="User key (UUID) or email address.")
    get_user.set_defaults(handler=handle_get_user)

    update_user = subparsers.add_parser("update-user", help="Update user details (name, active status, or photo URL).")
    update_user.add_argument("user_key", help="User key (UUID) or email address.")
    update_user.add_argument("--name", default=None, help="User's name.")
    update_user.add_argument("--is-active", type=lambda x: x.lower() in ("true", "1", "yes"), default=None, help="User active status (true/false).")
    update_user.add_argument("--photo-url", default=None, help="User's photo URL.")
    update_user.set_defaults(handler=handle_update_user)

    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        settings = load_settings()
        with OdcClient(settings) as client:
            args.handler(client, args)
    except (OdcApiError, httpx.HTTPError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0
