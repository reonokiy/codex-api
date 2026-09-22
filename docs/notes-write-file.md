# Notes write file

`POST /backend-api/codex/alpha/notes/v2/write_file` · Alias: `/codex/alpha/notes/v2/write_file`

**Request:** Required: `path:string`, `text:string`. Creates or replaces the virtual note. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`, `x-openai-encrypted-tool-arguments`.

**Response:** Arbitrary JSON tool output.

Virtual note paths and consistency follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L94) · [Native rules](native.md)
