# Oauth revoke

`POST /auth/oauth/revoke`

**Request:** JSON `{token:string,token_type_hint:"refresh_token"|"access_token",client_id?:string}`; `client_id` accompanies refresh-token revocation.

**Response:** Original HTTP status; no result body is required.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/login/src/auth/revoke.rs#L150) · [Native rules](native.md)
