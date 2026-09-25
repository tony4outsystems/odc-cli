# OutSystems ODC CLI vs. OutSystems MCP Server

A detailed comparison between the `odc` CLI tool and the `outsystems-mcp` server for interacting with OutSystems Developer Cloud (ODC).

## Quick Summary

| Aspect | ODC CLI (`odc`) | OutSystems MCP |
|--------|-----------------|----------------|
| **Access** | Direct CLI or via Claude Code skill/agent | Agent-only (via Claude Code MCP integration) |
| **Output Mode** | Compact by default, `--json` for detailed | Always verbose JSON responses |
| **Token Efficiency** | ✅ High (reduced output, built-in polling) | ⚠️ Lower (full JSON always streamed, polling requires repeated calls) |
| **Polling** | ✅ Built-in, deterministic | ⚠️ Agent-driven, non-deterministic |
| **Setup** | Environment variables (optionally via `.env` + mise) | Configured via Claude Code MCP settings |
| **Installation** | Homebrew, binary, or `cargo install` | Via Claude Code MCP integration |
| **Use Cases** | Scripts, CI/CD, direct commands, agents | Agent-based automation only |
| **Cost per Operation** | Lower | Higher |

---

## Access Patterns

### ODC CLI
- **Direct execution**: Run commands from terminal or scripts without Claude
- **Agent integration**: Claude Code can invoke via skill (`/odc-cli-helper` skill in `skills/` directory)
- **Flexibility**: Use directly for one-off commands, or delegate to agent for complex workflows

```bash
# Direct usage
odc deploy --asset MyApp --env Production

# Via Claude Code skill (agent delegates to CLI)
# Ask: "Deploy MyApp to Production with odc"
```

### OutSystems MCP
- **Agent-only**: Must be used through Claude Code or agent interfaces
- **No direct CLI**: Requires Claude/Claude Code UI to invoke
- **Forced delegation**: Cannot run commands directly from terminal

```bash
# Not possible - MCP is agent-only
odc deploy --asset MyApp --env Production  # ❌ Would need to ask Claude to do this

# Must go through Claude:
# Ask: "Deploy MyApp to Production"
# Claude invokes MCP tool → executes via OutSystems
```

---

## Token Efficiency & Output

### ODC CLI
**Compact output by default**
- List commands show only key columns (name, key, type, revision, tag)
- Status messages and progress are concise
- Full records and GUIDs available only with `--json` flag

```bash
$ odc list-assets
┌─────────────────┬────────────────────┬─────────────────┬──────────────┐
│ Name            │ Key                │ Type            │ Revision     │
├─────────────────┼────────────────────┼─────────────────┼──────────────┤
│ MyApp           │ myapp              │ WebApplication  │ 5            │
│ SharedLib       │ sharedlib          │ LowCodeLibrary  │ 2            │
└─────────────────┴────────────────────┴─────────────────┴──────────────┘

# VS with --json (full record available when needed)
$ odc list-assets --json
[{"name":"MyApp","key":"myapp","type":"WebApplication",...,"guid":"..."}]
```

**Impact**: 
- Readable output for human operators
- Agent sees compact text, not flooded with unnecessary data
- When detailed data is needed, agent explicitly requests `--json`
- **Reduces token consumption significantly**

### OutSystems MCP
**Always returns full JSON**
- Every response includes complete record data
- GUIDs, timestamps, full object structures streamed always
- No way to request "summary only" view
- Agent receives every field even if only 2 are relevant

```json
// MCP always returns full structure (abbreviated example)
{
  "name": "MyApp",
  "key": "myapp",
  "type": "WebApplication",
  "revision": 5,
  "guid": "550e8400-e29b-41d4-a716-446655440000",
  "createdBy": {...},
  "lastModified": {...},
  "metadata": {...},
  ...100 more fields...
}
```

