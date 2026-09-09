# OutSystems ODC CLI

Small Python CLI (`odc`) for driving the OutSystems Developer Cloud (ODC) APIs described in `api-specs/`.

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
uv run odc discover
uv run odc validate --asset-key <asset-key> --environment-key <environment-key>
uv run odc latest-revision --asset-key <asset-key>
uv run odc deploy --asset-key <asset-key> --environment-key <environment-key>
```

The `validate` command confirms that the given asset and environment keys are visible to the API client, then prints a short summary of both objects. The `deploy` command runs the same validation, resolves the latest revision, starts a Release build, waits for it to finish, then deploys it to the given environment.

## Commands

### Inspection

#### discover

Fetch OIDC discovery metadata (issuer, token endpoint, supported scopes).

```bash
uv run odc discover
```

#### validate

Confirm the given asset and environment are visible to the API client.

```bash
uv run odc validate --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```

#### latest-revision

Print the latest revision number for an asset.

```bash
uv run odc latest-revision --asset-key <asset-key>
```

#### list-environments

List environments visible to the API client (name, key, type).

```bash
uv run odc list-environments
```

#### producer-graph

Generate a Mermaid graph of an asset's producer dependencies.

```bash
uv run odc producer-graph <asset-key> --max-depth 2 --output graph.mmd
```

- `asset_key` — positional (also settable via `--asset-key`)
- `--revision` — defaults to the latest revision
- `--environment-key` — environment context for resolving producers
- `--max-depth` — maximum producer depth to traverse; `0` (default) means infinite
- `--producer-type-filter` — `Deployable` (default), `Libraries`, or `All`
- `--all-producers` — shortcut for `--producer-type-filter All`
- `--output` — output path; defaults to `producer-graph-<asset>-rev-<revision>.mmd`

#### get-user

Retrieve user information by user key (UUID) or email address.

```bash
uv run odc get-user <user-key-or-email>
```

### Deploying

#### deploy

Validate, resolve the latest revision, build (Release by default), wait for the build to finish, then deploy — all for one asset/environment.

```bash
uv run odc deploy --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```

#### batch-deploy

Build and deploy every app listed in a text file, one app per line: `asset_key` to deploy its latest revision, or `asset_key@revision` to pin a specific one (blank lines and `#` comments ignored). See [apps.txt](apps.txt) for an example. Per-app pinning avoids the ambiguity of a single `--revision` flag when the file lists apps that need different revisions.

```bash
uv run odc batch-deploy apps.txt --environment-key <environment-key>
```

By default, each app's producer dependencies are resolved via the producer graph, deduplicated across all listed apps, and deployed before the apps that need them. Dependencies always deploy at the revision resolved from the producer graph, regardless of any pinned revision on the apps that depend on them.

Options:

- `--environment-key` — environment name or key (required)
- `--build-type` — `Debug` or `Release` (default `Release`)
- `--skip-dependencies` — deploy only the apps listed in the file, without automatically including their producer dependencies
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

Example with overrides:

```bash
uv run odc batch-deploy apps.txt --environment-key <env> --build-type Release --max-parallel 3 --continue-on-error
```

### Undeploying

#### undeploy

Undeploy a single app from an environment.

```bash
uv run odc undeploy --asset-key <asset-key> --environment-key <environment-key>
```

#### dangerous-batch-undeploy-all

**Irreversible.** Undeploys every app currently deployed to an environment. Double-check `--environment-key` before running this.

```bash
uv run odc dangerous-batch-undeploy-all --environment-key <environment-key>
```

Options:

- `--environment-key` — environment name or key (required)
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

### Asset management

#### delete-app

Permanently delete an asset from the asset repository.

```bash
uv run odc delete-app --asset-key <asset-key>
```

#### update-user

Update a user's name, active status, or photo URL. At least one of `--name`, `--is-active`, or `--photo-url` is required.

```bash
uv run odc update-user <user-key-or-email> --name "Jane Doe" --is-active true --photo-url https://example.com/photo.jpg
```

### Internal single-step operations

`internal-build`, `internal-publish`, and `internal-deploy` are the raw single-step operations that `deploy` and `batch-deploy` are built from. Reach for them only when you need to drive one step in isolation — e.g. deploying a build that already exists via `internal-deploy --build-key <build-key>`. Each requires `--asset-key`/`--environment-key` and polls until the operation finishes (`--no-wait` to skip polling):

```bash
uv run odc internal-build --asset-key <asset-key> --environment-key <environment-key> --revision 1 --build-type Release
uv run odc internal-publish --asset-key <asset-key> --environment-key <environment-key> --revision 1
uv run odc internal-deploy --asset-key <asset-key> --environment-key <environment-key> --revision 1 --build-key <build-key>
```

- `--build-type` — `Debug` or `Release` (default `Release`; `internal-build` only)
- `internal-deploy` also requires `--build-key <build-key>`

### Shared polling and parallel options

Every command that starts and waits on an operation (`validate`, `deploy`, `internal-build`, `internal-publish`, `internal-deploy`, `undeploy`, `batch-deploy`, `dangerous-batch-undeploy-all`) accepts:

- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — seconds to wait before giving up (default `1800`)
- `--no-wait` — return after starting the operation instead of polling (`internal-build`/`internal-publish`/`internal-deploy`/`undeploy` only)

Commands that run multiple assets in parallel (`batch-deploy`, `dangerous-batch-undeploy-all`) additionally accept:

- `--max-parallel` — maximum apps to process concurrently (default `3`)
- `--continue-on-error` — keep going on remaining apps if one fails, instead of stopping. Only fully honored when `--max-parallel 1`; with concurrency, in-flight apps are not cancelled on a failure either way
