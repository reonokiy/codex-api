# Remote refresh

`POST /backend-api/wham/remote/control/server/refresh` · Alias: `/api/codex/remote/control/server/refresh`

**Request:** JSON `{server_id: string, installation_id: string}`.

**Headers:** `x-codex-installation-id`.

**Response:** `{server_id,environment_id,remote_control_token,expires_at}`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/app-server-transport/src/transport/remote_control/protocol.rs#L227) · [Native rules](native.md)
