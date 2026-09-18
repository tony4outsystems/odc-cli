# Example Prompts for the OutSystems ODC CLI

## Authentication & Setup

```bash
# Check if your tenant credentials work
odc discover

# Save credentials for future use (will prompt for client secret)
odc login https://mytenant.outsystems.dev client-id-here
```

## Listing & Inspecting

### Environments
```bash
# List all available environments
odc list-environments

# Check which environments you have access to
odc list-environments --json
```

### Apps
```bash
# List all apps in the tenant
odc list-apps

# Search for apps by name or key
odc list-apps "CustomerPortal"

# Find all web applications
odc list-apps --type WebApplication

# Find libraries only
odc list-apps --type LowCodeLibrary

# Get paginated results (first 50 apps)
odc list-apps --limit 50 --offset 0

# Export all app details to JSON
odc list-apps --json > all_apps.json
```

### Deployments
```bash
# See which apps are deployed and where
odc list-deployed-apps

# Check deployments for a specific environment
odc list-deployed-apps --env Production

# Find all versions of an app deployed across environments
odc list-deployed-apps "CustomerPortal"

# See deployments in JSON format for automation
odc list-deployed-apps --env Staging --json
```

### App Details
```bash
# Get full metadata for an app
odc get-app "CustomerPortal"

# Retrieve metadata in JSON
odc get-app "CustomerPortal" --json

# Inspect an app using its key instead of name
odc get-app "CustomerPortal_Key"
```

### Revisions
```bash
# Find the latest revision of an app
odc latest-revision --app "CustomerPortal"

# List all revisions of an app
odc list-revisions --app "CustomerPortal"

# Get details on a specific revision
odc get-revision --app "CustomerPortal" --revision 5

# See all revisions in JSON
odc list-revisions "CustomerPortal" --json
```

### Dependencies
```bash
# Visualize what an app depends on (as a Mermaid diagram)
odc producer-graph "CustomerPortal"

# Generate dependency graph for a specific revision
odc producer-graph "CustomerPortal" --revision 3

# Include all producer types (not just deployable apps)
odc producer-graph "CustomerPortal" --all-producers

# Limit graph depth to 2 levels
odc producer-graph "CustomerPortal" --max-depth 2

# Save the dependency graph to a file
odc producer-graph "CustomerPortal" --output deps.mmd
```

## Source Code Management

```bash
# Download the OML source code of the latest revision
odc download-source-code "CustomerPortal"

# Download a specific revision
odc download-source-code "CustomerPortal" --revision 2

# Save to a custom filename
odc download-source-code "CustomerPortal" --output my_app_backup.oml

# Upload a new app or create a new revision
odc upload-source-code my_module.oml

# After uploading, deploy to an environment
odc deploy --app "MyNewApp" --env Development
```

## Deployment Operations

### Single App Deployments
```bash
# Deploy an app to an environment (latest revision)
odc deploy --app "CustomerPortal" --env Production

# Deploy a specific revision
odc deploy --app "CustomerPortal" --env Production --revision 5

# Deploy as Debug build instead of Release
odc deploy --app "CustomerPortal" --env Staging --build-type Debug

# Deploy with a longer timeout (in case it takes time)
odc deploy --app "CustomerPortal" --env Production --timeout 3600

# Start a deployment but don't wait for it to finish
odc deploy --app "CustomerPortal" --env Production --no-wait

# Check deployment status more frequently (poll every 5 seconds)
odc deploy --app "CustomerPortal" --env Production --poll-interval 5
```

### Batch Deployments
```bash
# Deploy multiple apps from a file
odc batch-deploy apps_to_deploy.txt --env Production

# Deploy apps with custom parallelization (process 5 at a time)
odc batch-deploy apps_to_deploy.txt --env Production --max-parallel 5

# Continue deploying even if one app fails
odc batch-deploy apps_to_deploy.txt --env Production --continue-on-error

# Deploy as Debug builds
odc batch-deploy apps_to_deploy.txt --env Staging --build-type Debug

# Deploy without automatically including dependencies
odc batch-deploy apps_to_deploy.txt --env Production --skip-dependencies
```

### Impact Analysis
```bash
# Analyze the impact before deploying an app
odc analyze-deployment --app "CustomerPortal" --env Production

# Analyze a specific revision
odc analyze-deployment --app "CustomerPortal" --env Production --revision 3

# Get analysis result in JSON
odc analyze-deployment --app "CustomerPortal" --env Production --json

# Analyze impact without waiting (get analysis key immediately)
odc analyze-deployment --app "CustomerPortal" --env Production --no-wait

# Analyze the impact of deleting an app (what would break?)
odc analyze-deletion --app "LegacyApp"
```

## Undeploying & Cleanup

