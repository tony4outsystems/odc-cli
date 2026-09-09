# OutSystems ODC CLI

Small Go CLI (`odc`) for driving the OutSystems Developer Cloud (ODC) APIs described in `api-specs/`.

![odc demo](demo.gif)

## Install

On macOS, install with Homebrew:

```bash
brew install tony4outsystems/tap/odc-cli
```

To upgrade an existing installation, run `brew update && brew upgrade odc-cli`. Homebrew builds the Go binary from source.

### Build from source

Requires Go 1.23 or later. Build and install from a checkout:

```bash
git clone https://github.com/tony4outsystems/odc-cli.git
cd odc-cli
go install ./cmd/odc
```

Ensure `$(go env GOPATH)/bin` is on your `PATH`. To build a local binary instead:

```bash
go build -o bin/odc ./cmd/odc
./bin/odc --help
```

During development, use `go run ./cmd/odc ...`. The binary has no Python or third-party Go runtime dependencies.

## Setup

Auth is read from environment variables, loaded from the nearest `.env` file in the current directory or a parent directory (existing environment variables take precedence) (never pass credentials as CLI arguments — they'd leak into shell history and process listings). Required:

- `ODC_TENANT_URL`
- `ODC_CLIENT_ID`
- `ODC_CLIENT_SECRET`

Asset and environment are not read from `.env` — pass `--asset`/`--env` explicitly to each command that needs them. Either accepts a name or a key. Asset names require an exact, case-insensitive match; environment names also accept a unique partial match. Ambiguous names produce suggestions and stop the command.

### Getting your tenant URL, client ID, and client secret

1. Sign in to your organization's ODC Portal (e.g. `https://<your-tenant>.outsystems.dev/`). That URL is `ODC_TENANT_URL`.
2. Go to **Management > API Clients** (or navigate directly to `https://<your-tenant>.outsystems.dev/usersaccess/apiclients`).
3. Create a new **API Client**, grant it the permissions the commands you'll run need (e.g. **Application management** for build/publish/deploy, **User management** for `get-user`/`update-user`), and save it.
4. Copy the generated **Client ID** and **Client Secret** into `.env` as `ODC_CLIENT_ID` and `ODC_CLIENT_SECRET` — the secret is only shown once, so store it now. Copy [.env.example](.env.example) to `.env` as a starting point.
5. Run `odc discover` to confirm the tenant URL and credentials are correct; it fetches OIDC metadata without needing an asset or environment.

## Quick start

```bash
odc discover
odc validate --asset <asset-name-or-key> --env <environment-name-or-key>
odc latest-revision --asset <asset-name-or-key>
odc deploy --asset <asset-name-or-key> --env <environment-name-or-key>
```

The `validate` command confirms that the given asset and environment keys are visible to the API client, then prints a short summary of both objects. The `deploy` command runs the same validation, selects the current asset revision (falling back to the latest), starts a Release build, waits for it to finish, then deploys it to the given environment.

## Output

Results use readable tables and labeled fields by default. Terminal output includes colored headings and status values; redirected output is plain text.

- `--json` prints JSON for scripts. Commands with multiple stages emit successive JSON values, with progress messages on stderr.
- `--color auto|always|never` controls ANSI colors (default `auto`). Automatic color respects `NO_COLOR` and `TERM=dumb`.

```bash
odc list-apps
odc get-app MyApp --color always
odc list-apps --json > apps.json
```

## Commands

In the usage examples below, arguments in `[brackets]` are optional (with a default or a resolved fallback); everything else is required.

### Inspection

#### discover

Fetch OIDC discovery metadata (issuer, token endpoint, supported scopes).

```bash
odc discover
```

#### validate

Confirm the given asset and environment are visible to the API client.

```bash
odc validate --asset <asset-name-or-key> --env <environment-name-or-key> [--revision <revision>]
```

- `--revision` — defaults to the asset's current revision (falls back to the latest if that isn't available)

#### latest-revision

Print the latest revision number for an asset.

```bash
odc latest-revision --asset <asset-name-or-key>
```

#### list-environments

List environments visible to the API client (name, key, type).

```bash
odc list-environments
```

#### list-apps

List assets visible to the API client (name, key, type).

```bash
odc list-apps [--type WebApplication] [--search eGov]
```

- `--type` — filter by asset type: `WebApplication`, `MobileApplication`, `LowCodeLibrary`, `ExtensionLibrary`, `ExternalConnection`, `ExternalLibrary`, `Workflow`, `WidgetLibrary`, `AIModelConnection`, `SearchServiceConnection`, `Agent`, `MCPConnection`, `A2AConnection`, `KnowledgeBase`
- `--search` — filter by a name/key substring (case-insensitive); combine with `--type` to narrow further

#### get-app

Retrieve a single asset by name or key.

```bash
odc get-app <asset-name-or-key>
```

#### producer-graph

Generate a Mermaid graph of an asset's producer dependencies.

```bash
odc producer-graph <asset-name-or-key> [--max-depth 2] [--output graph.mmd]
```

