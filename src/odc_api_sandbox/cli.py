from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

import httpx

from .client import BUILD_TERMINAL_STATUSES, OPERATION_TERMINAL_STATUSES, OdcClient
from .errors import OdcApiError
from .mermaid import default_mermaid_output_path, render_producer_graph_mermaid
from .resolve import resolve_asset_key, resolve_environment_key, resolve_user_key
from .settings import load_settings
from .utils import print_json, require_key
from .workflows import batch_deploy, preflight, run_all_for_asset, undeploy_all_in_environment, wait_for


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


def handle_undeploy(client: OdcClient, args: argparse.Namespace) -> dict[str, Any]:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    environment_key = resolve_environment_key(client, args.environment_key or client.settings.environment_key)
    response = client.undeploy(resolved_key, environment_key)
    print_json(response)
    operation_key = require_key(response.get("key"), "deployment operation key")
    if args.no_wait:
        return response

    details = wait_for(
        f"undeploy {operation_key}",
        lambda: client.get_deployment(operation_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=args.poll_interval,
        timeout_seconds=args.timeout,
    )
    print_json(details)
    if details.get("status") != "Finished":
        raise OdcApiError(f"Undeploy did not finish successfully: {details.get('status')}")
    return details


def handle_list_environments(client: OdcClient, _args: argparse.Namespace) -> None:
    environments = client.list_environments()
    print_json(
        [
            {
                "name": environment.get("name"),
                "key": environment.get("key"),
                "type": environment.get("type") or environment.get("stage"),
            }
            for environment in environments
        ]
    )


def handle_delete_app(client: OdcClient, args: argparse.Namespace) -> None:
    asset_key = args.asset_key or client.settings.asset_key
    resolved_key = resolve_asset_key(client, asset_key)
    client.delete_asset(resolved_key)
    print_json({"status": "success", "message": f"Asset {resolved_key} deleted successfully"})


def handle_undeploy_all(client: OdcClient, args: argparse.Namespace) -> None:
    environment_key = args.environment_key or client.settings.environment_key
    summary = undeploy_all_in_environment(
        client,
        environment_key,
        args.poll_interval,
        args.timeout,
        args.max_parallel,
    )

    print("\n=== Undeploy-all summary ===")
    print_json(summary)

    if any(entry["status"] == "failed" for entry in summary):
        raise OdcApiError("One or more apps failed to undeploy; see summary above.")


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


def handle_batch_deploy(client: OdcClient, args: argparse.Namespace) -> None:
    environment_key = args.environment_key or client.settings.environment_key
    summary = batch_deploy(
        client,
        args.apps_file,
        environment_key,
        args.revision,
        args.build_type,
        args.poll_interval,
        args.timeout,
        args.max_parallel,
        args.continue_on_error,
        args.skip_dependencies,
    )

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

    undeploy = subparsers.add_parser("undeploy", help="Undeploy a single app from an environment.")
    add_common_args(undeploy)
    undeploy.add_argument("--no-wait", action="store_true")
    undeploy.set_defaults(handler=handle_undeploy)

    list_environments = subparsers.add_parser("list-environments", help="List environments visible to the API client.")
    list_environments.set_defaults(handler=handle_list_environments)

    delete_app = subparsers.add_parser("delete-app", help="Delete an asset from the asset repository.")
    delete_app.add_argument("--asset-key", default=None, help="Asset name or key. Defaults to ODC_ASSET_KEY.")
    delete_app.set_defaults(handler=handle_delete_app)

    undeploy_all = subparsers.add_parser(
        "undeploy-all",
        help="Undeploy every app currently deployed to an environment.",
    )
    undeploy_all.add_argument(
        "--environment-key",
        default=None,
        help="Environment name or key. Defaults to ODC_ENVIRONMENT_KEY.",
    )
    undeploy_all.add_argument("--poll-interval", type=float, default=10.0)
    undeploy_all.add_argument("--timeout", type=float, default=1800.0)
    undeploy_all.add_argument(
        "--max-parallel",
        type=int,
        default=3,
        help="Maximum number of apps to undeploy concurrently. Defaults to 3.",
    )
    undeploy_all.set_defaults(handler=handle_undeploy_all)

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

    batch_deploy_parser = subparsers.add_parser(
        "batch-deploy",
        help="Build, then deploy every app listed in a text file (one app name or key per line).",
    )
    batch_deploy_parser.add_argument(
        "apps_file",
        help="Path to a text file with one app name/key per line. Blank lines and lines starting with # are ignored.",
    )
    batch_deploy_parser.add_argument(
        "--environment-key",
        default=None,
        help="Environment name or key. Defaults to ODC_ENVIRONMENT_KEY.",
    )
    batch_deploy_parser.add_argument("--revision", type=int, default=None)
    batch_deploy_parser.add_argument("--poll-interval", type=float, default=10.0)
    batch_deploy_parser.add_argument("--timeout", type=float, default=1800.0)
    batch_deploy_parser.add_argument("--build-type", choices=["Debug", "Release"], default="Release")
    batch_deploy_parser.add_argument(
        "--max-parallel",
        type=int,
        default=3,
        help="Maximum number of apps to build/deploy concurrently. Defaults to 3.",
    )
    batch_deploy_parser.add_argument(
        "--continue-on-error",
        action="store_true",
        help=(
            "Keep deploying remaining apps if one fails instead of stopping. "
            "Only fully honored when --max-parallel is 1; with concurrency, "
            "in-flight apps are not cancelled on a failure either way."
        ),
    )
    batch_deploy_parser.add_argument(
        "--skip-dependencies",
        action="store_true",
        help=(
            "Deploy only the apps listed in the file, without automatically including "
            "their producer dependencies. Off by default: dependencies are resolved via "
            "the producer graph, deduplicated across apps, and deployed before the apps "
            "that need them."
        ),
    )
    batch_deploy_parser.set_defaults(handler=handle_batch_deploy)

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
