# Native standalone search

`POST /codex/alpha/search` · Alias: `/backend-api/codex/alpha/search`  
[Common rules](common.md)

This is Codex's `web.run` transport, not an OpenAI public REST API. For official SDK-compatible hosted search, use [Responses `web_search`](responses.md#tools).

Call this endpoint independently to get search text and structured results;
no preceding Responses request is required. The gateway applies the shared
[header cleanup](headers.md), retaining session/turn metadata and using its own
Codex client identity upstream.

## Standalone usage

```sh
curl http://127.0.0.1:8080/codex/alpha/search \
  -H "Authorization: Bearer $CODEX_GATEWAY_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"id":"my-search-session","model":"gpt-5.5","commands":{"search_query":[{"q":"OpenAI Codex","domains":["openai.com"]}],"response_length":"short"}}'
```

Structured `text_result` entries can contain `title`, `url`, `snippet`, `domain`
and `ref_id`. To open a returned reference, reuse the same `id` and send
`{"open":[{"ref_id":"turn0search0"}],"response_length":"short"}` as `commands`.
Use a new session ID for an unrelated search. Result shapes depend on the command.

The official Python SDK has no typed resource for this native endpoint, but its
custom-request method works:

```python
import os
import uuid
from openai import OpenAI

with OpenAI(base_url="http://127.0.0.1:8080", api_key=os.environ["CODEX_GATEWAY_API_KEY"]) as client:
    session = str(uuid.uuid4())
    result = client.post("/codex/alpha/search", cast_to=dict, body={
        "id": session,
        "model": "gpt-5.5",
        "commands": {"search_query": [{"q": "OpenAI Codex"}], "response_length": "short"},
    })
    print(result["output"])
    print(result.get("results", []))
```

This is a gateway/native extension; it is not a standard `/v1/search` API.
It uses the gateway's ChatGPT login and request limits.

## JSON request

```json
{"id":"search-session-1","model":"gpt-5.5","commands":{"search_query":[{"q":"OpenAI Codex","domains":["openai.com"]}],"response_length":"short"},"settings":{"external_web_access":true,"allowed_callers":["direct"]}}
```

| Field | Type / behavior |
| --- | --- |
| `id` | Required non-empty session string; reuse for related search commands |
| `model` | Required non-empty string; upstream validates availability |
| `input` | Optional string or array of native Codex input items |
| `commands` | Optional command object below; `input` or `commands` must be present |
| `settings` | Optional settings object below |
| `reasoning` | Optional object: `effort` string, `summary` = `auto`, `concise`, `detailed` or `none`; `context` = `auto`, `current_turn` or `all_turns` |
| `max_output_tokens` | Optional unsigned integer |

Each command field except `response_length` is an array of objects. `?` marks optional fields:

| Command | Item fields |
| --- | --- |
| `search_query`, `image_query` | `q:string`, `recency?:uint`, `domains?:string[]` |
| `open` | `ref_id:string` (URL or previous reference), `lineno?:uint` |
| `click` | `ref_id:string`, `id:uint` |
| `find` | `ref_id:string`, `pattern:string` |
| `screenshot` | `ref_id:string`, `pageno:uint` (zero-based PDF page) |
| `finance` | `ticker:string`, `type:equity\|fund\|crypto\|index`, `market?:string` |
| `weather` | `location:string`, `start?:YYYY-MM-DD`, `duration?:uint` |
| `sports` | `fn:schedule\|standings`, `league:nba\|wnba\|nfl\|nhl\|mlb\|epl\|ncaamb\|ncaawb\|ipl`; optional `tool:"sports"`, `team`, `opponent`, `date_from`, `date_to`, `locale` strings and `num_games:uint` |
| `time` | `utc_offset:string`, e.g. `+02:00` |
| `response_length` | String, not an array: `short\|medium\|long` |

Settings:

| Field | Type |
| --- | --- |
| `external_web_access` | Boolean or `cached\|indexed\|live` |
| `allowed_callers` | Array of `direct\|shell\|code_interpreter` |
| `search_context_size` | `low\|medium\|high` |
| `filters` | `{allowed_domains?:string[], blocked_domains?:string[]}` |
| `user_location` | `{type:"approximate", country?:string, region?:string, city?:string, timezone?:string}` |
| `image_settings` | `{max_results?:uint, caption?:boolean}` |

The original `SearchCommands`/`SearchSettings` types serialize requests. Unknown request/command/settings fields are rejected. `allowed_callers` is upstream search metadata; it does not enable a hosted code interpreter. Command availability and query limits remain upstream-controlled.

## Response

```json
{"output":"Search results with reference IDs...","encrypted_output":"...","results":[]}
```

`output` is text; `encrypted_output` is an optional string and `results` is an optional array of opaque JSON values. Structured result variants and additional fields are preserved without narrowing them to a gateway-specific schema. Related requests can use returned references with the same search session ID. Errors preserve upstream status/body.

For native Codex, use the provider configuration in the README and enable `codex --enable standalone_web_search --search` when this executor is desired. The gateway does not execute a separate local browser.
