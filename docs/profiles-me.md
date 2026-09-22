# Profiles me

`GET /backend-api/wham/profiles/me` · Alias: `/api/codex/profiles/me`

**Request:** No body.

**Response:** JSON `{stats: TokenUsageProfileStats}`. Nested fields: [TokenUsageProfile](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/types.rs#L522).

`stats` contains optional integer `lifetime_tokens`, `peak_daily_tokens`, `longest_running_turn_sec`, `current_streak_days`, `longest_streak_days`, and `daily_usage_buckets:[{start_date:string,tokens:integer}]`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client.rs#L361) · [Native rules](native.md)
