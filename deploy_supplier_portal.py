#!/usr/bin/env python3
"""
Deploy SupplierPortal app to production environment with all dependencies.

Usage:
    python deploy_supplier_portal.py production [--revision REV]
    python deploy_supplier_portal.py --target-env production [--revision REV]

The environment argument can be:
    - An environment name (e.g., "production", "staging")
    - An environment key GUID (e.g., "a3d20420-bfc2-4e25-9f7c-9bc2eead3c96")

Environment variables required:
    ODC_TENANT_URL - OutSystems ODC tenant URL
    ODC_CLIENT_ID - OAuth client ID
    ODC_CLIENT_SECRET - OAuth client secret
"""

import argparse
import sys
from pathlib import Path

# Add the package to path
sys.path.insert(0, str(Path(__file__).parent / "src"))

from odc_api_sandbox import (
    OdcClient,
    OdcApiError,
    load_settings,
    resolve_asset_key,
    preflight,
    wait_for,
    BUILD_TERMINAL_STATUSES,
    OPERATION_TERMINAL_STATUSES,
    require_key,
    print_json,
    GUID_PATTERN,
)


def resolve_environment_key(client: OdcClient, input_env: str) -> str:
    """
    Resolve environment name or key to environment key.
    If input_env is already a GUID, return it. Otherwise, search by name.
    """
    if GUID_PATTERN.match(input_env):
        return input_env

    environments = client._request("GET", client.url("portfolios", "/environments"))
    results = environments.get("results") or []

    matches = [
        e for e in results
        if input_env.lower() in e.get("name", "").lower()
        or input_env.lower() in e.get("key", "").lower()
    ]

    if not matches:
        print(f"error: No environments found matching '{input_env}'", file=sys.stderr)
        print("Available environments:", file=sys.stderr)
        for env in results:
            print(f"  - {env.get('name')} ({env.get('key')})", file=sys.stderr)
        sys.exit(1)

    exact_matches = [
        e for e in matches if e.get("name", "").lower() == input_env.lower()
    ]

    if len(exact_matches) == 1:
        env_key = exact_matches[0].get("key")
        return require_key(env_key, "environment key")
    elif len(exact_matches) > 1:
        print("error: Multiple environments match the name (ambiguous):", file=sys.stderr)
        for env in exact_matches:
            print(f"  - {env.get('name')} ({env.get('key')})", file=sys.stderr)
        sys.exit(1)
    else:
        if len(matches) == 1:
            env_key = matches[0].get("key")
            return require_key(env_key, "environment key")
        print(
            f"error: No exact match for '{input_env}'. Did you mean:",
            file=sys.stderr,
        )
        for env in matches[:10]:
            print(f"  - {env.get('name')} ({env.get('key')})", file=sys.stderr)
        if len(matches) > 10:
            print(f"  ... and {len(matches) - 10} more", file=sys.stderr)
        sys.exit(1)


