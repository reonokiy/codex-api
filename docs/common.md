# Common rules

See [header handling](headers.md) for the shared client-header cleanup and
per-route credential rules.

Send `Authorization: Bearer <CODEX_GATEWAY_API_KEY>` to subscription adapters. They use the gateway’s ChatGPT credentials upstream. Native routes that need independent credentials use `X-Codex-Gateway-Authorization: Bearer <gateway-key>` alongside upstream `Authorization`; see [native routing](native.md). `/healthz` and expiring [transfer URLs](transfers.md) do not require a gateway Bearer header. Authentication is optional only when no gateway key is configured on loopback.

Use `Content-Type: application/json` unless an endpoint documents multipart or WebSocket transport. Use HTTPS/WSS outside a trusted local connection.

| Limit | Default |
| --- | --- |
| Request body / WebSocket message | 16 MiB |
| Decoded native zstd request | 16 MiB |
| Buffered tool response / HTTP Responses event stream | 64 MiB |
| Upstream model catalog | 16 MiB |
| Concurrent generation/native requests or WebSocket connections | 4; `--max-concurrency` |
| Request deadline / streaming idle timeout | 300 seconds; `--timeout-seconds` |

Models and health checks do not occupy generation slots. Images, standalone search, uploads and native routes share slots with Responses. Signed transfers have separate limits. WebSocket slots last for the connection lifetime.

## Startup login

When no credentials and no `auth.json` exist in `CODEX_HOME`, the executable
starts the pinned Codex library's device code flow. The login URL and one-time
code appear in container logs; no browser, shell or inbound callback is needed
in the container. Enable device code login in your ChatGPT security settings
or workspace permissions first; see [OpenAI's authentication guide](https://developers.openai.com/codex/auth).

Login has a 10-minute deadline, including the code request and token exchange.
On timeout or failure the process exits nonzero. Kubernetes Deployment pods
restart it automatically; Docker needs `--restart unless-stopped` or a similar
restart policy. A standalone process exits without restarting itself.

The gateway only listens after login succeeds. In Kubernetes, allow more than
10 minutes with a startup probe (for example, `/healthz` every 10 seconds with
`failureThreshold: 66`) so liveness checks do not interrupt login. Read the
instructions with `kubectl logs -f deployment/codex-api -n codex-api`.

Mount a writable PVC at `/data` and allow UID/GID `65532:65532` to write there.
The default file credential store saves `/data/auth.json`; explicitly configured
Codex credential stores and authentication restrictions remain respected.
Existing credentials are reused, and an existing invalid file is not replaced
automatically. Run one replica with the `Recreate` update strategy.

Token refresh uses Codex's `AuthManager`, including proactive checks when
credentials are requested, authentication-error recovery, and persistence of
refreshed tokens. Revoked or expired refresh tokens require a new login.

## Errors

Gateway errors use this shape; `message` describes the failure:

```json
{"error":{"type":"invalid_request_error","code":"invalid_request_error","message":"...","param":null}}
```

| Status | Meaning |
| --- | --- |
| 400 | Invalid or unsupported request |
| 401 | Missing/wrong gateway key or unavailable subscription credentials |
| 404 | Unknown endpoint |
| 413 | Request size limit exceeded; some JSON extractor failures use 400 |
| 415 | Unsupported media type or native content encoding |
| 429 | All generation slots occupied (`gateway_busy`), or upstream rate limit |
| 502 | Upstream transport, parsing or incomplete-stream failure |
| 504 | Gateway deadline exceeded |

Upstream HTTP failures preserve their status and body, which may be non-JSON, plus headers such as `Retry-After`; transparent native routes preserve all end-to-end headers. Once SSE or WebSocket starts, failures are conveyed as events or connection closure; HTTP 200 alone does not mean generation succeeded.

Public adapters reject unsupported parameters instead of silently pretending to implement them. Native Responses preserves unknown protocol fields; native images/search use the pinned Codex types. Compatibility targets Codex **0.155.1**, not every OpenAI service or future Codex release.
