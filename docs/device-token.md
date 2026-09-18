# Device token

`POST /auth/api/accounts/deviceauth/token`

**Request:** JSON `{device_auth_id:string,user_code:string}`.

**Response:** `{authorization_code:string,code_challenge:string,code_verifier:string}; upstream 403/404 means pending authorization to the pinned polling client`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/login/src/device_code_auth.rs#L107) · [Native rules](native.md)
