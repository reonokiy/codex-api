# Thread usage

`POST /backend-api/wham/usage/thread_usage/query` · Alias: `/api/codex/usage/thread_usage/query`

**Request:** JSON `{thread_ids:string[]}`.

**Response:** JSON `{threads: ThreadUsage[]}`. Nested fields: [ThreadUsageQueryResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/thread_usage.rs#L43).

Each thread has `thread_id`, integer `estimated_usage_credits_micros`, optional integer `estimated_usage_usd_micros`, and `groups`. Each group includes optional `model`, `reasoning_effort`, `speed`, token counts, and integer `estimated_usage_credits_micros`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/thread_usage.rs#L80) · [Native rules](native.md)
