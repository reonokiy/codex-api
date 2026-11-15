# Health and version

`GET /healthz` · No authentication, body or query parameters required.

```json
{"status":"ok","codex_release":"0.169.0","codex_revision":"d32063f96c2dff1dc96af839b3952abb6acfeda4"}
```

HTTP 200 confirms the gateway is running and reports its pinned source version. It does not call the upstream or guarantee current subscription validity, quota or model availability.
