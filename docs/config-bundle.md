# Config bundle

`GET /backend-api/wham/config/bundle` · Alias: `/api/codex/config/bundle`

**Request:** No body.

**Response:** JSON `{config_toml?: DeliveredConfigToml, requirements_toml?: DeliveredRequirementsToml}`. Nested fields: [ConfigBundleResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-backend-openapi-models/src/models/config_bundle_response.rs#L16).

Each delivered object may contain `enterprise_managed:Fragment[]` and `managed_layers:{baseline:Fragment[],system_overlay:Fragment[]}`. A fragment is `{id:string,name:string,contents:string}`; `contents` contains TOML.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L474) · [Native rules](native.md)
