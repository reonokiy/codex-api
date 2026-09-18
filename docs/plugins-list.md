# Plugins list

`GET /backend-api/ps/plugins/list`

**Request:** Query scope,limit,pageToken?,collection?.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugins: RemotePluginDirectoryItem[], pagination: RemotePluginPagination}`. Nested fields: [RemotePluginListResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L759).

Plugin details use the [catalog schema](plugin-read.md); pagination is `{next_page_token?:string}`. Scope values are `GLOBAL`, `USER`, `WORKSPACE`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L2173) · [Native rules](native.md)
