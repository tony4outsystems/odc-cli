# Regenerating the README demo

The demo runs the real `odc` CLI against a real tenant (through your `.env`) — it is not a fake/canned recording.

## Prerequisites

- `.env` file configured with `TENANT_URL`, `CLIENT_ID`, `CLIENT_SECRET`
- `odc` CLI built and available in PATH
- `asciinema` installed (for recording)
- `agg` installed (for GIF conversion; installed automatically by the task if needed)

## Regenerating demo.gif

Edit the commands in `cmds.txt` as needed, then run:

```bash
mise run demo
```

This will:
1. Build `odc` in release mode
2. Record the commands from `cmds.txt` using asciinema
3. Convert the recording to `demo.gif` at the repo root

The task is automated via [mise](https://mise.jdx.dev/) and defined in `mise.toml` at the repo root.
