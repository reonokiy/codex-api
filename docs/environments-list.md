# Environments list

`GET /backend-api/wham/environments` · Alias: `/api/codex/environments`

**Request:** No body.

**Response:** JSON `[{id:string,label?:string,is_pinned?:boolean,task_count?:integer}]`; additional environment fields are preserved.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/cloud-tasks/src/env_detect.rs#L90) · [Native rules](native.md)
