# Plugin share targets

`PUT /backend-api/ps/plugins/{plugin_id}/shares`

**Request:** JSON `{discoverability:string,targets:[{principal_type:string,principal_id:string,role:string}]}`; allowed values are below.

**Headers:** `oai-product-sku`.

**Response:** JSON `{principals: RemotePluginSharePrincipal[], discoverability: "LISTED" | "UNLISTED" | "PRIVATE"}`. Nested fields: [RemotePluginShareUpdateTargetsResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L138).

`discoverability`: `LISTED`, `UNLISTED` or `PRIVATE`. Each target: `{principal_type:"user"|"group"|"workspace",principal_id:string,role:"reader"|"editor"}`. Returned principals also contain `name`; their role may be `owner`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote/share.rs#L320) · [Native rules](native.md)
