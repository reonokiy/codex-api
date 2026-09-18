# Connectors workspace directory

`GET /backend-api/connectors/directory/list_workspace`

**Request:** Query external_logos=true.

**Response:** `{apps:[DirectoryApp],next_token?:string}; nextToken is also accepted by Codex`.

Each `DirectoryApp` includes `id`, `name`, optional `description`, `app_metadata`, `branding`, `labels`, `logo_url`, `logo_url_dark`, `icon_assets`, `icon_dark_assets`, `distribution_channel`, `visibility`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/connectors/src/lib.rs#L191) · [Native rules](native.md)
