# Plugin skill read

`GET /backend-api/ps/plugins/{plugin_id}/skills/{skill_name}`

**Request:** Path plugin_id,skill_name.

**Headers:** `oai-product-sku`.

**Response:** JSON `{plugin_id: string, name: string, skill_md_contents?: string}`. Nested fields: [RemotePluginSkillDetailResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L585).

`skill_md_contents` is optional Markdown. This reads an installed/catalog plugin skill; public `/v1/skills` stores versioned project-owned uploads and is a different resource.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L2275) · [Native rules](native.md)
