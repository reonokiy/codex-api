# Thread estimates

`POST /backend-api/wham/usage/thread-estimates/query` · Alias: `/api/codex/usage/thread-estimates/query`

**Request:** JSON `{threads:[{thread_id:string,turn_ids:string[]}],include_settled_response_ids:boolean}`; the pinned client sends `true`.

**Response:** JSON `{threads: ChatgptThreadTurnCosts[]}`. Nested fields: [TurnCostsResponse](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/chatgpt_turn_cost.rs#L40).

Each thread is `{thread_id:string,turns:[{turn_id:string,model?:string,estimated_usage_usd_micros?:integer,settled_response_ids?:string[]}]}`.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/backend-client/src/client/chatgpt_turn_cost.rs#L55) · [Native rules](native.md)
