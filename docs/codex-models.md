# Native Codex model catalog

`GET /codex/models` · Alias: `/backend-api/codex/models`  
[Common rules](common.md)

| Query parameter | Type / behavior |
| --- | --- |
| `client_version` | Forwarded unchanged when supplied; no gateway default or format validation. |

Example: `/backend-api/codex/models?client_version=0.159.0`.

The gateway forwards the original query using its subscription, preserving upstream status, headers, body and errors. Upstream validates the request. The complete upstream JSON is preserved, including fields outside the pinned model schema:

```json
{"models":[{"slug":"gpt-5.5","display_name":"GPT-5.5","description":"Coding model","supported_reasoning_levels":[{"effort":"medium","description":"Medium reasoning effort"}]}]}
```

The example shows selected fields; the response also includes capability flags, instructions and other model metadata, which vary upstream. `ETag` is included when returned upstream. This is the native catalog format, not the public `{object:"list",data:[...]}` shape.
