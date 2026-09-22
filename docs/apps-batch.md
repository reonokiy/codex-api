# Apps batch

`POST /backend-api/ps/apps/batch`

**Request:** GetAppsRequest {app_ids:[string],include_tools:boolean}.

**Headers:** `oai-product-sku`.

**Response:** JSON `{apps: BatchApp[]}`. Nested fields: [GetAppsResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/chatgpt/src/connectors.rs#L223).

`BatchApp` fields: `id`, `name`, optional `description`, `icon_url`, `icon_dark_url`, `distribution_channel`, `tools`. Each tool has `name`, `description`, `is_read_only`, optional `title`, `is_enabled`, `disabled_reason`. Additional action metadata is preserved.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/chatgpt/src/connectors.rs#L173) · [Native rules](native.md)
