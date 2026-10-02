# Compact response history

`POST /v1/responses/compact` · Alias: `/codex/responses/compact`  
[Common rules](common.md)

## Request

Use the [public Responses schema](responses.md): `model` and `input` are required. Optional instructions, reasoning and tool definitions follow the same validation. The response is always JSON, even if `stream:true` is supplied.

```json
{"model":"gpt-5.5","input":[{"role":"user","content":"Summarize this conversation context..."}]}
```

The gateway appends the pinned Codex `compaction_trigger` item and sends remote compaction through Responses, including the model's Responses Lite rules.

## Response

```json
{"id":"resp_...","object":"response.compaction","created_at":1780000000,"output":[{"type":"compaction","encrypted_content":"..."}],"usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}
```

`output` contains the upstream output items and exactly one `compaction` or `compaction_summary` item. Their complete schemas and encrypted content remain upstream-defined. Send the returned compaction items as history on subsequent requests.

An upstream failed/incomplete response is returned unchanged rather than wrapped as successful compaction. This is a compatibility facade, not a stored-response service; there is no GET/DELETE response-history API.
