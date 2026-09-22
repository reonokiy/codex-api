# Reset credits list

`GET /backend-api/wham/rate-limit-reset-credits` · Alias: `/api/codex/rate-limit-reset-credits`

**Request:** No body.

**Response:** JSON `{credits: RateLimitResetCreditDetails[], available_count: integer}`. Nested fields: [RateLimitResetCreditsDetails](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/types.rs#L28).

Each credit contains `id`, `reset_type`, `status`, `granted_at` and optional `expires_at`, `title`, `description` (strings).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/rate_limit_resets.rs#L137) · [Native rules](native.md)
