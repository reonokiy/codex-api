# Connectors directory

`GET /backend-api/connectors/directory/list`

**Request:** Query token?:string, external_logos=true.

**Response:** `{apps:[DirectoryApp],next_token?:string}; nextToken is also accepted by Codex`.

Each `DirectoryApp` includes `id`, `name`, optional `description`, `app_metadata`, `branding`, `labels`, `logo_url`, `logo_url_dark`, `icon_assets`, `icon_dark_assets`, `distribution_channel`, `visibility`. Codex also accepts camelCase aliases for metadata/logo fields.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/connectors/src/lib.rs#L272) · [Native rules](native.md)
