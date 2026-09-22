# Plugins suggested

`GET /backend-api/ps/plugins/suggested/codex`

**Request:** Query scope=GLOBAL.

**Headers:** `oai-product-sku`.

**Response:** JSON `{enabled: boolean, plugins: RecommendedPluginItem[]}`. Nested fields: [RecommendedPluginsResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L765).

Each recommendation is `{id:string,name:string,display_name:string}`. Only `scope=GLOBAL` is used by the pinned client.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L1021) · [Native rules](native.md)
