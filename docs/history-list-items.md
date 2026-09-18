# History list items

`POST /backend-api/codex/alpha/history/v2/list_items` · Alias: `/codex/alpha/history/v2/list_items`

**Request:** All optional: `limit:integer≥1`, `recent_first:boolean`, `tool_namespace:string|null`, `role:"user"|"assistant"|"tool"|"system"|"developer"|null`, `agent_name:string|null`, `tool_name:string|null`, `window_id:string|null`, `max_chars_per_item:integer≥1`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`.

**Response:** Arbitrary JSON tool output.

The pinned client treats the result as server-owned JSON; no stable output DTO is declared. Truncation/encryption headers follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/ext/history-notes/src/tools.rs#L87) · [Native rules](native.md)
