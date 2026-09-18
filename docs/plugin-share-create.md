# Plugin share create

`POST /backend-api/public/plugins/workspace`

**Request:** JSON `{file_id: string, etag: string, discoverability?: "LISTED" | "UNLISTED" | "PRIVATE", share_targets?: RemotePluginShareTarget[]}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugin_id: string, share_url?: string, can_publish_to_workspace?: boolean}`. Nested fields: [RemoteWorkspacePluginCreateResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L124).

`discoverability`: `LISTED`, `UNLISTED` or `PRIVATE`. `share_targets` use `{principal_type:"user"|"group"|"workspace",principal_id:string,role:"reader"|"editor"}`. `etag` comes from the signed blob upload.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L457) · [Native rules](native.md)
