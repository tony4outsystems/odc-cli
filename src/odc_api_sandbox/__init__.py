from __future__ import annotations

import argparse
import json
import os
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx


BUILD_TERMINAL_STATUSES = {"Finished", "FinishedWithErrors", "Deleted", "ToBeDeleted"}
OPERATION_TERMINAL_STATUSES = {"Finished", "FinishedWithError"}


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


def load_dotenv(path: Path = Path(".env")) -> None:
    if not path.exists():
        return

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        os.environ.setdefault(key, value)


def load_settings() -> Settings:
    load_dotenv()
    required = [
        "ODC_TENANT_URL",
        "ODC_CLIENT_ID",
        "ODC_CLIENT_SECRET",
        "ODC_ASSET_KEY",
        "ODC_ENVIRONMENT_KEY",
    ]
    missing = [name for name in required if not os.environ.get(name)]
    if missing:
        raise OdcApiError(f"Missing required environment variables: {', '.join(missing)}")

    return Settings(
        tenant_url=os.environ["ODC_TENANT_URL"],
        client_id=os.environ["ODC_CLIENT_ID"],
        client_secret=os.environ["ODC_CLIENT_SECRET"],
        asset_key=os.environ["ODC_ASSET_KEY"],
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

    def url(self, api: str, path: str) -> str:
        return f"{self.settings.tenant_origin}/api/{api}/v1{path}"

    def _request(
        self,
        method: str,
        url: str,
        *,
        auth: bool = True,
        json_data: dict[str, Any] | None = None,
        data: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        headers = {"Accept": "application/json"}
        if auth:
            headers["Authorization"] = f"Bearer {self.token()}"

        response = self.http.request(method, url, headers=headers, json=json_data, data=data)
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
            print(f"{label}: {status}")
            last_status = status
        if status in terminal_statuses:
            return details
        if time.monotonic() >= deadline:
            raise OdcApiError(f"Timed out waiting for {label}; last response: {details}")
        time.sleep(interval_seconds)


def print_json(payload: Any) -> None:
    print(json.dumps(payload, indent=2, sort_keys=True))


def add_common_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--asset-key", default=None)
    parser.add_argument("--environment-key", default=None)
    parser.add_argument("--revision", type=int, default=None)
    parser.add_argument("--poll-interval", type=float, default=10.0)
    parser.add_argument("--timeout", type=float, default=1800.0)


def resolve_revision(client: OdcClient, asset_key: str, revision: int | None) -> int:
    if revision is not None:
        return revision
    resolved = client.latest_revision(asset_key)
    print(f"Using latest revision: {resolved}")
    return resolved


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
    print(client.latest_revision(asset_key))


def handle_build(client: OdcClient, args: argparse.Namespace) -> dict[str, Any]:
    asset_key = args.asset_key or client.settings.asset_key
    revision = resolve_revision(client, asset_key, args.revision)
    response = client.start_build(asset_key, revision, args.build_type)
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
    environment_key = args.environment_key or client.settings.environment_key
    revision = resolve_revision(client, asset_key, args.revision)
    response = client.publish(asset_key, revision, environment_key)
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
    environment_key = args.environment_key or client.settings.environment_key
    revision = resolve_revision(client, asset_key, args.revision)
    build_key = require_key(args.build_key, "--build-key")
    response = client.deploy(asset_key, revision, build_key, environment_key)
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


def handle_run_all(client: OdcClient, args: argparse.Namespace) -> None:
    asset_key = args.asset_key or client.settings.asset_key
    environment_key = args.environment_key or client.settings.environment_key
    revision = resolve_revision(client, asset_key, args.revision)

    build_response = client.start_build(asset_key, revision, args.build_type)
    print_json({"build_started": build_response})
    build_key = require_key(build_response.get("buildKey"), "buildKey")
    build_details = wait_for(
        f"build {build_key}",
        lambda: client.get_build(build_key),
        BUILD_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    if build_details.get("status") != "Finished":
        raise OdcApiError(f"Build did not finish successfully: {build_details.get('status')}")

    publish_response = client.publish(asset_key, revision, environment_key)
    print_json({"publish_started": publish_response})
    publish_key = require_key(publish_response.get("key"), "publish operation key")
    publish_details = wait_for(
        f"publish {publish_key}",
        lambda: client.get_publish(publish_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    if publish_details.get("status") != "Finished":
        raise OdcApiError(f"Publish did not finish successfully: {publish_details.get('status')}")
    build_key = publish_details.get("buildKey") or build_key

    deploy_response = client.deploy(asset_key, revision, build_key, environment_key)
    print_json({"deploy_started": deploy_response})
    deploy_key = require_key(deploy_response.get("key"), "deployment operation key")
    deploy_details = wait_for(
        f"deployment {deploy_key}",
        lambda: client.get_deployment(deploy_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    if deploy_details.get("status") != "Finished":
        raise OdcApiError(f"Deployment did not finish successfully: {deploy_details.get('status')}")

    print_json(
        {
            "assetKey": asset_key,
            "environmentKey": environment_key,
            "revision": revision,
            "build": build_details,
            "publish": publish_details,
            "deployment": deploy_details,
        }
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Small ODC API client for build, publish, and deploy tests.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    discover = subparsers.add_parser("discover", help="Fetch OIDC discovery metadata.")
    discover.set_defaults(handler=handle_discover)

    latest_revision = subparsers.add_parser("latest-revision", help="Print the latest asset revision.")
    latest_revision.add_argument("--asset-key", default=None)
    latest_revision.set_defaults(handler=handle_latest_revision)

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

    run_all = subparsers.add_parser("run-all", help="Build, publish, then deploy the configured asset.")
    add_common_args(run_all)
    run_all.add_argument("--build-type", choices=["Debug", "Release"], default="Release")
    run_all.set_defaults(handler=handle_run_all)

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
