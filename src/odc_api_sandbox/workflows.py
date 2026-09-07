from __future__ import annotations

import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any

import httpx

from .client import BUILD_TERMINAL_STATUSES, OPERATION_TERMINAL_STATUSES, OdcClient
from .errors import OdcApiError
from .resolve import resolve_asset_key, resolve_environment_key
from .utils import _PRINT_LOCK, compact_dict, print_json, require_key


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


def preflight(client: OdcClient, asset_key: str, environment_key: str, revision: int | None) -> int:
    asset = client.get_asset(asset_key)
    environment = client.get_environment(environment_key)
    resolved_revision = revision if revision is not None else asset.get("revision")
    if not isinstance(resolved_revision, int):
        resolved_revision = client.latest_revision(asset_key)
    print_preflight_summary(asset, environment, resolved_revision)
    return resolved_revision


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


def _undeploy_one(
    client: OdcClient,
    asset_key: str,
    asset_name: str,
    environment_key: str,
    poll_interval: float,
    timeout_seconds: float,
) -> dict[str, Any]:
    with _PRINT_LOCK:
        print(f"\n=== Undeploying '{asset_name}' ===")
    try:
        response = client.undeploy(asset_key, environment_key)
        operation_key = require_key(response.get("key"), "deployment operation key")
        details = wait_for(
            f"[{asset_name}] undeploy {operation_key}",
            lambda: client.get_deployment(operation_key),
            OPERATION_TERMINAL_STATUSES,
            interval_seconds=poll_interval,
            timeout_seconds=timeout_seconds,
        )
        if details.get("status") != "Finished":
            raise OdcApiError(f"Undeploy did not finish successfully: {details.get('status')}")
        return {"app": asset_name, "assetKey": asset_key, "status": "success", "result": details}
    except (OdcApiError, httpx.HTTPError) as exc:
        with _PRINT_LOCK:
            print(f"error: [{asset_name}] {exc}", file=sys.stderr)
        return {"app": asset_name, "assetKey": asset_key, "status": "failed", "error": str(exc)}


def undeploy_all_in_environment(
    client: OdcClient,
    environment_key: str,
    poll_interval: float,
    timeout_seconds: float,
    max_parallel: int,
) -> list[dict[str, Any]]:
    resolved_environment_key = resolve_environment_key(client, environment_key)
    deployed_assets = client.list_deployed_assets(resolved_environment_key)
    if not deployed_assets:
        print(f"No deployed apps found in environment {resolved_environment_key}.")
        return []

    max_parallel = max(1, max_parallel)
    client.token()

    apps = []
    for asset in deployed_assets:
        asset_key = asset.get("key")
        if not asset_key:
            continue
        deployments = asset.get("deployments") or []
        name = next((d.get("name") for d in deployments if d.get("name")), None) or asset_key
        apps.append((asset_key, name))

    summary: list[dict[str, Any]] = []
    with ThreadPoolExecutor(max_workers=max_parallel) as executor:
        futures = {
            executor.submit(
                _undeploy_one,
                client,
                asset_key,
                asset_name,
                resolved_environment_key,
                poll_interval,
                timeout_seconds,
            ): asset_key
            for asset_key, asset_name in apps
        }
        for future in as_completed(futures):
            summary.append(future.result())

    return summary


def build_dependency_plan(
    client: OdcClient, asset_keys: list[str], environment_key: str
) -> list[list[tuple[str, int | None]]]:
    """Expand asset_keys with their producer dependencies into ordered deploy levels.

    Each level is a list of (asset_key, revision) pairs that can be deployed in
    parallel; every level must finish before the next one starts. A dependency
    shared by multiple apps is only included once, at the earliest level all of
    its consumers need it deployed by.
    """
    revisions: dict[str, int | None] = {}
    producers_of: dict[str, set[str]] = {}

    def record(node_key: str, revision: int | None) -> None:
        if node_key not in producers_of:
            producers_of[node_key] = set()
            revisions[node_key] = revision
        elif revision is not None and revisions.get(node_key) is None:
            revisions[node_key] = revision

    def visit_producer_node(node: dict[str, Any]) -> str:
        node_key = require_key(node.get("key"), "producer key")
        record(node_key, node.get("revision"))
        for child in node.get("producers") or []:
            child_key = visit_producer_node(child)
            producers_of[node_key].add(child_key)
        return node_key

    for asset_key in asset_keys:
        resolved_key = resolve_asset_key(client, asset_key)
        record(resolved_key, None)
        revision = revisions[resolved_key] or client.latest_revision(resolved_key)
        graph = client.producer_graph(
            resolved_key, revision, environment_key=environment_key, producer_type_filter="All"
        )
        for producer in graph.get("results") or []:
            child_key = visit_producer_node(producer)
            producers_of[resolved_key].add(child_key)

    levels: dict[str, int] = {}

    def level_of(node_key: str) -> int:
        if node_key in levels:
            return levels[node_key]
        levels[node_key] = 0  # guard against cycles
        deps = producers_of.get(node_key) or set()
        level = 1 + max((level_of(dep) for dep in deps), default=-1)
        levels[node_key] = level
        return level

    for node_key in producers_of:
        level_of(node_key)

    max_level = max(levels.values(), default=-1)
    plan: list[list[tuple[str, int | None]]] = [[] for _ in range(max_level + 1)]
    for node_key, level in levels.items():
        plan[level].append((node_key, revisions.get(node_key)))
    return plan


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


def batch_deploy(
    client: OdcClient,
    apps_file: str,
    environment_key: str,
    revision: int | None,
    build_type: str,
    poll_interval: float,
    timeout_seconds: float,
    max_parallel: int,
    continue_on_error: bool,
    skip_dependencies: bool,
) -> list[dict[str, Any]]:
    apps = read_apps_file(apps_file)
    resolved_environment_key = resolve_environment_key(client, environment_key)
    max_parallel = max(1, max_parallel)

    # Force token acquisition once up front so concurrent workers don't race on it.
    client.token()

    if skip_dependencies:
        plan = [[(asset_key, revision) for asset_key in apps]]
    else:
        explicit_keys = {resolve_asset_key(client, asset_key) for asset_key in apps}
        plan = [
            # Only apply the requested revision to apps explicitly listed in the file;
            # dependencies deploy at the revision resolved from the producer graph.
            [(key, revision if key in explicit_keys else dep_revision) for key, dep_revision in level]
            for level in build_dependency_plan(client, apps, resolved_environment_key)
        ]
        added = sum(1 for level in plan for key, _ in level if key not in explicit_keys)
        if added:
            print(f"Including {added} dependency app(s) not listed in {apps_file}.")

    summary: list[dict[str, Any]] = []
    stop = False
    for level in plan:
        if stop:
            break
        if max_parallel == 1 and not continue_on_error:
            for asset_key, level_revision in level:
                entry = _deploy_one(
                    client,
                    asset_key,
                    resolved_environment_key,
                    level_revision,
                    build_type,
                    poll_interval,
                    timeout_seconds,
                )
                summary.append(entry)
                if entry["status"] == "failed":
                    stop = True
                    break
        else:
            with ThreadPoolExecutor(max_workers=max_parallel) as executor:
                futures = {
                    executor.submit(
                        _deploy_one,
                        client,
                        asset_key,
                        resolved_environment_key,
                        level_revision,
                        build_type,
                        poll_interval,
                        timeout_seconds,
                    ): asset_key
                    for asset_key, level_revision in level
                }
                for future in as_completed(futures):
                    summary.append(future.result())
            if not continue_on_error and any(entry["status"] == "failed" for entry in summary):
                stop = True

    return summary
