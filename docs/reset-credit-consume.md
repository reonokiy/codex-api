# Reset credit consume

`POST /backend-api/wham/rate-limit-reset-credits/consume` · Alias: `/api/codex/rate-limit-reset-credits/consume`

**Request:** JSON `{redeem_request_id:string,credit_id?:string}`.

**Response:** JSON `{code: "reset" | "nothing_to_reset" | "no_credit" | "already_redeemed", windows_reset?: integer}`. Nested fields: [ConsumeRateLimitResetCreditResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/types.rs#L114).

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/rate_limit_resets.rs#L151) · [Native rules](native.md)
