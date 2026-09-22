# Desktop appcast

`GET /backend-api/wham/app/appcast` · Alias: `/api/codex/app/appcast`

**Request:** Query installation_id,arch=x64|arm64,app_version,beta=false,os-version,plan_type=unknown.

**Response:** Appcast XML.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/cli/src/doctor/updates.rs#L43) · [Native rules](native.md)
