# User settings

`GET /backend-api/wham/settings/user` · Alias: `/api/codex/settings/user`

**Request:** Cache-Control: no-cache, no-store.

**Response:** JSON `{commit_attribution_enabled?: boolean}`. Nested fields: [CodexUserSettingsResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/types.rs#L85).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L674) · [Native rules](native.md)
