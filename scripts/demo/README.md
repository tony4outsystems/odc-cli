# Regenerating the README demo

`demo.tape` drives the real `odc` CLI via `odc ...` against a real
tenant (through your `.env`) — it is not a fake/canned recording. Before
running it:

- Run `go install ./cmd/odc` from the repository root and ensure `$(go env GOPATH)/bin` is on your `PATH`.

- Edit the `--app`/`--env` values in `demo.tape` to point at apps and an
  environment you're OK deploying to publicly (the last command really runs
  `deploy`, not a simulation).
- Prefer a sandbox/non-production environment — the recording performs a
  real build and deploy.

To regenerate `demo.gif` at the repo root:

```bash
brew install charmbracelet/tap/vhs
cd /path/to/repo/root
vhs scripts/demo/demo.tape
```

VHS drives a real terminal and screenshots it via headless Chrome, so it
needs a real desktop session — it won't produce output in a fully headless
CI/sandbox environment.
