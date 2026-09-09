---
name: odc-cli-helper
description: |
  Install, configure, and use the OutSystems ODC CLI tool. Trigger this skill whenever the user mentions "odc-cli", wants to deploy apps to ODC, list apps in their tenant, analyze dependencies, or manage app deployments and revisions. The skill handles installation guidance (Homebrew on macOS, build from source on Windows/Linux), credential setup via `odc login`, and runs CLI commands directly on the user's machine. Use this skill whenever the user wants to: deploy all apps matching a name pattern, list apps in their environment, display dependency graphs, validate environments, check app revisions, or perform any other ODC CLI operation.
---

# ODC CLI Helper

You help OutSystems users install, configure, and use the ODC CLI tool to manage applications in the OutSystems Developer Cloud. This is a command-line tool for deploying apps, managing revisions, analyzing dependencies, and inspecting app metadata.

## Quick reference

The ODC CLI is installed as the `odc` command. Common use cases:

- **Deploy an app**: `odc deploy --app <app-name> --env <environment>`
- **List all apps**: `odc list-apps`
- **List apps deployed to an environment**: `odc list-deployed-apps --env <environment>`
- **Show app dependencies**: `odc producer-graph <app-name>`
- **Validate setup**: `odc discover` (checks OIDC discovery) or `odc validate --app <app-name> --env <environment>`

## Installation

### macOS (Homebrew)

```bash
brew install tony4outsystems/tap/odc-cli
```

To upgrade: `brew update && brew upgrade odc-cli`

### Windows, Linux, or building from source

Requires Go 1.23 or later:

```bash
git clone https://github.com/tony4outsystems/odc-cli.git
cd odc-cli
go install ./cmd/odc
```

Ensure `$(go env GOPATH)/bin` is on your `PATH`. On Windows, this is typically `%USERPROFILE%\go\bin` — add it to your system PATH via Settings > System > Environment Variables.

To build a local binary without installing globally:

```bash
go build -o bin/odc ./cmd/odc
./bin/odc --help  # or bin\odc --help on Windows
```

## Configuration

### Step 1: Get your credentials

1. Sign in to your ODC Portal (`https://<your-tenant>.outsystems.dev/`)
2. Go to **Management > API Clients** (or direct link: `https://<your-tenant>.outsystems.dev/usersaccess/apiclients`)
3. Create a new **API Client** with the permissions you need (e.g., **Application management** for deploy/build, **User management** for user commands)
4. Save the **Client Secret** — it's only shown once
5. Note your **Tenant URL**, **Client ID**, and **Client Secret**

### Step 2: Save credentials with `odc login`

```bash
odc login https://<your-tenant>.outsystems.dev <client-id>
```

This prompts for your client secret interactively (input is hidden), then saves all three values in `~/.odc/config.json` with owner-only permissions.

### Step 3: Verify setup

```bash
odc discover
```

This fetches OIDC metadata without needing an app or environment — a quick sanity check that your tenant URL is correct.

## Common commands

All commands below assume your credentials are configured. Wherever you see `<app-name-or-key>` or `<environment-name-or-key>`, you can use the app/environment name (exact, case-insensitive) or its key. Environment names accept unique partial matches.

### Inspection

**List all apps:**
```bash
odc list-apps
```

Filter by type or name:
```bash
odc list-apps --type WebApplication
odc list-apps --search eGov
odc list-apps --type WebApplication --search eGov
```

**List apps deployed to an environment:**
```bash
odc list-deployed-apps --env Production
```

**Get details of one app:**
```bash
odc get-app eGovPortal
```

**List all environments:**
```bash
odc list-environments
```

**Show an app's dependency graph (producers):**
```bash
odc producer-graph eGovPortal
odc producer-graph eGovPortal --max-depth 2  # limit depth
odc producer-graph eGovPortal --output deps.mmd  # save to file
```

**Get latest revision number:**
```bash
odc latest-revision --app eGovPortal
```

**Analyze deployment impact (what changes if you deploy?):**
```bash
odc analyze-deployment --app eGovPortal --env Production
```

**Analyze deletion impact:**
```bash
odc analyze-deletion --app eGovPortal
```

### Deploying

**Deploy a single app:**
```bash
odc deploy --app eGovPortal --env Production
```

With a specific revision:
```bash
odc deploy --app eGovPortal --env Production --revision 42
```

Build type (default is `Release`):
```bash
odc deploy --app eGovPortal --env Production --build-type Debug
```

**Deploy multiple apps from a file:**

Create a text file with one app per line (`app_key` or `app_key@revision`). Blank lines and `#` comments are ignored:

```
eGovPortal
eGovAdmin@12
eGovCommon
```

Then:
```bash
odc batch-deploy apps.txt --env Production
```

Options:
- `--max-parallel 5` — deploy up to 5 apps concurrently (default 3)
- `--continue-on-error` — keep going if one app fails
- `--skip-dependencies` — deploy only listed apps, not their dependencies

By default, `batch-deploy` resolves and deploys all producer dependencies automatically.

### Undeploying

**Undeploy one app:**
```bash
odc undeploy --app eGovPortal --env Production
```

**Undeploy ALL apps in an environment (irreversible):**
```bash
odc dangerous-batch-undeploy-all --env Production
```

### App management

**Delete an app from the repository (irreversible):**
```bash
odc delete-app --app eGovPortal
```

## Output formats

By default, results show as readable tables or labeled fields with color (when output is to a terminal).

**JSON output** (for scripts or piping):
```bash
odc list-apps --json
odc deploy --app eGovPortal --env Production --json
```

**Control color:**
```bash
odc list-apps --color always   # force color
odc list-apps --color never    # no color
odc list-apps --color auto     # auto (default)
```

## Troubleshooting

**"Command not found: odc"**
- macOS: Did Homebrew installation complete? Try `brew install tony4outsystems/tap/odc-cli` again, then restart your terminal.
- Other platforms: Ensure `$(go env GOPATH)/bin` (or `%USERPROFILE%\go\bin` on Windows) is in your system `PATH`.

**"invalid credentials" or "unauthorized"**
- Run `odc discover` to verify the tenant URL is correct (should return OIDC metadata).
- Check that your API Client has the required permissions in the ODC Portal (e.g., **Application management** for deploy commands).
- Verify your credentials were saved correctly: `odc login` should have created `~/.odc/config.json`.

**"app not found" or "ambiguous name"**
- The API client may not have visibility into that app. Run `odc list-apps` to see what's available.
- Environment names accept unique partial matches; app names require an exact match (case-insensitive).

**"operation timed out"**
- Builds and deployments can take time. Increase the timeout with `--timeout 3600` (in seconds).
- Use `--poll-interval 20` to poll less frequently and reduce API load.

## Tips

1. **Test your setup before deploying to production:** Run `odc validate --app <app> --env <env>` to check visibility and get a summary.
2. **Dependency resolution is automatic:** `batch-deploy` finds and deploys producers by default; use `--skip-dependencies` only if you know what you're doing.
3. **Dry-run with `--analyze-deployment`:** Before deploying, run this to see what would change.
4. **Redirected output loses color:** Use `--color always` if you want color even when piping to a file.
