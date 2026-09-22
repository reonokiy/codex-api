# Remote clients list

`GET /backend-api/wham/remote/control/environments/{environment_id}/clients` · Alias: `/api/codex/remote/control/environments/{environment_id}/clients`

**Request:** Query cursor?:string,limit?:integer,order?:asc|desc.

**Response:** `{items:[{client_id:string,display_name?:string,device_type?:string,platform?:string,os_version?:string,device_model?:string,app_version?:string,last_seen_at?:RFC3339 string}],cursor?:string}`.

[Pinned source 1](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/app-server-transport/src/transport/remote_control/clients.rs#L258) · [Pinned source 2](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/app-server-transport/src/transport/remote_control/clients.rs#L26) · [Native rules](native.md)
