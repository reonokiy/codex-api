# Thread usage

`POST /backend-api/wham/usage/thread_usage/query` · Alias: `/api/codex/usage/thread_usage/query`

**Request:** JSON `{thread_ids:string[]}`.

**Response:** JSON `{threads: ThreadUsage[]}`. Nested fields: [ThreadUsageQueryResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client/thread_usage.rs#L43).

Each thread has `thread_id`, integer `estimated_usage_credits_micros`, optional integer `estimated_usage_usd_micros`, and `groups`. Each group includes optional `model`, `reasoning_effort`, `speed`, token counts, and integer `estimated_usage_credits_micros`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client/thread_usage.rs#L80) · [Native rules](native.md)
