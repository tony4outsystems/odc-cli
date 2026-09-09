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

During development, use `go run ./cmd/odc ...`. The CLI uses [Cobra](https://github.com/spf13/cobra) for command and flag parsing. The compiled binary needs no separate runtime. Run `odc completion --help` for shell completion setup.

## Setup

Save your credentials with:

```bash
odc login https://<your-tenant>.outsystems.dev <client-id>
```

The command interactively prompts for the client secret with input hidden, then saves all three values in `~/.odc/config.json`. The file contains the secret in plain text and has owner-only read/write permissions (`0600`). Running `login` again replaces the saved configuration; it saves credentials without validating them against the server. Never pass the client secret as a CLI argument.

Auth is loaded from the nearest `.env` file in the current directory or a parent directory. If no `.env` is found, the CLI uses `~/.odc/config.json` instead. Existing environment variables take precedence over either file. An incomplete `.env` does not fall back to the saved configuration. Required environment variables when using `.env`:

- `ODC_TENANT_URL`
- `ODC_CLIENT_ID`
- `ODC_CLIENT_SECRET`

App and environment are not read from `.env` — pass `--app`/`--env` explicitly to each command that needs them. Either accepts a name or a key. App names require an exact, case-insensitive match; environment names also accept a unique partial match. Ambiguous names produce suggestions and stop the command.

### Getting your tenant URL, client ID, and client secret

1. Sign in to your organization's ODC Portal (e.g. `https://<your-tenant>.outsystems.dev/`). That URL is `ODC_TENANT_URL`.
2. Go to **Management > API Clients** (or navigate directly to `https://<your-tenant>.outsystems.dev/usersaccess/apiclients`).
3. Create a new **API Client**, grant it the permissions the commands you'll run need (e.g. **Application management** for build/publish/deploy, **User management** for `get-user`/`update-user`), and save it.
4. Run `odc login <tenant-url> <client-id>` and enter the generated **Client Secret**, or copy the **Client ID** and **Client Secret** into `.env` as `ODC_CLIENT_ID` and `ODC_CLIENT_SECRET` — the secret is only shown once, so store it now. Copy [.env.example](.env.example) to `.env` as a starting point.
5. Run `odc discover` to check the tenant URL; it fetches OIDC metadata without needing an app or environment.

## Quick start

```bash
odc discover
odc validate --app <app-name-or-key> --env <environment-name-or-key>
odc latest-revision --app <app-name-or-key>
odc deploy --app <app-name-or-key> --env <environment-name-or-key>
```

The `validate` command confirms that the given app and environment keys are visible to the API client, then prints a short summary of both objects. The `deploy` command runs the same validation, selects the current app revision (falling back to the latest), starts a Release build, waits for it to finish, then deploys it to the given environment.

## Terminology

Use `--app` wherever commands previously used `--asset`. Help, messages, and Go identifiers use app terminology. JSON responses retain ODC API field names such as `assetKey` and `assetType`; API paths and the checked-in API specifications retain their official names. Human-readable output labels use App.

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

Confirm the given app and environment are visible to the API client.

```bash
odc validate --app <app-name-or-key> --env <environment-name-or-key> [--revision <revision>]
```

- `--revision` — defaults to the app's current revision (falls back to the latest if that isn't available)

#### latest-revision

Print the latest revision number for an app.

```bash
odc latest-revision --app <app-name-or-key>
```

#### list-environments

List environments visible to the API client (name, key, type).

```bash
odc list-environments
```

#### list-apps

List apps visible to the API client (name, key, type).

```bash
odc list-apps [--type WebApplication] [--search eGov]
```

- `--type` — filter by app type: `WebApplication`, `MobileApplication`, `LowCodeLibrary`, `ExtensionLibrary`, `ExternalConnection`, `ExternalLibrary`, `Workflow`, `WidgetLibrary`, `AIModelConnection`, `SearchServiceConnection`, `Agent`, `MCPConnection`, `A2AConnection`, `KnowledgeBase`
- `--search` — filter by a name/key substring (case-insensitive); combine with `--type` to narrow further

#### list-deployed-apps

List apps deployed across all visible environments, including their deployed revision, tag, URL, and deployment details.

```bash
odc list-deployed-apps [--env <environment-name-or-key>] [--search <name-or-key-substring>]
```

`--search` matches app names or keys case-insensitively. Omit `--env` to include all visible environments, or supply it to filter to one environment. Each row includes its environment key; all result pages are fetched.

#### list-revisions / get-revision

List all revisions of an app, or retrieve one specific revision.

```bash
odc list-revisions --app <app-name-or-key>
odc get-revision --app <app-name-or-key> --revision <revision>
```

Both commands also accept the app as a positional argument. `get-revision` requires a positive revision number.

#### analyze-deployment / analyze-deletion

Run impact analysis and print the resulting report. These commands do not deploy or delete the app.

```bash
odc analyze-deployment --app <app-name-or-key> --env <environment-name-or-key> [--revision <revision>]
odc analyze-deletion --app <app-name-or-key>
```

Both commands also accept the app as a positional argument. Deployment analysis defaults to the latest revision. Deletion analysis applies to the whole app and takes no environment or revision.

