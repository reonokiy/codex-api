# Notes append to file

`POST /backend-api/codex/alpha/notes/v2/append_to_file` · Alias: `/codex/alpha/notes/v2/append_to_file`

**Request:** Required: `path:string`, `text:string`. Appends UTF-8 text. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`, `x-openai-encrypted-tool-arguments`.

**Response:** Arbitrary JSON tool output.

Virtual note paths and consistency follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/ext/history-notes/src/tools.rs#L93) · [Native rules](native.md)
