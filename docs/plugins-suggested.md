# Plugins suggested

`GET /backend-api/ps/plugins/suggested/codex`

**Request:** Query scope=GLOBAL.

**Headers:** `oai-product-sku`.

**Response:** JSON `{enabled: boolean, plugins: RecommendedPluginItem[]}`. Nested fields: [RecommendedPluginsResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L765).

Each recommendation is `{id:string,name:string,display_name:string}`. Only `scope=GLOBAL` is used by the pinned client.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L1021) · [Native rules](native.md)
