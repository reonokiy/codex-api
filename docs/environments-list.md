# Environments list

`GET /backend-api/wham/environments` · Alias: `/api/codex/environments`

**Request:** No body.

**Response:** JSON `[{id:string,label?:string,is_pinned?:boolean,task_count?:integer}]`; additional environment fields are preserved.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/cloud-tasks/src/env_detect.rs#L90) · [Native rules](native.md)
