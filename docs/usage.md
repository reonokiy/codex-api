# Usage

`GET /backend-api/wham/usage` · Alias: `/api/codex/usage`

**Request:** No body; optional x-openai-codex-luna-reserve:1 request header.

**Headers:** `x-openai-codex-luna-reserve`.

**Response:** RateLimitStatusPayload; original client maps snapshots/reset credits

Each rate-limit window is `{used_percent,limit_window_seconds,reset_after_seconds,reset_at}` (integers). The backend may additionally return `account_id`, `user_id`, `rate_limit_upsell` and `rate_limit_reset_credits:{available_count}`. This is subscription quota; Platform organization usage is a separate API.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/rate_limit_resets.rs#L127) · [Native rules](native.md)
