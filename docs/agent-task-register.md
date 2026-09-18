# Agent task register

`POST /auth/api/accounts/v1/agent/{agent_runtime_id}/task/register`

**Request:** JSON `{signature:string,timestamp:string}`; signature is base64 Ed25519 and timestamp is RFC3339.

**Response:** JSON object with optional task_id, taskId, encrypted_task_id, encryptedTaskId string fields.

The request proves possession of the registered Ed25519 key. No account Bearer token is attached upstream.

[Pinned source 1](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/agent-identity/src/lib.rs#L479) · [Pinned source 2](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/agent-identity/src/lib.rs#L139) · [Native rules](native.md)
