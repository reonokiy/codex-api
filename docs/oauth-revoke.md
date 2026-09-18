# Oauth revoke

`POST /auth/oauth/revoke`

**Request:** JSON `{token:string,token_type_hint:"refresh_token"|"access_token",client_id?:string}`; `client_id` accompanies refresh-token revocation.

**Response:** Original HTTP status; no result body is required.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/login/src/auth/revoke.rs#L150) · [Native rules](native.md)
