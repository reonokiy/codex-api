# User settings

`GET /backend-api/wham/settings/user` · Alias: `/api/codex/settings/user`

**Request:** Cache-Control: no-cache, no-store.

**Response:** JSON `{commit_attribution_enabled?: boolean}`. Nested fields: [CodexUserSettingsResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/types.rs#L85).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client.rs#L674) · [Native rules](native.md)
