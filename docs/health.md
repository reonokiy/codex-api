# Health and version

`GET /healthz` · No authentication, body or query parameters required.

```json
{"status":"ok","codex_release":"0.157.0","codex_revision":"00c972ed5d6ff6499317fd41b7f23605b8e6850d"}
```

HTTP 200 confirms the gateway is running and reports its pinned source version. It does not call the upstream or guarantee current subscription validity, quota or model availability.
