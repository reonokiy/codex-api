# Api key turn costs

`POST /telemetry/costs`

**Request:** {turn_ids:[string]}

**Headers:** `openai-organization`, `openai-project`.

**Response:** JSON `{turns: ApiKeyTurnCost[]}`. Nested fields: [ApiKeyTurnCostsResponse](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client/turn_usage.rs#L41).

Requires caller-supplied API credentials via `Authorization` and gateway credentials via `X-Codex-Gateway-Authorization`. Original `openai-organization` and `openai-project` headers are preserved. Each cost has `turn_id`, `status`, optional `total_usd`, `event_count`, `responses:[{response_id,total_usd}]`, `model`, `speed`, `reasoning_effort`.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client/turn_usage.rs#L62) · [Native rules](native.md)
