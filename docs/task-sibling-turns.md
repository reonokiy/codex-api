# Task sibling turns

`GET /backend-api/wham/tasks/{task_id}/turns/{turn_id}/sibling_turns` · Alias: `/api/codex/tasks/{task_id}/turns/{turn_id}/sibling_turns`

**Request:** Path task_id,turn_id.

**Response:** JSON `{sibling_turns?: object[]}`. Nested fields: [TurnAttemptsSiblingTurnsResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/types.rs#L516).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L456) · [Native rules](native.md)
