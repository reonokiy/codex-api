# Remote pair status

`POST /backend-api/wham/remote/control/server/pair/status` · Alias: `/api/codex/remote/control/server/pair/status`

**Request:** `{pairing_code?:string,manual_pairing_code?:string}; exactly selected code is sent`.

**Response:** `{claimed:boolean}`.

Send exactly one pairing code. Use the enrolled server’s `remote_control_token` and the separate gateway auth header.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/app-server-transport/src/transport/remote_control/enroll.rs#L169) · [Native rules](native.md)
