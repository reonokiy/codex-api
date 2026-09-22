# Agent register

`POST /auth/api/accounts/v1/agent/register`

**Request:** JSON `{abom:{agent_version:string,agent_harness_id:string,running_location:string},agent_public_key:string,capabilities:string[],ttl?:integer}`.

**Response:** `{agent_runtime_id:string}`.

The gateway supplies its subscription authorization. Registration yields an agent runtime ID; signing keys and agent execution remain client-owned.

[Pinned source 1](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/agent-identity/src/lib.rs#L470) · [Pinned source 2](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/agent-identity/src/lib.rs#L139) · [Native rules](native.md)
