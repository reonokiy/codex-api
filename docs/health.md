# Health and version

`GET /healthz` · No authentication, body or query parameters required.

```json
{"status":"ok","codex_release":"0.159.0","codex_revision":"687a119f0fcaace47e1f1abcc77cec6c813fd6da"}
```

HTTP 200 confirms the gateway is running and reports its pinned source version. It does not call the upstream or guarantee current subscription validity, quota or model availability.
