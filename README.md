# ODC API Sandbox

Small Python client for testing the ODC APIs described in `api-specs/`.

The default `.env` values target the provided tenant, asset key, and environment key.

```bash
uv run odc-api-sandbox discover
uv run odc-api-sandbox latest-revision
uv run odc-api-sandbox run-all
```

The `run-all` command resolves the latest revision, starts a Release build, waits for it to finish, publishes the revision, then deploys it to the configured environment.

Individual operations are also available:

```bash
uv run odc-api-sandbox build --revision 1
uv run odc-api-sandbox publish --revision 1
uv run odc-api-sandbox deploy --revision 1 --build-key <build-key>
```

Useful overrides:

```bash
uv run odc-api-sandbox run-all --asset-key <asset-key> --environment-key <environment-key> --revision <revision>
```
