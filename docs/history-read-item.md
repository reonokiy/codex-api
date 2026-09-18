# History read item

`POST /backend-api/codex/alpha/history/v2/read_item` · Alias: `/codex/alpha/history/v2/read_item`

**Request:** Required: `item_id:string`, `window_id:string`. Optional: `agent_name:string|null`, `offset_chars:integer≥0`, `limit_chars:integer≥1`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`.

**Response:** Arbitrary JSON tool output.

The pinned client treats the result as server-owned JSON; no stable output DTO is declared. Truncation/encryption headers follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/ext/history-notes/src/tools.rs#L88) · [Native rules](native.md)