**Impact**:
- Higher token consumption per command
- Agent wastes tokens parsing unnecessary fields
- Streaming verbose JSON over MCP protocol adds overhead
- **Increases token usage substantially**

---

## Polling & Async Operations

### ODC CLI
**Built-in polling with deterministic behavior**
- CLI handles all polling internally
- Single command invocation waits for completion
- Progress streamed to stderr while JSON (if requested) goes to stdout
- Agent issues one command, CLI manages the entire operation lifecycle

```bash
$ odc deploy --asset MyApp --env Prod
Starting build for MyApp revision 5...
Build in progress [=========>          ] 45%
Build complete.
Deploying to Prod...
Deployment in progress [===========>      ] 70%
✓ Deployment complete
```

**Token usage**:
- One CLI invocation + one response
- Agent doesn't poll manually
- No repeated API calls from agent perspective
- **Minimal token overhead**

### OutSystems MCP
**Agent-driven polling with non-deterministic behavior**
- MCP returns immediately with operation ID
- Agent must poll repeatedly to check status
- Each poll is a separate MCP call with separate JSON response
- Agent must implement retry logic, timeout handling, backoff

```
Agent: "Deploy MyApp to Prod"
  ↓
MCP: Returns {operationId: "abc123", status: "started"}
  ↓
Agent: Poll operation (token cost #1)
MCP: {operationId: "abc123", status: "building", progress: 0.3}
  ↓
Agent: Poll operation again (token cost #2)
MCP: {operationId: "abc123", status: "building", progress: 0.6}
  ↓
Agent: Poll operation again (token cost #3)
MCP: {operationId: "abc123", status: "complete"}
```

**Token usage**:
- 1 initial call + 3-10+ polling calls
- Each poll includes full JSON response
- Agent reasoning for poll decision logic
- **Significant token overhead per operation**

---

## Setup & Configuration

### ODC CLI

**Credential management**: environment variables, optionally loaded from a project-local `.env` via mise/direnv
```bash
# .env file (loaded into the environment by mise, not read directly by the CLI)
ODC_TENANT_URL=https://tenant.outsystems.dev
ODC_CLIENT_ID=your-client-id
ODC_CLIENT_SECRET=your-client-secret
```

**Installation**:
```bash
# Homebrew (recommended)
brew install tony4outsystems/tap/odc-cli

# Or: Download binary from GitHub Releases
# Or: Build from source
cargo install --path .
```

**No login command needed** — CLI reads `ODC_*` environment variables or `~/.odc/config.json`

### OutSystems MCP

**Setup**: Via Claude Code MCP configuration
- Configure through Claude Code UI settings
- Credentials stored in Claude's MCP config
- Requires MCP server connection to be established

