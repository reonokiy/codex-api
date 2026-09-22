# Plugins search

`GET /backend-api/ps/plugins/search`

**Request:** Query q,scope?,limit,pageToken?.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugins: RemotePluginDirectoryItem[], pagination: RemotePluginPagination}`. Nested fields: [RemotePluginListResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L759).

Plugin details use the [catalog schema](plugin-read.md); pagination is `{next_page_token?:string}`. Scope values are `GLOBAL`, `USER`, `WORKSPACE`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/search.rs#L44) · [Native rules](native.md)
