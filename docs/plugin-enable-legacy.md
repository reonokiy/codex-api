# Plugin enable legacy

`POST /backend-api/plugins/{plugin_id}/enable`

**Request:** No body.

**Response:** JSON `{id: string, enabled: boolean, app_ids_needing_auth?: string[]}`. Nested fields: [RemotePluginMutationResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L784).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote_legacy.rs#L194) · [Native rules](native.md)
