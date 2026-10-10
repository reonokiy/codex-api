# Create a response

`POST /v1/responses` · [Common rules](common.md) · [WebSocket variant](websocket.md)

## Codex CLI compatibility

Codex CLI can use the same `/v1` base URL as OpenAI SDK clients. Streaming requests containing `client_metadata`, and requests encoded with `Content-Encoding: zstd`, use the [native Responses contract](codex-responses.md): original payloads (including tools, Lite items and future fields) and SSE events are preserved. Native requests require `model` and `stream: true`; upstream validates their fields. Other requests use the public schema below.

## Request

```json
{"model":"gpt-5.5","input":"Hello","stream":false}
```

| Field | Type / behavior |
| --- | --- |
| `model` | Required string from [`GET /v1/models`](models.md) |
| `input` | Required string or non-empty array of input items |
| `instructions` | Optional string; defaults to the model's Codex instructions |
| `stream` | Boolean, default `false`; upstream always streams |
| `store` | Only `false` supported |
| `tools` | Array, default `[]`; see below |
| `tool_choice` | Only `"auto"` supported |
| `parallel_tool_calls` | Boolean, default `true`; disabled for Responses Lite |
| `reasoning` | Optional object; `effort` is a model-supported string, `summary` is `auto`, `concise`, `detailed` or `none`; `context` is `auto`, `current_turn` or `all_turns` |
| `text` | `{ "verbosity"?: "low"\|"medium"\|"high", "format"?: object }` |
| `text.format` | `{"type":"text"}` or `{ "type":"json_schema", "name":string, "schema":object, "strict"?:boolean }`; schema strict defaults to `true` |
| `service_tier` | `"auto"` or a tier supported by the model |
| `prompt_cache_key` | Optional string; defaults to the request session ID |
| `safety_identifier` | Optional string (at most 64 characters) or `null`; accepted for client compatibility and discarded locally, never forwarded to Codex |
| `max_output_tokens` | Positive unsigned 64-bit integer or `null`; accepted as a compatibility hint for clients such as Magpie, but not forwarded or enforced because Codex has no output-token limit parameter |
| `include` | Array of `"reasoning.encrypted_content"`, `"web_search_call.action.sources"`, `"web_search_call.results"`; encrypted reasoning is always requested |

Known reasoning efforts are `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max`, `ultra` and `persistent`; only values advertised by the selected model in the startup catalog are accepted, including model-defined values. Omitted reasoning and verbosity use that model's defaults.

`reasoning.generate_summary` is accepted as a deprecated alias of `summary`. Explicit `reasoning.context` values are preserved; `auto` and `current_turn` select regular Responses because Lite requires `all_turns`. Omitted/null context retains the Lite default of `all_turns` when the request uses Lite (and is omitted for regular requests). Requests containing hosted `web_search` always use regular Responses, where the hosted tool can execute. See the [SDK compatibility suite and parameter audit](sdk-compatibility.md).

Message inputs accept `role` (`user`, `assistant`, `system`, `developer`) and string or Codex content arrays. Public `system` messages are forwarded as `developer` messages because Codex rejects the `system` input role. Their content and position are preserved, and explicit top-level `instructions` retain their content and are prepended as a classified developer message, matching Codex 0.162.1 (for both standard and lite requests). Text and image content, reasoning, function/custom calls and their outputs, web-search calls and Codex compaction items are supported. Functions/custom tools run in the caller, which submits their outputs in the next request.

HTTP continuation requires complete input history. `previous_response_id`, `background`, temperature/sampling controls and other unlisted top-level fields are not implemented here. `max_output_tokens` does not cap generated output; clients requiring a hard limit must not rely on it.

## Tools

- `function`: `name`, `parameters` object; optional `description` and `strict` (default `false`).
- `custom`: `name`, optional `description`, `format: {"type":"grammar","syntax":"lark"|"regex","definition":string}`.
- `namespace`: `name`, optional `description`, non-empty `tools` containing function/custom definitions; no nested namespaces.
- `web_search`: optional `external_web_access`, `indexed_web_access` booleans, `filters: {"allowed_domains":[string]}`, `user_location: {"type":"approximate","country"?:string,"region"?:string,"city"?:string,"timezone"?:string}`, `search_context_size: "low"|"medium"|"high"`, `search_content_types: ["text"|"image"]`.

Web search runs upstream, including opening pages and finding text. Search is optional with `tool_choice:"auto"`. `web_search_preview`, `image_generation`, hosted `file_search`, `code_interpreter`, `mcp` and newer search parameters are unsupported. Use the [Images API](images-generations.md) for image generation.

## Response

Non-streaming requests return the full terminal Responses object. Minimal example:

```json
{"id":"resp_...","object":"response","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello","annotations":[]}]}],"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15}}
```

Search responses may contain `web_search_call` items, `action.sources`, `results`, and `url_citation` annotations. Other upstream fields are preserved. Check `status`, `error` and `incomplete_details`; failed/incomplete terminal responses remain failed/incomplete.

With `stream:true`, the content type is `text/event-stream`:

```text
event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_example","output_index":0,"content_index":0,"delta":"Hello","sequence_number":1}

event: response.completed
data: {"type":"response.completed","sequence_number":2,"response":{"id":"resp_example","object":"response","status":"completed","output":[{"type":"message","id":"msg_example","role":"assistant","content":[{"type":"output_text","text":"Hello","annotations":[]}]}]}}
```

Unknown events survive. If Codex sends output items separately but leaves terminal `output` empty, the public adapter assembles them. It does not invent successful completion for a truncated stream.

```python
response = client.responses.create(
    model="gpt-5.5", input="Find the official Codex page and cite it.",
    tools=[{"type": "web_search", "external_web_access": True}],
    include=["web_search_call.action.sources"],
)
print(response.output_text)
```
