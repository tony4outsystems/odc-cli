# Example Prompts for Asking Claude to Use the ODC CLI

These are natural language prompts users can ask Claude to execute ODC CLI commands.

## Authentication & Setup

- "Check if my ODC tenant credentials are valid"
- "Help me authenticate with my ODC tenant"
- "Verify the tenant URL and OAuth endpoints"

## Listing & Inspecting

### Environments
- "What environments do I have in my ODC tenant?"
- "List all available environments"

### Apps
- "Show me all apps in the tenant"
- "Find the CustomerPortal app"
- "List all web applications"
- "Show me only library apps"
- "How many apps are deployed?"
- "Give me a list of all apps in JSON format"

### Deployments
- "Where is the CustomerPortal app deployed?"
- "Show me all apps currently deployed to Production"
- "What's deployed in the Staging environment?"
- "Is the EmployeeApp deployed anywhere?"

### App Details
- "Get me the full details of the CustomerPortal app"
- "Show me metadata for the EmployeeApp"
- "What does the InventorySystem app contain?"

### Revisions
- "What's the latest revision of CustomerPortal?"
- "Show me all revisions of the EmployeeApp"
- "Tell me about revision 5 of the CustomerPortal"
- "How many revisions does the dashboard app have?"

### Dependencies
- "Show me what CustomerPortal depends on"
- "Visualize the dependencies of the EmployeeApp"
- "What are all the producers for the CoreLibrary?"
- "Create a dependency diagram for UserManagement"
- "How deep are the dependencies for this app?"

## Source Code Management

- "Download the source code for the EmployeeApp"
- "Give me the OML for revision 3 of CustomerPortal"
- "Back up the source code for all apps"
- "Upload this new app version to ODC"
- "Download the latest version of every app"

## Deployment Operations

### Single App Deployments
- "Deploy the CustomerPortal app to Production"
- "Deploy EmployeeApp to Staging"
- "Can you deploy revision 5 of CustomerPortal to Production?"
- "Deploy with a debug build instead of release"
- "Deploy the app but don't wait for it to finish"
- "Deploy and give me updates every 5 seconds"

### Before Deploying
- "What would happen if I deploy CustomerPortal to Production?"
- "Analyze the impact of deploying the EmployeeApp"
- "Check if there are any issues before deploying"
- "What would break if I deploy revision 10?"

### Batch Deployments
- "Deploy all these apps to Production: [App1, App2, App3]"
- "Deploy multiple apps from a file"
- "Roll out 5 apps to Staging simultaneously"
- "Deploy these apps even if one fails"
- "Deploy to Production without their dependencies"

### Undeploying & Cleanup
- "Remove the CustomerPortal from Staging"
- "Undeploy the EmployeeApp from Production"
- "Remove all apps from the Staging environment"
- "Delete the LegacyApp completely"

## User & Role Management

### User Information
- "Get the details for john.doe@company.com"
- "Find the user with UUID 550e8400-e29b-41d4-a716-446655440000"
- "Show me who john.smith@company.com is"

### User Updates
- "Update Jane Doe's profile name"
- "Deactivate the john.doe@company.com account"
- "Change the profile photo for jane.smith@company.com"

### Role Management
- "Grant the Manager role to john.doe@company.com in CustomerPortal"
- "Give jane.smith@company.com admin access to the EmployeeApp"
- "Remove john.doe's Manager role from CustomerPortal"
- "Revoke the Viewer role from jane.smith@company.com"

### Groups & Permissions
- "What are all the user groups?"
- "Find the Sales team group"
- "Who is in the SalesTeam?"
- "What roles are defined for the CustomerPortal?"
- "Who has access to the EmployeeApp and what roles?"
- "Add john.doe@company.com to the SalesTeam"
- "Remove jane.smith from the Engineering group"
- "Grant the SalesTeam the Viewer role for the CustomerPortal"

## Real-World Use Cases

### Pre-Deployment Checklist
- "Walk me through deploying CustomerPortal to Production safely"
- "I want to deploy an app. Check availability and analyze impact first"
- "Verify that all dependencies are available before deploying"

### CI/CD & Automation
- "Upload this built app and deploy it to Development"
- "Build and deploy the latest version to Staging"
- "Promote the current version from Staging to Production"

### Disaster Recovery
- "Backup all my apps' source code"
- "Create a complete snapshot of what's deployed"
- "Restore this app from backup"

### Audit & Compliance
- "Give me a complete list of all deployments"
- "Who has access to the CustomerPortal and what can they do?"
- "Show me all user group memberships"
- "Export a list of all apps and their deployments"

### Multi-Environment Release
- "Deploy these 5 apps to Staging first, then Production after review"
- "Stage version 5 of the dashboard in Testing and keep version 4 in Production"

## General Tasks

- "What's the latest version of [app name]?"
- "Where is [app name] currently deployed?"
- "Show me all web applications"
- "Is the app ready to deploy?"
- "What's the ODC CLI version?"
- "Help me install the ODC CLI"
