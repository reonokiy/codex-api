# Workspace messages

`GET /backend-api/wham/workspace-messages` · Alias: `/api/codex/workspace-messages`

**Request:** Cache-Control: no-store.

**Response:** JSON `{messages?: CodexWorkspaceMessage[]}`. Nested fields: [CodexWorkspaceMessagesResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/types.rs#L78).

Each message contains string `message_id`, `message_type` (`headline` or `announcement` in the pin), `message_body`, and optional timestamp strings `created_at`, `archived_at`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L667) · [Native rules](native.md)
