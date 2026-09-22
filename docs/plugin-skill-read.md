# Plugin skill read

`GET /backend-api/ps/plugins/{plugin_id}/skills/{skill_name}`

**Request:** Path plugin_id,skill_name.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugin_id: string, name: string, skill_md_contents?: string}`. Nested fields: [RemotePluginSkillDetailResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L585).

`skill_md_contents` is optional Markdown. This reads an installed/catalog plugin skill; public `/v1/skills` stores versioned project-owned uploads and is a different resource.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote.rs#L2275) · [Native rules](native.md)
