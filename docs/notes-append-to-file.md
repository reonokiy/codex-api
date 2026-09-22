# Notes append to file

`POST /backend-api/codex/alpha/notes/v2/append_to_file` · Alias: `/codex/alpha/notes/v2/append_to_file`

**Request:** Required: `path:string`, `text:string`. Appends UTF-8 text. Every request also includes `context: {session_id:string, current_agent_name:string}`.

**Headers:** `x-openai-tool-output-truncation-policy`, `x-openai-encrypted-tool-arguments`.

**Response:** Arbitrary JSON tool output.

Virtual note paths and consistency follow [native rules](native.md#history-and-notes).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/ext/history-notes/src/tools.rs#L93) · [Native rules](native.md)
