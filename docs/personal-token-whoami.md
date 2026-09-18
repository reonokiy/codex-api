# Personal token whoami

`GET /auth/api/accounts/v1/user-auth-credential/whoami`

**Request:** No body; Authorization: Bearer <personal-access-token>.

**Response:** `{email?:string,chatgpt_user_id:string,chatgpt_account_id:string,chatgpt_plan_type:string,chatgpt_account_is_fedramp:boolean}`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/login/src/auth/personal_access_token.rs#L13) · [Native rules](native.md)