def deploy_with_dependencies(
    client: OdcClient,
    asset_name: str,
    target_environment: str,
    revision: int | None = None,
    poll_interval: float = 10.0,
    timeout_seconds: float = 1800.0,
) -> dict:
    """
    Deploy an app to target environment with all its dependencies.

    This performs:
    1. Asset resolution (name to key)
    2. Environment resolution (name to key)
    3. Preflight validation
    4. Producer graph analysis (shows dependencies)
    5. Build operation (Release type)
    6. Deployment to target environment
    """
    print(f"\n🚀 Starting deployment of '{asset_name}' to '{target_environment}'...")

    # Step 1: Resolve environment key
    print(f"\n📍 Resolving environment key for '{target_environment}'...")
    resolved_env_key = resolve_environment_key(client, target_environment)
    env_details = client.get_environment(resolved_env_key)
    env_name = env_details.get("name", target_environment)
    print(f"   ✓ Environment: {env_name} ({resolved_env_key})")

    # Step 2: Resolve asset key
    print(f"\n📍 Resolving asset key for '{asset_name}'...")
    resolved_key = resolve_asset_key(client, asset_name)
    print(f"   ✓ Asset key: {resolved_key}")

    # Step 3: Preflight validation
    print(f"\n✅ Running preflight checks...")
    resolved_revision = preflight(client, resolved_key, resolved_env_key, revision)
    print(f"   ✓ Using revision: {resolved_revision}")

    # Step 4: Show producer graph (dependencies)
    print(f"\n📦 Analyzing dependencies...")
    try:
        graph = client.producer_graph(resolved_key, resolved_revision, environment_key=resolved_env_key)
        producers = graph.get("results") or []
        print(f"   ✓ Found {len(producers)} dependencies")
        for producer in producers:
            producer_name = producer.get("name", "Unknown")
            producer_type = producer.get("type", "Unknown")
            producer_status = producer.get("status", "Unknown")
            print(f"     - {producer_name} ({producer_type}) - {producer_status}")
    except Exception as e:
        print(f"   ⚠ Could not retrieve dependencies: {e}")

    # Step 5: Build
    print(f"\n🔨 Starting build operation (Release mode)...")
    build_response = client.start_build(resolved_key, resolved_revision, "Release")
    build_key = require_key(build_response.get("buildKey"), "buildKey")
    print(f"   ✓ Build started: {build_key}")

    build_details = wait_for(
        "   Build progress",
        lambda: client.get_build(build_key),
        BUILD_TERMINAL_STATUSES,
        interval_seconds=poll_interval,
        timeout_seconds=timeout_seconds,
    )

    build_status = build_details.get("status")
    if build_status != "Finished":
        raise OdcApiError(f"Build failed with status: {build_status}")
    print(f"   ✓ Build completed successfully")

    # Step 6: Deploy
    print(f"\n🚢 Starting deployment to '{env_name}'...")
    deploy_response = client.deploy(resolved_key, resolved_revision, build_key, resolved_env_key)
    deploy_key = require_key(deploy_response.get("key"), "deployment operation key")
    print(f"   ✓ Deployment started: {deploy_key}")

    deploy_details = wait_for(
        "   Deployment progress",
        lambda: client.get_deployment(deploy_key),
        OPERATION_TERMINAL_STATUSES,
        interval_seconds=poll_interval,
        timeout_seconds=timeout_seconds,
    )

    deploy_status = deploy_details.get("status")
    if deploy_status != "Finished":
        raise OdcApiError(f"Deployment failed with status: {deploy_status}")
    print(f"   ✓ Deployment completed successfully")

    # Summary
    print(f"\n✨ Deployment Summary:")
    print_json({
        "app": asset_name,
        "assetKey": resolved_key,
        "revision": resolved_revision,
        "targetEnvironment": env_name,
        "targetEnvironmentKey": resolved_env_key,
        "buildKey": build_key,
        "buildStatus": build_status,
        "deploymentKey": deploy_key,
        "deploymentStatus": deploy_status,
    })

    return {
        "assetKey": resolved_key,
        "environmentKey": resolved_env_key,
        "revision": resolved_revision,
        "buildKey": build_key,
        "deploymentKey": deploy_key,
        "success": True,
    }


def main():
    parser = argparse.ArgumentParser(
        description="Deploy SupplierPortal to target environment with all dependencies."
    )
    parser.add_argument(
        "target_env_arg",
        nargs="?",
        default=None,
        help="Target environment name or key (positional argument)",
    )
    parser.add_argument(
        "--asset-name",
        default="SupplierPortal",
        help="Asset name or key to deploy (default: SupplierPortal)",
    )
    parser.add_argument(
        "--target-env",
        default=None,
        help="Target environment key or name (overrides positional argument)",
    )
    parser.add_argument(
        "--revision",
        type=int,
        default=None,
        help="Specific revision to deploy (default: latest)",
    )
    parser.add_argument(
        "--poll-interval",
        type=float,
        default=10.0,
        help="Polling interval in seconds (default: 10.0)",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=1800.0,
        help="Operation timeout in seconds (default: 1800.0)",
    )

    args = parser.parse_args()

    # Resolve target environment from positional or --target-env
    target_env = args.target_env or args.target_env_arg
    if not target_env:
        parser.error("target environment is required (positional argument or --target-env)")

    try:
        settings = load_settings()

        with OdcClient(settings) as client:
            result = deploy_with_dependencies(
                client,
                args.asset_name,
                target_env,
                revision=args.revision,
                poll_interval=args.poll_interval,
                timeout_seconds=args.timeout,
            )
            print(f"\n✅ Deployment completed successfully!\n")
            return 0
    except (OdcApiError, Exception) as exc:
        print(f"\n❌ Deployment failed: {exc}\n", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
