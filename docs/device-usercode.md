# Device usercode

`POST /auth/api/accounts/deviceauth/usercode`

**Request:** JSON `{client_id:string}`.

**Response:** JSON `{device_auth_id:string,user_code:string,interval?:string}`. `usercode` is also accepted by Codex; `interval` is parsed as an integer number of seconds.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/login/src/device_code_auth.rs#L68) · [Native rules](native.md)
