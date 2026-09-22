# Plugin share create

`POST /backend-api/public/plugins/workspace`

**Request:** JSON `{file_id: string, etag: string, discoverability?: "LISTED" | "UNLISTED" | "PRIVATE", share_targets?: RemotePluginShareTarget[]}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugin_id: string, share_url?: string, can_publish_to_workspace?: boolean}`. Nested fields: [RemoteWorkspacePluginCreateResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/share.rs#L124).

`discoverability`: `LISTED`, `UNLISTED` or `PRIVATE`. `share_targets` use `{principal_type:"user"|"group"|"workspace",principal_id:string,role:"reader"|"editor"}`. `etag` comes from the signed blob upload.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/share.rs#L457) · [Native rules](native.md)
