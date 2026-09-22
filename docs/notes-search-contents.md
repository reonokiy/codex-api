# Notes search contents

`POST /backend-api/codex/alpha/notes/v2/search_contents` · Alias: `/codex/alpha/notes/v2/search_contents`

**Request:** Required: `query:string` (case-sensitive literal). Optional: `max_matches_per_file:integer≥1`, `recent_file_first:boolean`, `max_files:integer≥1`, `path_prefix:string|null`. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`, `x-openai-encrypted-tool-arguments`.

**Response:** Arbitrary JSON tool output.

Virtual note paths and consistency follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L92) · [Native rules](native.md)
