---
name: actual-swagger-sync
description: Start a local Actual HTTP API Docker container, download `/api-docs/swagger.json`, and save it into `crates/api/actual/swagger.json`. Use when refreshing the Actual API schema in this repository or when the user asks to regenerate/update the local swagger file from image `jhonderson/actual-http-api:26.3.0`.
---

# Actual Swagger Sync

Run `scripts/sync_actual_swagger.ps1` from repository root:

```powershell
powershell -ExecutionPolicy Bypass -File skills/actual-swagger-sync/scripts/sync_actual_swagger.ps1
```

Use optional arguments only when needed:

```powershell
powershell -ExecutionPolicy Bypass -File skills/actual-swagger-sync/scripts/sync_actual_swagger.ps1 `
  -OutputPath crates/api/actual/swagger.json `
  -Port 5007 `
  -Image "jhonderson/actual-http-api:26.3.0"
```

The script:

1. Starts the Docker container with:
   - `ACTUAL_SERVER_URL=localhost`
   - `ACTUAL_SERVER_PASSWORD=pass`
   - `API_KEY=pass`
2. Waits for `http://127.0.0.1:<port>/api-docs/swagger.json`.
3. Writes the response body to the target output path.
4. Stops and removes the temporary container.

If the endpoint is unavailable before timeout, fail with an explicit error and still clean up the container.