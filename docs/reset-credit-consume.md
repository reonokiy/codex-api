# Reset credit consume

`POST /backend-api/wham/rate-limit-reset-credits/consume` · Alias: `/api/codex/rate-limit-reset-credits/consume`

**Request:** JSON `{redeem_request_id:string,credit_id?:string}`.

**Response:** JSON `{code: "reset" | "nothing_to_reset" | "no_credit" | "already_redeemed", windows_reset?: integer}`. Nested fields: [ConsumeRateLimitResetCreditResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/types.rs#L114).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client/rate_limit_resets.rs#L151) · [Native rules](native.md)
