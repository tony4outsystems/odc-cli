from __future__ import annotations

import json
import time
from typing import Any

import httpx

from .errors import OdcApiError
from .settings import Settings

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


class OdcClient:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.http = httpx.Client(timeout=httpx.Timeout(60.0))
        self._token: str | None = None
        self._discovery: dict[str, Any] | None = None
        self._assets_cache: list[dict[str, Any]] | None = None

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

    def list_deployed_assets(self, environment_key: str) -> list[dict[str, Any]]:
        assets: list[dict[str, Any]] = []
        offset = 0
        while True:
            response = self._request(
                "GET",
                self.url("portfolios", "/deployed-assets"),
                params={"environmentKey": environment_key, "limit": 100, "offset": offset},
            )
            assets.extend(response.get("results") or [])
            page = response.get("page") or {}
            next_offset = page.get("nextPageOffset")
            total_results = page.get("totalResults")
            if next_offset is None or next_offset <= offset or len(assets) >= (total_results or len(assets)):
                break
            offset = next_offset
        return assets

    def list_assets(self) -> list[dict[str, Any]]:
        if self._assets_cache is not None:
            return self._assets_cache
        assets: list[dict[str, Any]] = []
        offset = 0
        while True:
            response = self._request(
                "GET",
                self.url("asset-repository", "/assets"),
                params={"limit": 100, "offset": offset},
            )
            assets.extend(response.get("results") or [])
            page = response.get("page") or {}
            next_offset = page.get("nextPageOffset")
            total_results = page.get("totalResults")
            if next_offset is None or next_offset <= offset or len(assets) >= (total_results or len(assets)):
                break
            offset = next_offset
        self._assets_cache = assets
        return assets

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

    def undeploy(self, asset_key: str, environment_key: str) -> dict[str, Any]:
        return self._request(
            "POST",
            self.url("deployments", "/deployment-operations"),
            json_data={
                "operation": "Undeploy",
                "assetKey": asset_key,
                "environmentKey": environment_key,
            },
        )

    def get_deployment(self, operation_key: str) -> dict[str, Any]:
        return self._request("GET", self.url("deployments", f"/deployment-operations/{operation_key}"))

    def undeploy(self, asset_key: str, environment_key: str) -> dict[str, Any]:
        return self._request(
            "POST",
            self.url("deployments", "/deployment-operations"),
            json_data={
                "operation": "Undeploy",
                "assetKey": asset_key,
                "environmentKey": environment_key,
            },
        )

    def delete_asset(self, asset_key: str) -> None:
        self._request("DELETE", self.url("asset-repository", f"/assets/{asset_key}"))

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

        max_attempts = 6
        for attempt in range(1, max_attempts + 1):
            response = self.http.request(
                method,
                url,
                headers=headers,
                json=json_data,
                data=data,
                params=params,
            )
            if response.status_code == 429 and attempt < max_attempts:
                time.sleep(_retry_delay_seconds(response, attempt))
                continue
            if response.status_code >= 400:
                raise OdcApiError(format_error(response))
            if not response.content:
                return {}
            return response.json()


def _retry_delay_seconds(response: httpx.Response, attempt: int) -> float:
    retry_after = response.headers.get("Retry-After")
    if retry_after:
        try:
            return max(0.0, float(retry_after))
        except ValueError:
            pass
    return min(30.0, 2.0**attempt)


def format_error(response: httpx.Response) -> str:
    try:
        payload = response.json()
    except json.JSONDecodeError:
        payload = response.text
    return f"{response.request.method} {response.url} failed with {response.status_code}: {payload}"
