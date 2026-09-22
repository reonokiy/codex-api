# History read item

`POST /backend-api/codex/alpha/history/v2/read_item` · Alias: `/codex/alpha/history/v2/read_item`

**Request:** Required: `item_id:string`, `window_id:string`. Optional: `agent_name:string|null`, `offset_chars:integer≥0`, `limit_chars:integer≥1`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`.

**Response:** Arbitrary JSON tool output.

The pinned client treats the result as server-owned JSON; no stable output DTO is declared. Truncation/encryption headers follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L88) · [Native rules](native.md)
