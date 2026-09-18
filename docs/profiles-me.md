# Profiles me

`GET /backend-api/wham/profiles/me` · Alias: `/api/codex/profiles/me`

**Request:** No body.

**Response:** JSON `{stats: TokenUsageProfileStats}`. Nested fields: [TokenUsageProfile](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/types.rs#L522).

`stats` contains optional integer `lifetime_tokens`, `peak_daily_tokens`, `longest_running_turn_sec`, `current_streak_days`, `longest_streak_days`, and `daily_usage_buckets:[{start_date:string,tokens:integer}]`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L361) · [Native rules](native.md)
