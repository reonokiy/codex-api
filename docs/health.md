# Health and version

`GET /healthz` · No authentication, body or query parameters required.

```json
{"status":"ok","codex_release":"0.163.0","codex_revision":"83086188ae0614bd882818ff1aa23f3194962012"}
```

HTTP 200 confirms the gateway is running and reports its pinned source version. It does not call the upstream or guarantee current subscription validity, quota or model availability.