**Installation**:
- Pre-integrated with Claude Code if enabled
- Requires no local install (runs in Claude's environment)

---

## Feature & Command Coverage

### ODC CLI
**Comprehensive command set** (see `odc --help`):
- Authentication: `discover`, `login`
- Portfolio & Asset management: `list-portfolios`, `list-assets`, `get-asset`
- Revisions & Source: `list-revisions`, `get-revision`, `download-source-code`, `upload-source-code`
- Deployment: `deploy`, `batch-deploy`, `undeploy`, `batch-undeploy`, `delete-asset`
- Users & Groups: `get-user`, `update-user`, `list-groups`, `list-group-members`
- Roles: `grant-role`, `revoke-role`, `list-role-assignments`
- Mentor (AI-assisted development): `mentor-start-session`, `mentor-prompt`, `mentor-publish`
- Advanced: `analyze-deployment`, `analyze-deletion`, `producer-graph`, internal single-step operations

**Running with `cargo run`**:
```bash
cargo run -- list-assets
cargo run -- deploy --asset MyApp --env Production
```

### OutSystems MCP
**MCP tools** (via OutSystems MCP server):
- Edit apps
- Publish/deploy
- Search tenant elements
- Manage external libraries
- Basic CRUD operations

**Coverage**: Likely smaller subset focused on common operations

---

## Use Cases & Recommendations

### Use **ODC CLI** When:

✅ **Scripts & Automation**: Batch operations, CI/CD pipelines, scheduled tasks
- Built-in polling makes long-running operations simple
- Exit codes and deterministic behavior suitable for shell scripts
- Cost-effective for repeated operations

✅ **Direct Terminal Access**: One-off commands, exploration, debugging
- No Claude overhead for quick checks
- Full shell integration (piping, redirection, scripting)

✅ **Agent Delegation**: Complex workflows that need human feedback
- Skill-based invocation: `"Deploy MyApp to Production with odc"`
- Agent can use tool output for decision-making
- Lower token cost than repeated MCP polling

✅ **CI/CD & DevOps**: GitHub Actions, GitLab CI, Jenkins, etc.
- Direct binary execution
- No agent overhead
- Works offline if credentials cached

### Use **OutSystems MCP** When:

✅ **Claude Code Agent-Only Interface**: Only access method is Claude
- If you cannot or do not want to install/run CLI locally
- Preference for unified MCP tooling

✅ **Browser-Based Operations**: Working exclusively in Claude web UI
- No terminal access needed
- Fully browser-integrated experience

⚠️ **Not Recommended For**: 
- Frequent deployments (token cost of polling)
- Batch operations (per-operation polling multiplies costs)
- CI/CD pipelines (install CLI instead, much cheaper)

---

## Token Cost Comparison: Example Scenario

**Scenario**: Deploy 3 apps sequentially using polling (10s interval, typical 60s completion)

### With ODC CLI
```bash
odc deploy --asset App1 --env Prod  # Waits 60s, outputs compact summary
odc deploy --asset App2 --env Prod  # Waits 60s, outputs compact summary
odc deploy --asset App3 --env Prod  # Waits 60s, outputs compact summary
```

**Token cost**:
- 3 CLI invocations
- 3 concise responses
- Built-in polling (not tokenized)
- **~300-500 tokens total** (rough estimate for compact output)

### With OutSystems MCP
```
Agent: Deploy App1
  MCP call #1: Start deploy → {operationId, status, full JSON}
  Agent polls (token cost)
  MCP call #2: Poll → {status, full JSON}
  Agent polls (token cost)
  MCP call #3: Poll → {status, full JSON}
Agent: Deploy App2
  MCP call #4-#6: Poll cycle (3 calls)
Agent: Deploy App3
  MCP call #7-#9: Poll cycle (3 calls)
```

**Token cost**:
- 9+ MCP calls
- 9+ verbose JSON responses (all fields)
- Agent reasoning for poll decisions
- **~2000-3000+ tokens total** (rough estimate for full JSON streaming)

**Difference**: **CLI is 4-6x more token-efficient** for polling-heavy operations

---

## Summary Table

| Dimension | ODC CLI | OutSystems MCP |
|-----------|---------|----------------|
| **Access** | Direct + Agent | Agent-only |
| **Output** | Compact default, `--json` optional | Always verbose JSON |
| **Polling** | Built-in, deterministic | Agent-driven, repeated calls |
| **Token Efficiency** | High | Lower |
| **Setup** | Environment variables (`.env` + mise) | MCP configuration |
| **Installation** | Local binary/Homebrew | None (server-hosted) |
| **Best For** | Scripts, CI/CD, agents | Browser-only use cases |
| **Worst For** | (None notable) | CI/CD, high-frequency ops |

---

## Recommendation

**For most workflows, use the ODC CLI**:
- ✅ Lower token cost
- ✅ Better for automation & scripts
- ✅ Works with or without Claude
- ✅ Deterministic polling prevents token waste
- ✅ More flexible (direct + agent access)

**Use OutSystems MCP only if**:
- You cannot install/run the CLI locally
- You prefer unified MCP tooling in Claude
- Your workflows are infrequent and token cost isn't a concern
