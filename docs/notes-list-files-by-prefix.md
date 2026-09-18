# Notes list files by prefix

`POST /backend-api/codex/alpha/notes/v2/list_files_by_prefix` · Alias: `/codex/alpha/notes/v2/list_files_by_prefix`

**Request:** All optional: `prefix:string|null`, `max_results:integer≥1`, `file_order_by:"name"|"created_at"|"updated_at"`, `file_order:"ascending"|"descending"`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`.

**Response:** Arbitrary JSON tool output.

Virtual note paths and consistency follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/ext/history-notes/src/tools.rs#L90) · [Native rules](native.md)
