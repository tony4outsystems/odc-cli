- Do not use the `outsystems` MCP server or any `odc` MCP servers for tasks in this repo by default. Use the OutSystems ODC CLI (`odc`) in this project. Run it using `cargo run -- <args>`. Read help from the CLI using arg `-h` to understand how to use it.
- authentication is configured via `.env` file instead of requiring `login` command:
  - Create a `.env` file with: `TENANT_URL`, `CLIENT_ID`, `CLIENT_SECRET`
  - The CLI will automatically read these environment variables
  - No need to run `cargo run -- login` during development
- If needed, read README.md.
- Add commands to the project as needed.

## ODC Concepts: Tenant, Environment, and Roles

Understanding these core concepts is essential for building and using ODC CLI commands:

### Tenant

A **Tenant** is the top-level organizational unit in OutSystems ODC. It represents a single customer's isolated instance.

- **Authentication scope**: All CLI commands authenticate to a single tenant via command login or credentials in `.env`
- **Multi-tenant access**: A user with multiple tenants would need separate CLI sessions or `.env` files for each
- **Operations**: Most operations (listing assets, users, groups, roles) are tenant-wide
- **In CLI commands**: Tenant context is implicit—it comes from `TENANT_URL`, `CLIENT_ID`, `CLIENT_SECRET` in `.env`

### Environment

An **Environment** is a target deployment stage *within* a tenant (e.g., Development, Testing, Production).

- **Hierarchy**: Tenant → Environments → Deployed Assets
- **Scoping**: Some resources like roles are defined per-environment, but role *assignments* are tenant-wide
- **For listing commands**: `--env` is optional — omit to see roles across all environments
  - `list-roles MyApp` — all roles in app
  - `list-roles MyApp --env Production` — filter to Production
  - `list-role-assignments MyApp` — all role assignments
  - `list-role-assignments MyApp --env Production` — filter to Production
- **For assignment commands**: Environment is a required positional argument (after app, before role) to eliminate ambiguity:
  - `grant-role MyApp Production Editor user@example.com`
  - `revoke-role MyApp Production Editor user@example.com`
  - `grant-group-role MyApp Production Editor MyGroup`
  - `revoke-group-role MyApp Production Editor MyGroup`

### Roles

A **Role** is a named set of permissions that is **always attached to both an app and an environment**. There is no such thing as a role that spans all environments — each role object has exactly one `assetKey` (the app) and one `environmentKey` (the environment).

- **Structure**: Tenant → Environment + App → Role (the role is defined at the intersection of an app deployment in an environment)
- **Implication**: The same logical role name (e.g., "Editor") exists as distinct role objects for each environment the app is deployed to. `list-roles MyApp` may return multiple "Editor" roles, one per environment.
- **Assignment**: Roles are assigned to users or groups at the tenant level, but the assignment targets a specific role object (which already encodes its app + environment)
- **Resolution in `grant-role <ASSET> <ENV> <ROLE> <USER>`**:
  - `<ENV>` is **required as a positional argument** — narrows role lookup to a single environment, eliminating ambiguity
  - `<ASSET>` filters roles to a specific app
  - `<ROLE>` is matched by name within that app+environment; if a GUID key, it's validated to belong to that environment
  - Underlying API: `POST /users/{key}/application-roles/{roleKey}` (environment is already encoded in the resolved role key)
- **Listing**: Use `list-roles MyApp` to see all roles across all environments, or `list-roles MyApp --env Production` to filter to one environment

### Users

A **User** exists at the **tenant level** — not per-environment.

- **Scope**: Tenant-wide; the same user exists across all environments
- **Identification**: Users can be identified by email or key (GUID)
- **Role assignment**: Roles are assigned to users tenant-wide. When you grant a role (which is app+environment scoped), the user gets that specific role anywhere it applies
- **API operations**: `POST /users/{key}/application-roles/{roleKey}` — grant a role to a user (no environment parameter; role already encodes it)
- **Commands**: `get-user`, `update-user`, `grant-role`, `revoke-role`

### Groups

A **Group** is a collection of users that is **scoped to an environment** — each group belongs to exactly one environment.

- **Scope**: Environment-specific; the same group name can exist in different environments with different members
- **Structure**: Tenant → Environment → Group
- **Members**: Groups contain users (who are tenant-wide), but the group itself is tied to an environment
- **Role assignment**: Roles are assigned to groups at the group level. When you grant a role to a group, all members get that role in that environment
- **API operations**: `PATCH /groups/{key}/application-roles` — modify roles for a group (environment is implicit in the group's definition)
- **Commands**: `list-groups`, `get-group`, `update-group`, `list-group-members`, `add-user-to-group`, `remove-user-from-group`, `grant-group-role`, `revoke-group-role`
- **Key difference from users**: Groups require `--env` on some commands (e.g., `list-groups --env Production`) because groups are environment-scoped, whereas users don't because they're tenant-wide

### How CLI Commands Interact with These Concepts

| Command | Tenant | Environment | Role |
|---------|--------|-------------|------|
| `list-environments` | ✓ (implicit) | — (lists all) | — |
| `list-assets` | ✓ (implicit) | — (lists all) | — |
| `list-roles X [--env E]` | ✓ (implicit) | optional filter | ✓ (lists roles; filter by env if given) |
| `list-role-assignments X [--env E]` | ✓ (implicit) | optional filter | ✓ (shows role assignments; filter by env if given) |
| `grant-role X E Role User` | ✓ (implicit) | ✓ (required positional) | ✓ (grants role to user in app X, env E) |
| `revoke-role X E Role User` | ✓ (implicit) | ✓ (required positional) | ✓ (revokes role from user in app X, env E) |
| `grant-group-role X E Role Group` | ✓ (implicit) | ✓ (required positional) | ✓ (grants role to group in app X, env E) |
| `revoke-group-role X E Role Group` | ✓ (implicit) | ✓ (required positional) | ✓ (revokes role from group in app X, env E) |
