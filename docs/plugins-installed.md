# Plugins installed

`GET /backend-api/ps/plugins/installed`

**Request:** Query limit? or scope?,pageToken?,includeDownloadUrls?.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugins: RemotePluginInstalledItem[], pagination: RemotePluginPagination}`. Nested fields: [RemotePluginInstalledResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L778).

Each installed item flattens the plugin directory fields into the same object, then adds `installed_at?`, `enabled`, `disabled_skill_names`. There is no nested `plugin` property.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L2215) · [Native rules](native.md)
