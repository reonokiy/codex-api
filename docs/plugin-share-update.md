# Plugin share update

`POST /backend-api/public/plugins/workspace/{plugin_id}`

**Request:** JSON `{file_id: string, etag: string, discoverability?: "LISTED" | "UNLISTED" | "PRIVATE", share_targets?: RemotePluginShareTarget[]}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugin_id: string, share_url?: string, can_publish_to_workspace?: boolean}`. Nested fields: [RemoteWorkspacePluginCreateResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L124).

Same request fields as [creation](plugin-share-create.md); the path selects the existing plugin.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L287) · [Native rules](native.md)
