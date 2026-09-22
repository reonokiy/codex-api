# Remote enroll

`POST /backend-api/wham/remote/control/server/enroll` · Alias: `/api/codex/remote/control/server/enroll`

**Request:** `{name,os,arch,app_server_version,installation_id}`.

**Headers:** `x-codex-installation-id`.

**Response:** `{server_id,environment_id,remote_control_token,expires_at}`.

`expires_at` is a timestamp string. The pinned client accepts ChatGPT domains and localhost for remote-control URL overrides; an arbitrary Kubernetes service hostname requires an explicit client routing change.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/app-server-transport/src/transport/remote_control/protocol.rs#L224) · [Native rules](native.md)
