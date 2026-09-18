# Remote pair status

`POST /backend-api/wham/remote/control/server/pair/status` · Alias: `/api/codex/remote/control/server/pair/status`

**Request:** `{pairing_code?:string,manual_pairing_code?:string}; exactly selected code is sent`.

**Response:** `{claimed:boolean}`.

Send exactly one pairing code. Use the enrolled server’s `remote_control_token` and the separate gateway auth header.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/app-server-transport/src/transport/remote_control/enroll.rs#L169) · [Native rules](native.md)
