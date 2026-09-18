# Plugin uninstall legacy

`POST /backend-api/plugins/{plugin_id}/uninstall`

**Request:** No body.

**Response:** JSON `{id: string, enabled: boolean, app_ids_needing_auth?: string[]}`. Nested fields: [RemotePluginMutationResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L784).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote_legacy.rs#L194) · [Native rules](native.md)
