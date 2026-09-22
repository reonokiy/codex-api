# Task sibling turns

`GET /backend-api/wham/tasks/{task_id}/turns/{turn_id}/sibling_turns` · Alias: `/api/codex/tasks/{task_id}/turns/{turn_id}/sibling_turns`

**Request:** Path task_id,turn_id.

**Response:** JSON `{sibling_turns?: object[]}`. Nested fields: [TurnAttemptsSiblingTurnsResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/types.rs#L516).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client.rs#L456) · [Native rules](native.md)
