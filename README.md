# ODC API Sandbox

Small Python client for testing the ODC APIs described in `api-specs/`.

## Setup

Configuration is read from environment variables, loaded from a `.env` file in the project root. Required:

- `ODC_TENANT_URL`
- `ODC_CLIENT_ID`
- `ODC_CLIENT_SECRET`
- `ODC_ENVIRONMENT_KEY`

Optional:

- `ODC_ASSET_KEY` — default asset for commands that accept `--asset-key`
- `ODC_SCOPE` — OAuth scope override

The default `.env` values target the provided tenant, asset key, and environment key.

## Quick start

```bash
uv run odc-api-sandbox discover
uv run odc-api-sandbox validate
uv run odc-api-sandbox latest-revision
uv run odc-api-sandbox run-all
```

The `validate` command confirms that the configured asset and environment keys are visible to the API client, then prints a short summary of both objects. The `run-all` command runs the same validation, resolves the latest revision, starts a Release build, waits for it to finish, then deploys it to the configured environment.

## Commands

### discover

Fetch OIDC discovery metadata (issuer, token endpoint, supported scopes).

```bash
uv run odc-api-sandbox discover
```

### validate

Confirm the configured asset and environment are visible to the API client.

```bash
uv run odc-api-sandbox validate --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```

### latest-revision

Print the latest revision number for an asset.

```bash
uv run odc-api-sandbox latest-revision --asset-key <asset-key>
```

### build / publish / deploy

Individual operations, each defaulting to the configured asset/environment and polling until the operation finishes (`--no-wait` to skip polling):

```bash
uv run odc-api-sandbox build --revision 1 --build-type Release
uv run odc-api-sandbox publish --revision 1
uv run odc-api-sandbox deploy --revision 1 --build-key <build-key>
```

Common options for `build`, `publish`, `deploy`, `validate`, and `run-all`:

- `--asset-key` — asset name or key (defaults to `ODC_ASSET_KEY`)
- `--environment-key` — environment name or key (defaults to `ODC_ENVIRONMENT_KEY`)
- `--revision` — asset revision (defaults to the latest revision)
- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — seconds to wait before giving up (default `1800`)
- `--no-wait` — return after starting the operation instead of polling (build/publish/deploy only)
- `--build-type` — `Debug` or `Release` (default `Release`; build/run-all only)
- `deploy` also requires `--build-key <build-key>`

### run-all

Validate, resolve the latest revision, build (Release by default), wait for the build to finish, then deploy — all for one asset/environment.

```bash
uv run odc-api-sandbox run-all --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```

### producer-graph

Generate a Mermaid graph of an asset's producer dependencies.

```bash
uv run odc-api-sandbox producer-graph <asset-key> --max-depth 2 --output graph.mmd
```

- `asset_key` — positional; defaults to `ODC_ASSET_KEY` if omitted (also settable via `--asset-key`)
- `--revision` — defaults to the latest revision
- `--environment-key` — environment context for resolving producers
- `--max-depth` — maximum producer depth to traverse; `0` (default) means infinite
- `--producer-type-filter` — `Deployable` (default), `Libraries`, or `All`
- `--all-producers` — shortcut for `--producer-type-filter All`
- `--output` — output path; defaults to `producer-graph-<asset>-rev-<revision>.mmd`

### batch-deploy

Build and deploy every app listed in a text file, one app name or key per line (blank lines and `#` comments ignored). See [apps.txt](apps.txt) for an example.

```bash
uv run odc-api-sandbox batch-deploy apps.txt
```

By default, each app's producer dependencies are resolved via the producer graph, deduplicated across all listed apps, and deployed before the apps that need them.

Options:

- `--environment-key` — environment name or key (defaults to `ODC_ENVIRONMENT_KEY`)
- `--revision` — applied only to apps explicitly listed in the file; dependencies deploy at the revision resolved from the producer graph
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