```bash
# Remove an app from an environment
odc undeploy --app "CustomerPortal" --env Staging

# Undeploy multiple apps from a file
odc batch-undeploy apps_to_remove.txt --env Staging

# Undeploy with more parallelization
odc batch-undeploy apps_to_remove.txt --env Staging --max-parallel 5

# ⚠️ DANGEROUS: Remove ALL apps from an environment
odc dangerous-batch-undeploy-all --env Staging

# Delete an app entirely from the repository
odc delete-app --app "ObsoleteApp"
```

## User & Role Management

```bash
# Get user information by email
odc get-user "john.doe@company.com"

# Get user information by UUID
odc get-user "550e8400-e29b-41d4-a716-446655440000"

# Update a user's name
odc update-user "john.doe@company.com" --name "John Doe"

# Deactivate a user account
odc update-user "john.doe@company.com" --is-active false

# Update user's profile photo
odc update-user "jane.smith@company.com" --photo-url "https://example.com/jane.jpg"

# Grant an app role to a user
odc grant-role "CustomerPortal" "Manager" "john.doe@company.com"

# Revoke a role from a user
odc revoke-role "CustomerPortal" "Manager" "john.doe@company.com"

# Grant a role to an entire group
odc grant-group-role "CustomerPortal" "Viewer" "SalesTeam"

# List all roles defined in an app
odc list-roles "CustomerPortal"

# List all role assignments for an app
odc list-role-assignments "CustomerPortal"

# List end-user groups
odc list-groups

# Find groups by name
odc list-groups "Sales"

# List members of a specific group
odc list-group-members "SalesTeam"

# Create a new group
odc update-group "NewTeam" --name "NewTeam" --description "The new team"

# Add a user to a group
odc add-user-to-group "john.doe@company.com" "SalesTeam"

# Remove a user from a group
odc remove-user-from-group "john.doe@company.com" "SalesTeam"
```

## Advanced: Internal Build/Publish/Deploy

```bash
# Start a build (returns build key)
odc internal-build --app "CustomerPortal" --env Production --revision 5

# Build with Debug type
odc internal-build --app "CustomerPortal" --env Production --build-type Debug

# Publish a build to an environment
odc internal-publish --app "CustomerPortal" --env Production --revision 5

# Deploy an existing build by its key
odc internal-deploy --app "CustomerPortal" --env Production --build-key "build-key-here"

# Deploy without waiting
odc internal-deploy --app "CustomerPortal" --env Production --build-key "build-key-here" --no-wait
```

## Output & Format Options

```bash
# Always show colors (even if output is piped)
odc list-apps --color always

# Never use colors
odc list-apps --color never

# Get output as JSON for scripts/automation
odc list-deployed-apps --json

# Combine filters and JSON output
odc list-apps "Portal" --type WebApplication --json

# Pretty-print with colors to file
odc list-apps > apps.txt
odc list-apps --color always > apps_colored.txt
```

## Real-World Scenarios

### Pre-deployment checklist
```bash
# 1. List environments available
odc list-environments

# 2. Find the app to deploy
odc list-apps "MyApp"

# 3. Check its latest revision
odc latest-revision --app "MyApp"

# 4. Analyze impact before deploying
odc analyze-deployment --app "MyApp" --env Production --revision 10

# 5. Deploy when ready
odc deploy --app "MyApp" --env Production --revision 10
```

### CI/CD Pipeline
```bash
# In your pipeline, upload a built app
odc upload-source-code ./built_app.oml

# Then deploy it
odc deploy --app "MyApp" --env Staging

# If successful, promote to production
odc deploy --app "MyApp" --env Production --revision 5
```

### Audit & Compliance
```bash
# Export all deployments to JSON for auditing
odc list-deployed-apps --json > deployments.json

# Find all roles assigned to a user
odc list-role-assignments "CustomerPortal" --json | grep -i "john.doe"

# Document all users and their access
odc list-groups --json > groups_snapshot.json
```

### Disaster Recovery
```bash
# Backup all apps' source code
for app in $(odc list-apps --json | jq -r '.[].key'); do
  odc download-source-code "$app" --output "backups/${app}.oml"
done

# Restore from backup
odc upload-source-code backups/MyApp.oml
odc deploy --app "MyApp" --env Production
```

### Release to Multiple Environments
```bash
# Create a file listing apps (apps.txt):
# App1
# App2@5
# App3

# Deploy to staging first
odc batch-deploy apps.txt --env Staging

# After testing, deploy to production
odc batch-deploy apps.txt --env Production
```

## Shell Completion

```bash
# Generate completion for bash
odc completion bash | source

# Generate completion for zsh
odc completion zsh | source

# Install bash completion permanently
odc completion bash > /etc/bash_completion.d/odc

# Generate for other shells
odc completion fish
odc completion powershell
```

## Getting Help

```bash
# Show all commands
odc --help

# Get help for a specific command
odc deploy --help

# Get help for batch operations
odc batch-deploy --help

# Show version
odc --version
```
