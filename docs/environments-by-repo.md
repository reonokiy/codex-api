# Environments by repo

`GET /backend-api/wham/environments/by-repo/{provider}/{owner}/{repo}` · Alias: `/api/codex/environments/by-repo/{provider}/{owner}/{repo}`

**Request:** Path provider="github", owner:string, repo:string.

**Response:** JSON `[{id:string,label?:string,is_pinned?:boolean,task_count?:integer}]`; additional environment fields are preserved.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/cloud-tasks/src/env_detect.rs#L57) · [Native rules](native.md)