By default, commands poll until analysis finishes. Use `--poll-interval` (default 10 seconds), `--timeout` (default 1800 seconds), or `--no-wait` to return the analysis key immediately. Processing failures and timeouts return an error. A completed analysis prints its report, including any warnings or errors found; findings themselves do not change the exit status.

#### get-app

Retrieve a single app by name or key.

```bash
odc get-app <app-name-or-key>
```

#### producer-graph

Generate a Mermaid graph of an app's producer dependencies.

```bash
odc producer-graph <app-name-or-key> [--max-depth 2] [--output graph.mmd]
```

- `app_key` — positional (also settable via `--app`); name or key
- `--revision` — defaults to the latest revision
- `--env` — environment context for resolving producers
- `--max-depth` — maximum producer depth to traverse; `0` (default) means infinite
- `--producer-type-filter` — `Deployable` (default), `Libraries`, or `All`
- `--all-producers` — shortcut for `--producer-type-filter All`
- `--output` — output path; defaults to `producer-graph-<app>-rev-<revision>.mmd`

#### get-user

Retrieve user information by user key (UUID) or email address.

```bash
odc get-user <user-key-or-email>
```

### Deploying

#### deploy

Validate, select the current app revision, build (Release by default), wait for the build to finish, then deploy — all for one app/environment.

```bash
odc deploy --app <app-name-or-key> --env <environment-name-or-key> [--revision <revision>]
```

- `--revision` — defaults to the app's current revision (falls back to the latest if unavailable)
- `--build-type` — `Debug` or `Release` (default `Release`)

#### batch-deploy

Build and deploy every app listed in a text file, one app per line: `app_key` to deploy its latest revision, or `app_key@revision` to pin a specific one (blank lines and `#` comments ignored). See [examples/10-clicks-demos.txt](examples/10-clicks-demos.txt) for an example. Per-app pinning avoids the ambiguity of a single `--revision` flag when the file lists apps that need different revisions.

```bash
odc batch-deploy examples/10-clicks-demos.txt --env <environment-name-or-key> [--build-type Release] [--skip-dependencies]
```

By default, each app's producer dependencies are resolved via the producer graph, deduplicated across all listed apps, and deployed before the apps that need them. Dependencies deploy at the revision resolved from the producer graph. Cycles and conflicting revisions for the same app are rejected before any builds start. Apps without a pinned revision use the latest revision when dependency planning is enabled; with `--skip-dependencies`, they use the current revision, falling back to the latest.

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
odc undeploy --app <app-name-or-key> --env <environment-name-or-key>
```

#### dangerous-batch-undeploy-all

**Irreversible.** Undeploys every app currently deployed to an environment. Double-check `--env` before running this.

```bash
odc dangerous-batch-undeploy-all --env <environment-name-or-key>
```

Options:

- `--env` — environment name or key (required)
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

### App management

#### delete-app

Permanently delete an app from the app repository.

```bash
odc delete-app --app <app-name-or-key>
```

#### update-user

Update a user's name, active status, or photo URL. At least one of `--name`, `--is-active`, or `--photo-url` is required.

```bash
odc update-user <user-key-or-email> --name "Jane Doe" --is-active true --photo-url https://example.com/photo.jpg
```

### Internal single-step operations

`internal-build`, `internal-publish`, and `internal-deploy` are the raw single-step operations that `deploy` and `batch-deploy` are built from. Reach for them only when you need to drive one step in isolation — e.g. deploying a build that already exists via `internal-deploy --build-key <build-key>`. Each requires `--app`/`--env` and polls until the operation finishes (`--no-wait` to skip polling):

```bash
odc internal-build --app <app-name-or-key> --env <environment-name-or-key> [--revision 1] [--build-type Release]
odc internal-publish --app <app-name-or-key> --env <environment-name-or-key> [--revision 1]
odc internal-deploy --app <app-name-or-key> --env <environment-name-or-key> [--revision 1] --build-key <build-key>
```

- `--revision` — defaults to the app's current revision (falls back to the latest if that isn't available)
- `--build-type` — `Debug` or `Release` (default `Release`; `internal-build` only)
- `internal-deploy` also requires `--build-key <build-key>`

### Shared polling and parallel options

Every command that starts and waits on an operation (`validate`, `deploy`, `internal-build`, `internal-publish`, `internal-deploy`, `undeploy`, `batch-deploy`, `dangerous-batch-undeploy-all`) accepts:

- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — positive seconds to wait before giving up (default `1800`)
- `--no-wait` — return after starting the operation instead of polling (`internal-build`/`internal-publish`/`internal-deploy`/`undeploy` only)

Commands that run multiple apps in parallel (`batch-deploy`, `dangerous-batch-undeploy-all`) additionally accept:

- `--max-parallel` — maximum apps to process concurrently (default `3`)
- `--continue-on-error` — keep going on remaining apps if one fails, instead of stopping. Only fully honored when `--max-parallel 1`; with concurrency, in-flight apps are not cancelled on a failure either way

## TODO

* Claude skill
* Readme: Terminal session demo
* cli autocompletion
* Support Portfolio


Tech
- Settings in user home directory