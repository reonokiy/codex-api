# Plugin upload url

`POST /backend-api/public/plugins/workspace/upload-url`

**Request:** `{filename,mime_type:"application/gzip",size_bytes,plugin_id?}`.

**Headers:** `oai-product-sku`.

**Response:** JSON `{file_id: string, upload_url: string, etag?: string}`. Nested fields: [RemoteWorkspacePluginUploadUrlResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/share.rs#L107).

Upload gzip bytes using the returned `upload_url`; preserve the returned upload `ETag` for [creation](plugin-share-create.md) or [update](plugin-share-update.md). Upload URLs may be rewritten to [transfers](transfers.md).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote/share.rs#L405) · [Native rules](native.md)
