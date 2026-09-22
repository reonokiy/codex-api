# Plugin install

`POST /backend-api/ps/plugins/{plugin_id}/install`

**Request:** `Query includeAppsNeedingAuth=true; optional JSON {install_attempt_id:string}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{id: string, enabled: boolean, app_ids_needing_auth?: string[]}`. Nested fields: [RemotePluginMutationResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L784).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L1594) · [Native rules](native.md)