- `asset_key` — positional (also settable via `--asset`); name or key
- `--revision` — defaults to the latest revision
- `--env` — environment context for resolving producers
- `--max-depth` — maximum producer depth to traverse; `0` (default) means infinite
- `--producer-type-filter` — `Deployable` (default), `Libraries`, or `All`
- `--all-producers` — shortcut for `--producer-type-filter All`
- `--output` — output path; defaults to `producer-graph-<asset>-rev-<revision>.mmd`

#### get-user

Retrieve user information by user key (UUID) or email address.

```bash
odc get-user <user-key-or-email>
```

### Deploying

#### deploy

Validate, select the current asset revision, build (Release by default), wait for the build to finish, then deploy — all for one asset/environment.

```bash
odc deploy --asset <asset-name-or-key> --env <environment-name-or-key> [--revision <revision>]
```

- `--revision` — defaults to the asset's current revision (falls back to the latest if unavailable)
- `--build-type` — `Debug` or `Release` (default `Release`)

#### batch-deploy

Build and deploy every app listed in a text file, one app per line: `asset_key` to deploy its latest revision, or `asset_key@revision` to pin a specific one (blank lines and `#` comments ignored). See [examples/10-clicks-demos.txt](examples/10-clicks-demos.txt) for an example. Per-app pinning avoids the ambiguity of a single `--revision` flag when the file lists apps that need different revisions.

```bash
odc batch-deploy examples/10-clicks-demos.txt --env <environment-name-or-key> [--build-type Release] [--skip-dependencies]
```

By default, each app's producer dependencies are resolved via the producer graph, deduplicated across all listed apps, and deployed before the apps that need them. Dependencies deploy at the revision resolved from the producer graph. Cycles and conflicting revisions for the same asset are rejected before any builds start. Apps without a pinned revision use the latest revision when dependency planning is enabled; with `--skip-dependencies`, they use the current revision, falling back to the latest.

Options:

- `--env` — environment name or key (required)
- `--build-type` — `Debug` or `Release` (default `Release`)
- `--skip-dependencies` — deploy only the apps listed in the file, without automatically including their producer dependencies
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

Example with overrides:

```bash
odc batch-deploy examples/10-clicks-demos.txt --env <env> --build-type Release --max-parallel 3 --continue-on-error
```

### Undeploying

#### undeploy

Undeploy a single app from an environment.

```bash
odc undeploy --asset <asset-name-or-key> --env <environment-name-or-key>
```

#### dangerous-batch-undeploy-all

**Irreversible.** Undeploys every app currently deployed to an environment. Double-check `--env` before running this.

```bash
odc dangerous-batch-undeploy-all --env <environment-name-or-key>
```

Options:

- `--env` — environment name or key (required)
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

### Asset management

#### delete-app

Permanently delete an asset from the asset repository.

```bash
odc delete-app --asset <asset-name-or-key>
```

#### update-user

Update a user's name, active status, or photo URL. At least one of `--name`, `--is-active`, or `--photo-url` is required.

```bash
odc update-user <user-key-or-email> --name "Jane Doe" --is-active true --photo-url https://example.com/photo.jpg
```

### Internal single-step operations

`internal-build`, `internal-publish`, and `internal-deploy` are the raw single-step operations that `deploy` and `batch-deploy` are built from. Reach for them only when you need to drive one step in isolation — e.g. deploying a build that already exists via `internal-deploy --build-key <build-key>`. Each requires `--asset`/`--env` and polls until the operation finishes (`--no-wait` to skip polling):

```bash
odc internal-build --asset <asset-name-or-key> --env <environment-name-or-key> [--revision 1] [--build-type Release]
odc internal-publish --asset <asset-name-or-key> --env <environment-name-or-key> [--revision 1]
odc internal-deploy --asset <asset-name-or-key> --env <environment-name-or-key> [--revision 1] --build-key <build-key>
```

- `--revision` — defaults to the asset's current revision (falls back to the latest if that isn't available)
- `--build-type` — `Debug` or `Release` (default `Release`; `internal-build` only)
- `internal-deploy` also requires `--build-key <build-key>`

### Shared polling and parallel options

Every command that starts and waits on an operation (`validate`, `deploy`, `internal-build`, `internal-publish`, `internal-deploy`, `undeploy`, `batch-deploy`, `dangerous-batch-undeploy-all`) accepts:

- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — positive seconds to wait before giving up (default `1800`)
- `--no-wait` — return after starting the operation instead of polling (`internal-build`/`internal-publish`/`internal-deploy`/`undeploy` only)

Commands that run multiple assets in parallel (`batch-deploy`, `dangerous-batch-undeploy-all`) additionally accept:

- `--max-parallel` — maximum apps to process concurrently (default `3`)
- `--continue-on-error` — keep going on remaining apps if one fails, instead of stopping. Only fully honored when `--max-parallel 1`; with concurrency, in-flight apps are not cancelled on a failure either way

## TODO

* Claude skill
* Readme: Terminal session demo
* cli autocompletion
* Support Portfolio


Tech
- Settings in user home directory