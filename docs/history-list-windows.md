# History list windows

`POST /backend-api/codex/alpha/history/v2/list_windows` · Alias: `/codex/alpha/history/v2/list_windows`

**Request:** All optional: `limit:integer≥1`, `agent_name:string|null`, `recent_first:boolean`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`.

**Response:** Arbitrary JSON tool output.

The pinned client treats the result as server-owned JSON; no stable output DTO is declared. Truncation/encryption headers follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L86) · [Native rules](native.md)
