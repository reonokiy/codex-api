# Signed file and plugin transfers

`GET /transfers/{handle}` · `PUT /transfers/{handle}`  
[Native rules](native.md)

Set `CODEX_GATEWAY_PUBLIC_URL=https://gateway.example.com` to replace native signed file/plugin upload URLs and plugin catalog `release.bundle_download_url` values with gateway URLs. Without it, the original signed URLs are returned. Native finalized file `download_url` fields are currently unchanged.

**Request:** use the returned URL and its assigned method. `PUT` carries raw file/gzip bytes and original storage content headers, including `x-ms-blob-type: BlockBlob`; `GET` has no body. No gateway Bearer header is required: the opaque URL authorizes that one transfer method.

**Response:** original storage status, headers and streamed bytes. Keep the reservation response's `etag` for plugin publication. Signed query parameters are retained upstream; account and gateway credentials are removed from storage requests.

Handles expire after 15 minutes, are stored only in memory, and are capped at 1,024 per process. Transfers allow 8 concurrent streams and 512 MiB per body. A missing/expired handle returns 404, a wrong method 405, and exhausted transfer capacity 503. In Kubernetes, use one replica or route each handle back to the process that issued it; restarts invalidate existing URLs.

[Pinned upload implementation](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/files.rs#L186)
