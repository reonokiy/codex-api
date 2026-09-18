# Remote pair

`POST /backend-api/wham/remote/control/server/pair` · Alias: `/api/codex/remote/control/server/pair`

**Request:** `{manual_code:boolean}`.

**Response:** `{pairing_code,manual_pairing_code?,server_id,environment_id,expires_at}`.

Use the enrolled server’s `remote_control_token` as upstream Bearer credentials and the separate gateway auth header.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/app-server-transport/src/transport/remote_control/enroll.rs#L66) · [Native rules](native.md)
