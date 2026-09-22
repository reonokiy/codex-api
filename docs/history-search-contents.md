# History search contents

`POST /backend-api/codex/alpha/history/v2/search_contents` · Alias: `/codex/alpha/history/v2/search_contents`

**Request:** Required: `query:string` (case-sensitive literal). Optional: `limit:integer≥1`, `recent_first:boolean`, `tool_namespace:string|null`, `role:"user"|"assistant"|"tool"|"system"|"developer"|null`, `agent_name:string|null`, `tool_name:string|null`, `window_id:string|null`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`, `x-openai-encrypted-tool-arguments`.

**Response:** Arbitrary JSON tool output.

The pinned client treats the result as server-owned JSON; no stable output DTO is declared. Truncation/encryption headers follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L89) · [Native rules](native.md)
