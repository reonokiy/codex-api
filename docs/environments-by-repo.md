# Environments by repo

`GET /backend-api/wham/environments/by-repo/{provider}/{owner}/{repo}` · Alias: `/api/codex/environments/by-repo/{provider}/{owner}/{repo}`

**Request:** Path provider="github", owner:string, repo:string.

**Response:** JSON `[{id:string,label?:string,is_pinned?:boolean,task_count?:integer}]`; additional environment fields are preserved.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/cloud-tasks/src/env_detect.rs#L57) · [Native rules](native.md)
