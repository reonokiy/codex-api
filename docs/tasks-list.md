# Tasks list

`GET /backend-api/wham/tasks/list` · Alias: `/api/codex/tasks/list`

**Request:** Query limit?,task_filter?,environment_id?,cursor?.

**Response:** JSON `{items: TaskListItem[], cursor?: string}`. Nested fields: [PaginatedListTaskListItem](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-backend-openapi-models/src/models/paginated_list_task_list_item_.rs#L16).

Each item includes `id`, `title`, `archived`, `has_unread_turn`, optional `has_generated_title`, numeric `created_at`/`updated_at`, `task_status_display`, and `pull_requests`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client.rs#L401) · [Native rules](native.md)
