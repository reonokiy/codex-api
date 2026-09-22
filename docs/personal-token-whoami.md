# Personal token whoami

`GET /auth/api/accounts/v1/user-auth-credential/whoami`

**Request:** No body; Authorization: Bearer <personal-access-token>.

**Response:** `{email?:string,chatgpt_user_id:string,chatgpt_account_id:string,chatgpt_plan_type:string,chatgpt_account_is_fedramp:boolean}`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/login/src/auth/personal_access_token.rs#L13) · [Native rules](native.md)
