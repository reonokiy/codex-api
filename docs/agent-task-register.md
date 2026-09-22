# Agent task register

`POST /auth/api/accounts/v1/agent/{agent_runtime_id}/task/register`

**Request:** JSON `{signature:string,timestamp:string}`; signature is base64 Ed25519 and timestamp is RFC3339.

**Response:** JSON object with optional task_id, taskId, encrypted_task_id, encryptedTaskId string fields.

The request proves possession of the registered Ed25519 key. No account Bearer token is attached upstream.

[Pinned source 1](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/agent-identity/src/lib.rs#L479) · [Pinned source 2](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/agent-identity/src/lib.rs#L139) · [Native rules](native.md)
