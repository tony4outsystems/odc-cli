# Regenerating the README demo

The demo runs the real `odc` CLI against a real tenant (through environment variables loaded from `.env` by mise) — it is not a fake/canned recording.

## Prerequisites

- `.env` file configured with `ODC_TENANT_URL`, `ODC_CLIENT_ID`, `ODC_CLIENT_SECRET` (loaded into the environment by mise; the CLI itself does not read `.env`)
- `odc` CLI built and available in PATH
- `vhs` (VHS CLI) installed; the task installs it automatically via `brew install charmbracelet/tap/vhs` if needed

## Regenerating demo.gif

Edit the commands in `scripts/demo/demo.tape` as needed, then run:

```bash
mise run demo
```

This will:
1. Build `odc` in release mode
2. Play the VHS tape to record and generate `demo.gif`
3. Output GIF to repo root

The task is automated via [mise](https://mise.jdx.dev/) and defined in `mise.toml` at the repo root.

VHS uses a declarative `.tape` format for terminal recordings. See [charmbracelet/vhs](https://github.com/charmbracelet/vhs) for syntax documentation.
