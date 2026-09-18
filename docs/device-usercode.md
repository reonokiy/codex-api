# Device usercode

`POST /auth/api/accounts/deviceauth/usercode`

**Request:** JSON `{client_id:string}`.

**Response:** JSON `{device_auth_id:string,user_code:string,interval?:string}`. `usercode` is also accepted by Codex; `interval` is parsed as an integer number of seconds.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/login/src/device_code_auth.rs#L68) · [Native rules](native.md)
