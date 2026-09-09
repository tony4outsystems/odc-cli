# ODC API Sandbox

Small Python client for testing the ODC APIs described in `api-specs/`.

## Setup

Auth is read from environment variables, loaded from a `.env` file in the project root (never pass credentials as CLI arguments — they'd leak into shell history and process listings). Required:

- `ODC_TENANT_URL`
- `ODC_CLIENT_ID`
- `ODC_CLIENT_SECRET`

Optional:

- `ODC_SCOPE` — OAuth scope override

Asset and environment are not read from `.env` — pass `--asset-key`/`--environment-key` explicitly to each command that needs them.

## Quick start

```bash
uv run odc-api-sandbox discover
uv run odc-api-sandbox validate --asset-key <asset-key> --environment-key <environment-key>
uv run odc-api-sandbox latest-revision --asset-key <asset-key>
uv run odc-api-sandbox deploy --asset-key <asset-key> --environment-key <environment-key>
```

The `validate` command confirms that the given asset and environment keys are visible to the API client, then prints a short summary of both objects. The `deploy` command runs the same validation, resolves the latest revision, starts a Release build, waits for it to finish, then deploys it to the given environment.

## Commands

### discover

Fetch OIDC discovery metadata (issuer, token endpoint, supported scopes).

```bash
uv run odc-api-sandbox discover
```

### validate

Confirm the given asset and environment are visible to the API client.

```bash
uv run odc-api-sandbox validate --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```

### latest-revision

Print the latest revision number for an asset.

```bash
uv run odc-api-sandbox latest-revision --asset-key <asset-key>
```

### internal-build / internal-publish / internal-deploy / internal-undeploy

Raw single-step operations against the build/publish/deploy/undeploy APIs. These are the building blocks `deploy`, `batch-deploy`, and `undeploy-all` are made of — reach for them only when you need to drive one step in isolation (e.g. deploy a build that already exists). Each requires `--asset-key`/`--environment-key` and polls until the operation finishes (`--no-wait` to skip polling):

```
uv run odc-api-sandbox internal-build --asset-key <asset-key> --environment-key <environment-key> --revision 1 --build-type Release
uv run odc-api-sandbox internal-publish --asset-key <asset-key> --environment-key <environment-key> --revision 1
uv run odc-api-sandbox internal-deploy --asset-key <asset-key> --environment-key <environment-key> --revision 1 --build-key <build-key>
uv run odc-api-sandbox internal-undeploy --asset-key <asset-key> --environment-key <environment-key>
```

Common options for `internal-build`, `internal-publish`, `internal-deploy`, `internal-undeploy`, `validate`, and `deploy`:

- `--asset-key` — asset name or key (required)
- `--environment-key` — environment name or key (required)
- `--revision` — asset revision (defaults to the latest revision)
- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — seconds to wait before giving up (default `1800`)
- `--no-wait` — return after starting the operation instead of polling (internal-build/internal-publish/internal-deploy/internal-undeploy only)
- `--build-type` — `Debug` or `Release` (default `Release`; internal-build/deploy only)
- `internal-deploy` also requires `--build-key <build-key>`

### deploy

Validate, resolve the latest revision, build (Release by default), wait for the build to finish, then deploy — all for one asset/environment.

```bash
uv run odc-api-sandbox deploy --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```

### list-environments

List environments visible to the API client (name, key, type).

```bash
uv run odc-api-sandbox list-environments
```

### delete-app

Permanently delete an asset from the asset repository.

```bash
uv run odc-api-sandbox delete-app --asset-key <asset-key>
```

### producer-graph

Generate a Mermaid graph of an asset's producer dependencies.

```bash
uv run odc-api-sandbox producer-graph <asset-key> --max-depth 2 --output graph.mmd
```

- `asset_key` — positional (also settable via `--asset-key`)
- `--revision` — defaults to the latest revision
- `--environment-key` — environment context for resolving producers
- `--max-depth` — maximum producer depth to traverse; `0` (default) means infinite
- `--producer-type-filter` — `Deployable` (default), `Libraries`, or `All`
- `--all-producers` — shortcut for `--producer-type-filter All`
- `--output` — output path; defaults to `producer-graph-<asset>-rev-<revision>.mmd`

### batch-deploy

Build and deploy every app listed in a text file, one app per line: `asset_key` to deploy its latest revision, or `asset_key@revision` to pin a specific one (blank lines and `#` comments ignored). See [apps.txt](apps.txt) for an example. Per-app pinning avoids the ambiguity of a single `--revision` flag when the file lists apps that need different revisions.

```bash
uv run odc-api-sandbox batch-deploy apps.txt --environment-key <environment-key>
```

By default, each app's producer dependencies are resolved via the producer graph, deduplicated across all listed apps, and deployed before the apps that need them. Dependencies always deploy at the revision resolved from the producer graph, regardless of any pinned revision on the apps that depend on them.

Options:

- `--environment-key` — environment name or key (required)
- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — seconds to wait per app before giving up (default `1800`)
- `--build-type` — `Debug` or `Release` (default `Release`)
- `--max-parallel` — maximum apps to build/deploy concurrently (default `5`)
- `--continue-on-error` — keep deploying remaining apps if one fails, instead of stopping. Only fully honored when `--max-parallel 1`; with concurrency, in-flight apps are not cancelled on a failure either way
- `--skip-dependencies` — deploy only the apps listed in the file, without automatically including their producer dependencies

Example with overrides:

```bash
uv run odc-api-sandbox batch-deploy apps.txt --environment-key <env> --build-type Release --max-parallel 3 --continue-on-error
```

### undeploy-all

Undeploy every app currently deployed to an environment.

```bash
uv run odc-api-sandbox undeploy-all --environment-key <environment-key>
```

Options:

- `--environment-key` — environment name or key (required)
- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — seconds to wait per app before giving up (default `1800`)
- `--max-parallel` — maximum apps to undeploy concurrently (default `3`)

### get-user

Retrieve user information by user key (UUID) or email address.

```bash
uv run odc-api-sandbox get-user <user-key-or-email>
```

### update-user

Update a user's name, active status, or photo URL. At least one of `--name`, `--is-active`, or `--photo-url` is required.

```bash
uv run odc-api-sandbox update-user <user-key-or-email> --name "Jane Doe" --is-active true --photo-url https://example.com/photo.jpg
```
