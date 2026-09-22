# Notes read file

`POST /backend-api/codex/alpha/notes/v2/read_file` · Alias: `/codex/alpha/notes/v2/read_file`

**Request:** Required: `path:string`. Optional: `start_line:integer|null`, `stop_line:integer|null` (inclusive, one-based; negative indexes count from the end). Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`.

**Response:** Arbitrary JSON tool output.

Virtual note paths and consistency follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L91) · [Native rules](native.md)
