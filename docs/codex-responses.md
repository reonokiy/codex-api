# Native Codex Responses

`POST /codex/responses` · Alias: `/backend-api/codex/responses`  
[Common rules](common.md) · [WebSocket variant](websocket.md)

## Request

Send the native request built by Codex. `model` must be a string and `stream` must be `true`:

```json
{"model":"gpt-5.5","instructions":"You are a coding assistant.","input":[{"role":"user","content":[{"type":"input_text","text":"Hello"}]}],"tools":[],"tool_choice":"auto","store":false,"stream":true}
```

Native JSON, tool namespaces, future fields and Responses Lite payloads are preserved. The gateway does not apply public catalog validation to `model`; the upstream validates native requests. `Content-Encoding` accepts `identity` or `zstd`.

Codex session/routing metadata is forwarded, including `session-id`, `thread-id`, `originator`, `user-agent`, `openai-beta` and relevant `x-codex-*`/Responses Lite headers from the gateway's allowlist. Authorization, account identity and routing authority are rebuilt using the gateway's subscription.

## Response

Always SSE with native event payloads, including unknown event fields. Unlike `/v1/responses`, this endpoint does not assemble or rewrite terminal `response.output`. The original Codex parser validates the stream; interrupted streams never become synthetic success responses.

Codex 0.157.0 performs remote compaction through native Responses using a `compaction_trigger` input item. The separate [compact facade](responses-compact.md) is for applications that want a single JSON result.

Local shell, filesystem, custom and MCP tools still execute in the calling Codex. Native [images](images-generations.md) and [standalone search](search.md) use their corresponding gateway routes.
