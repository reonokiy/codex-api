# OAuth token exchange

`POST /auth/oauth/token`  
[Native rules](native.md)

The gateway preserves the original grant and content type. The pinned clients use these formats:

| Grant | Request fields | Response fields |
| --- | --- | --- |
| `authorization_code` (form) | `client_id`, `code`, `redirect_uri`, `code_verifier` | `id_token`, `access_token`, `refresh_token` |
| `refresh_token` (JSON) | `client_id`, `refresh_token` | Optional `id_token`, `access_token`, `refresh_token` |
| `urn:ietf:params:oauth:grant-type:token-exchange` (form) | `client_id`, `requested_token=openai-api-key`, `subject_token`, `subject_token_type=urn:ietf:params:oauth:token-type:id_token` | `access_token` |
| `urn:ietf:params:oauth:grant-type:jwt-bearer` (form) | `assertion`, `federation_rule_id`, optional `workload_identity_context` | `access_token`, `chatgpt_account_id`, `chatgpt_account_user_id`, `chatgpt_plan_type?`, `expires_in`, `issued_token_type`, `scope`, `token_type`, `user_id` |

Each request also includes its `grant_type`. Form means `application/x-www-form-urlencoded`; token fields are strings and `expires_in` is an integer number of seconds. Additional upstream fields and errors remain intact. This relay does not persist returned tokens into the gateway credential store.

[Pinned browser exchange](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/login/src/server.rs#L827) · [Refresh](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/login/src/auth/manager.rs#L1599) · [Workload identity](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/workload-identity/src/exchange.rs#L172)
