# Desktop appcast

`GET /backend-api/wham/app/appcast` · Alias: `/api/codex/app/appcast`

**Request:** Query installation_id,arch=x64|arm64,app_version,beta=false,os-version,plan_type=unknown.

**Response:** Appcast XML.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/cli/src/doctor/updates.rs#L43) · [Native rules](native.md)
