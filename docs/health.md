# Health and version

`GET /healthz` · No authentication, body or query parameters required.

```json
{"status":"ok","codex_release":"0.155.0","codex_revision":"f0a1b8f0849d90960bc406b848f32e5a129b0457"}
```

HTTP 200 confirms the gateway is running and reports its pinned source version. It does not call the upstream or guarantee current subscription validity, quota or model availability.
