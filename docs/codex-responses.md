# Native Codex Responses

`POST /codex/responses` · Alias: `/backend-api/codex/responses`  
[Common rules](common.md) · [WebSocket variant](websocket.md)

## Request

Send the native request built by Codex. For example:

```json
{"model":"gpt-5.5","instructions":"You are a coding assistant.","input":[{"role":"user","content":[{"type":"input_text","text":"Hello"}]}],"tools":[],"tool_choice":"auto","store":false,"stream":true}
```

Native JSON, tool namespaces, future fields and Responses Lite payloads are preserved. The gateway does not apply public catalog validation to `model`; the upstream validates native requests. Body bytes, query parameters and content encoding are forwarded unchanged; upstream validates their format.

Codex session/routing metadata is forwarded, including `session-id`, `thread-id`, `openai-beta` and `x-codex-*`/Responses Lite headers, subject to [header cleanup](headers.md). Authorization, account identity and routing authority are rebuilt using the gateway's subscription.

## Response

Upstream status, headers and body bytes are relayed, including error bodies and SSE when returned. The gateway does not parse native SSE or assemble terminal `response.output`. `/v1/responses` retains its public adapter behavior.

Codex 0.159.0 performs remote compaction through native Responses using a `compaction_trigger` input item. The separate [compact facade](responses-compact.md) is for applications that want a single JSON result.

Local shell, filesystem, custom and MCP tools still execute in the calling Codex. Native [images](images-generations.md) and [standalone search](search.md) use their corresponding gateway routes.
