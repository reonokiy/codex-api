# Plugin upload url

`POST /backend-api/public/plugins/workspace/upload-url`

**Request:** `{filename,mime_type:"application/gzip",size_bytes,plugin_id?}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{file_id: string, upload_url: string, etag?: string}`. Nested fields: [RemoteWorkspacePluginUploadUrlResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L107).

Upload gzip bytes using the returned `upload_url`; preserve the returned upload `ETag` for [creation](plugin-share-create.md) or [update](plugin-share-update.md). Upload URLs may be rewritten to [transfers](transfers.md).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L405) · [Native rules](native.md)
