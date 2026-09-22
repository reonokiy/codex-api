# Task read

`GET /backend-api/wham/tasks/{task_id}` · Alias: `/api/codex/tasks/{task_id}`

**Request:** Path task_id.

**Response:** JSON `{current_user_turn?: Turn, current_assistant_turn?: Turn, current_diff_task_turn?: Turn}`. Nested fields: [CodeTaskDetailsResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/types.rs#L220).

Each turn contains optional `id`, `attempt_placement`, `turn_status`, `worklog`, `error`, plus arrays `sibling_turn_ids`, `input_items`, `output_items`. Items include `type`, optional `role`, `content`, `diff` and `output_diff`; the upstream may add fields.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client.rs#L437) · [Native rules](native.md)
