# Plugin share update

`POST /backend-api/public/plugins/workspace/{plugin_id}`

**Request:** JSON `{file_id: string, etag: string, discoverability?: "LISTED" | "UNLISTED" | "PRIVATE", share_targets?: RemotePluginShareTarget[]}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugin_id: string, share_url?: string, can_publish_to_workspace?: boolean}`. Nested fields: [RemoteWorkspacePluginCreateResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/share.rs#L124).

Same request fields as [creation](plugin-share-create.md); the path selects the existing plugin.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/share.rs#L287) · [Native rules](native.md)
