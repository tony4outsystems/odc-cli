# OutSystems ODC CLI

CLI (`odc`) for driving the OutSystems Developer Cloud (ODC) APIs described in `api-specs/`.

![odc demo](demo.gif)

## Install

### Homebrew (macOS)

```bash
brew install tony4outsystems/tap/odc-cli
```

To upgrade an existing installation, run `brew update && brew upgrade odc-cli`. The tap installs a prebuilt Rust binary (no build step).

### Download a binary

Download an archive from [GitHub Releases](https://github.com/tony4outsystems/odc-cli/releases) for your operating system (`windows`, `darwin` for macOS, or `linux`) and architecture (`amd64` for Intel/AMD, `arm64` for ARM, including Apple silicon). Extract it and put `odc` (or `odc.exe` on Windows) in a directory on your `PATH`. No Rust installation is required.

Windows archives use `.zip`; macOS and Linux archives use `.tar.xz`. Each binary release includes `checksums.txt` with SHA-256 hashes of the archives.

### Build from source

Requires Rust 1.85 or later (install with [rustup](https://rustup.rs)). Build and install from a checkout:

```bash
git clone https://github.com/tony4outsystems/odc-cli.git
cd odc-cli
cargo install --path .
```

Ensure `~/.cargo/bin` is on your `PATH`. To build a local binary instead:

```bash
cargo build --release
./target/release/odc --help
```

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

Asset and environment are not read from `.env` — pass `--asset`/`--env` explicitly to each command that needs them. Either accepts a name or a key. Asset names require an exact, case-insensitive match; environment names also accept a unique partial match. Ambiguous names produce suggestions and stop the command.

### Getting your tenant URL, client ID, and client secret

1. Sign in to your organization's ODC Portal (e.g. `https://<your-tenant>.outsystems.dev/`). That URL is `ODC_TENANT_URL`.
2. Go to **Management > API Clients** (or navigate directly to `https://<your-tenant>.outsystems.dev/usersaccess/apiclients`).
3. Create a new **API Client**, grant it the permissions the commands you'll run need (e.g. **Application management** for build/publish/deploy, **User management** for `get-user`/`update-user`/`grant-role`/`revoke-role`), and save it.
4. Run `odc login <tenant-url> <client-id>` and enter the generated **Client Secret**, or copy the **Client ID** and **Client Secret** into `.env` as `ODC_CLIENT_ID` and `ODC_CLIENT_SECRET` — the secret is only shown once, so store it now. Copy [.env.example](.env.example) to `.env` as a starting point.
5. Run `odc discover` to check the tenant URL; it fetches OIDC metadata without needing an app or environment.

## Quick start

```bash
odc discover
odc latest-revision --asset <asset-name-or-key>
odc deploy --asset <asset-name-or-key> --env <environment-name-or-key>
```

The `deploy` command confirms the given asset and environment keys are visible to the API client, selects the current asset revision (falling back to the latest), starts a Release build, waits for it to finish, then deploys it to the given environment.

## Claude Code skill

This repo includes a [Claude Code](https://claude.com/claude-code) skill at [skills/odc-cli-helper](skills/odc-cli-helper/SKILL.md) that teaches Claude how to install, configure, and drive the `odc` CLI on your behalf — deploying apps, listing dependencies, checking revisions, and so on.

To use it, copy the skill into a Claude Code skills directory so it loads automatically:

```bash
mkdir -p ~/.claude/skills
cp -r skills/odc-cli-helper ~/.claude/skills/
```

(Use `.claude/skills/` inside a specific project instead of `~/.claude/skills/` to scope it to that project only.)

Once installed, ask Claude Code things like "deploy MyApp to Production with odc" or "list all apps in my ODC tenant" and it will invoke the skill automatically.

## Output

Results use readable tables and labeled fields by default. Terminal output includes colored headings and status values; redirected output is plain text.

List commands (`list-assets`, `list-deployed-assets`, `list-revisions`) show only a few key columns in their table (e.g. name, key, type, revision, tag) rather than every field the API returns — the full record, guids and all, is always available with `--json`.

- `--json` prints JSON for scripts. Commands with multiple stages emit successive JSON values, with progress messages on stderr.
- `--color auto|always|never` controls ANSI colors (default `auto`). Automatic color respects `NO_COLOR` and `TERM=dumb`.

```bash
odc list-assets
odc get-asset MyAsset --color always
odc list-assets --json > assets.json
```

## Commands

```
$ odc --help
OutSystems ODC CLI

Usage: odc [OPTIONS] <COMMAND>

Auth:
  discover                        Show the OAuth discovery document (issuer, endpoints, scopes)
  login                           Save credentials in ~/.odc/config.json (prompts for client secret)

Portfolios:
  list-portfolios                 List portfolios in the tenant

Assets & Environments:
  list-environments               List environments in the tenant
  list-assets                     List assets in the tenant, optionally filtered by name/key and/or type
  list-deployed-assets            List deployed assets, optionally filtered by environment and name/key
  get-asset                       Retrieve asset metadata
  delete-asset                    Delete an asset

Revisions & Source:
  latest-revision                 Print the latest revision number of an asset
  list-revisions                  List all revisions of an asset
  get-revision                    Retrieve a specific asset revision
  producer-graph                  Render an asset's producer dependency graph as Mermaid
  download-source-code            Download the OML source code of an asset revision
  upload-source-code              Upload an OML/XIF file, creating a new asset or revision

Deployment:
  analyze-deployment              Analyze the impact of deploying an asset revision
  analyze-deletion                Analyze the impact of deleting an asset
  deploy                          Deploy an asset to an environment
  undeploy                        Undeploy an asset from an environment

Batch Operations:
  batch-deploy                    Deploy multiple assets listed in a file
  batch-undeploy                  Undeploy multiple assets listed in a file
  batch-delete                    Delete multiple assets listed in a file
  dangerous-batch-undeploy-all    Undeploy all assets from an environment

Users:
  get-user                        Retrieve a user's details
  update-user                     Update a user's name, active status, or photo URL

Groups:
  list-groups                     List end-user groups, optionally filtered by name and/or environment
  get-group                       Retrieve an end-user group's details
  update-group                    Update a group's name or description
  list-group-members              List the members of an end-user group
  add-user-to-group               Add a user to an end-user group
  remove-user-from-group          Remove a user from an end-user group

Roles:
  list-roles                      List application roles defined for an asset
  list-role-assignments           List, for each application role of an asset, the users and/or groups assigned to it
  grant-role                      Grant an application role to a user
  revoke-role                     Revoke an application role from a user
  grant-group-role                Grant an application role to an end-user group
  revoke-group-role               Revoke an application role from an end-user group

Internal (Advanced):
  internal-build                  Start a build for an asset revision
  internal-publish                Publish a build to an environment
  internal-deploy                 Deploy an existing build to an environment

Mentor:
  mentor-prompt                   Send a prompt to Mentor and wait for completion, auto-publishing the result

Mentor (Advanced):
  mentor-start-session            Start a new Mentor session; prints the sessionId used for follow-up commands
  mentor-create-asset             Create a new asset in a Mentor session, so `mentor-prompt` can edit it
  mentor-load-asset               Load an existing asset into a Mentor session, so `mentor-prompt` can edit it
  mentor-prompt-raw               Send a prompt to a Mentor session; returns a runId to poll with `mentor-get-run` (low-level)
  mentor-get-run                  Poll a Mentor run's progress events and status
  mentor-get-event                Fetch the full body of a truncated Mentor run event
  mentor-cancel-prompt            Cancel the in-flight prompt for a Mentor session
  mentor-request-upload           Mint a presigned upload URL for a Mentor session attachment
  mentor-publish                  Publish the asset loaded in a Mentor session to the connected dev environment
  mentor-close-session            Close a Mentor session and release its resources

Misc:
  completion                      Generate shell completion scripts

Options:
      --json                         Output in JSON format
      --color <COLOR>                Control color output
  -n, --no-resolve                   Disable resolution of external keys (environment names, role names, etc.) in user-friendly output
  -h, --help           Print help
  -V, --version        Print version
```

To enable shell completion, generate the script for your shell and source or install it — e.g. for bash, `source <(odc completion bash)` for the current session, or `odc completion bash > /etc/bash_completion.d/odc` to install it; `zsh`, `fish`, `powershell`, and `elvish` are also supported.

In the usage examples below, arguments in `[brackets]` are optional (with a default or a resolved fallback); everything else is required.

### Inspection

#### discover

Fetch OIDC discovery metadata (issuer, token endpoint, supported scopes).

```bash
odc discover
```

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

#### list-assets

List assets visible to the API client (name, key, type).

```bash
odc list-assets [name-or-key-substring] [--type WebApplication]
```

An optional positional filters to assets whose name or key contains it (case-insensitive). `--type` filters to an exact asset type; run `odc list-assets --help` for the full list of recognized values (`WebApplication`, `Agent`, `LowCodeLibrary`, ...).

See [Pagination](#pagination) for `--offset`/`--limit`.

#### list-deployed-assets

List assets deployed across all visible environments, including their deployed revision, tag, and environment. One row per asset/environment deployment.

```bash
odc list-deployed-assets [name-or-key-substring] [--env <environment-name-or-key>]
```

The positional filters to assets whose name or key contains it (case-insensitive). `--env` narrows to one environment (name, key, or unambiguous partial name — an unresolvable value is an error, it never matches nothing silently); omit it to include every environment the asset is deployed to.

See [Pagination](#pagination) for `--offset`/`--limit`.

#### list-revisions / get-revision

List all revisions of an asset, or retrieve one specific revision.

```bash
odc list-revisions --asset <asset-name-or-key>
odc get-revision --asset <asset-name-or-key> --revision <revision>
```

Both commands also accept the asset as a positional argument. `get-revision` requires a positive revision number. `list-revisions` supports `--offset`/`--limit`; see [Pagination](#pagination).

#### download-source-code

Download the OML source code of an asset revision.

```bash
odc download-source-code <asset-name-or-key> [--revision <revision>] [--output <path>]
```

- `--revision` — defaults to the latest revision
- `--output` — output file path; defaults to `<asset-key>-rev-<revision>.oml`

#### upload-source-code

Upload an OML/XIF file, creating a new asset (first revision) or a new revision of an existing one. The asset key is derived from the file's embedded module key, not from a name or `--asset` flag.

```bash
odc upload-source-code <oml-file>
```

Combine with `deploy --asset <asset-key> --env <environment>` to build and deploy the uploaded revision.

#### analyze-deployment / analyze-deletion

Run impact analysis and print the resulting report. These commands do not deploy or delete the asset.

```bash
odc analyze-deployment --asset <asset-name-or-key> --env <environment-name-or-key> [--revision <revision>]
odc analyze-deletion --asset <asset-name-or-key>
```

Both commands also accept the asset as a positional argument. Deployment analysis defaults to the latest revision. Deletion analysis applies to the whole asset and takes no environment or revision.

By default, commands poll until analysis finishes. Use `--poll-interval` (default 10 seconds), `--timeout` (default 1800 seconds), or `--no-wait` to return the analysis key immediately. Processing failures and timeouts return an error. A completed analysis prints its report, including any warnings or errors found; findings themselves do not change the exit status.

#### get-asset

Retrieve a single asset by name or key.

```bash
odc get-asset <asset-name-or-key>
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

Confirm the given asset and environment are visible to the API client, select the current asset revision, build (Release by default), wait for the build to finish, then deploy — all for one asset/environment.

```bash
odc deploy --asset <asset-name-or-key> --env <environment-name-or-key> [--revision <revision>]
```

- `--revision` — defaults to the asset's current revision (falls back to the latest if unavailable)
- `--build-type` — `Debug` or `Release` (default `Release`)

#### batch-deploy

Build and deploy every asset listed in a text file, one asset per line: `asset_key` to deploy its latest revision, or `asset_key@revision` to pin a specific one (blank lines and `#` comments ignored). See [examples/10-clicks-demos.txt](examples/10-clicks-demos.txt) for an example. Per-asset pinning avoids the ambiguity of a single `--revision` flag when the file lists assets that need different revisions.

```bash
odc batch-deploy examples/10-clicks-demos.txt --env <environment-name-or-key> [--build-type Release] [--skip-dependencies]
```

By default, each asset's producer dependencies are resolved via the producer graph, deduplicated across all listed assets, and deployed before the assets that need them. Dependencies deploy at the revision resolved from the producer graph. Cycles and conflicting revisions for the same asset are rejected before any builds start. Assets without a pinned revision use the latest revision when dependency planning is enabled; with `--skip-dependencies`, they use the current revision, falling back to the latest.

Options:

- `--env` — environment name or key (required)
- `--build-type` — `Debug` or `Release` (default `Release`)
- `--skip-dependencies` — deploy only the assets listed in the file, without automatically including their producer dependencies
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

Example with overrides:

```bash
odc batch-deploy examples/10-clicks-demos.txt --env <env> --build-type Release --max-parallel 3 --continue-on-error
```

### Undeploying

#### undeploy

Undeploy a single asset from an environment.

```bash
odc undeploy --asset <asset-name-or-key> --env <environment-name-or-key>
```

#### dangerous-batch-undeploy-all

**Irreversible.** Undeploys every asset currently deployed to an environment. Double-check `--env` before running this.

```bash
odc dangerous-batch-undeploy-all --env <environment-name-or-key>
```

Options:

- `--env` — environment name or key (required)
- see [Shared polling and parallel options](#shared-polling-and-parallel-options) below

### Asset management

#### delete-asset

Permanently delete an asset from the asset repository.

```bash
odc delete-asset --asset <asset-name-or-key>
```

#### update-user

Update a user's name, active status, or photo URL. At least one of `--name`, `--is-active`, or `--photo-url` is required.

```bash
odc update-user <user-key-or-email> --name "Jane Doe" --is-active true --photo-url https://example.com/photo.jpg
```

#### grant-role / revoke-role

Grant or revoke an application role for a user. The asset disambiguates which asset's role to use when the same role name exists on multiple assets.

```bash
odc grant-role <user-key-or-email> <role-name-or-key> --asset <asset-name-or-key>
odc revoke-role <user-key-or-email> <role-name-or-key> --asset <asset-name-or-key>
```

The API client needs the **User management > Manage end-user access** permission.

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

## Pagination

`list-assets`, `list-deployed-assets`, and `list-revisions` list results from API endpoints that
page their results. By default each of these commands fetches every page and returns the
combined result, so no flags are needed for the common case.

- `--offset` — fetch a single page starting at this result index, instead of every page. With `--json`, the response is wrapped in `{"results": [...], "page": {"offset", "limit", "nextOffset"}}` so `page.nextOffset` can be passed as the next `--offset` (it is `null` once there are no more pages).
- `--limit` — page size to request from the API (default `100`); applies whether or not `--offset` is set.

```bash
odc list-assets --offset 100 --limit 50
odc list-revisions --app MyApp --offset 0 --limit 20 --json
```

### Shared polling and parallel options

Every command that starts and waits on an operation (`deploy`, `internal-build`, `internal-publish`, `internal-deploy`, `undeploy`, `batch-deploy`, `dangerous-batch-undeploy-all`) accepts:

- `--poll-interval` — seconds between status polls (default `10`)
- `--timeout` — positive seconds to wait before giving up (default `1800`)
- `--no-wait` — return after starting the operation instead of polling (`internal-build`/`internal-publish`/`internal-deploy`/`undeploy` only)

Commands that run multiple apps in parallel (`batch-deploy`, `dangerous-batch-undeploy-all`) additionally accept:

- `--max-parallel` — maximum apps to process concurrently (default `3`)
- `--continue-on-error` — keep going on remaining apps if one fails, instead of stopping. Only fully honored when `--max-parallel 1`; with concurrency, in-flight apps are not cancelled on a failure either way

## Development

For running tests and cutting a release, see [DEVELOPMENT.md](DEVELOPMENT.md).
