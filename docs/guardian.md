# Guardian

`POST / GET /backend-api/codex/guardian` · Alias: `/codex/guardian`

**Request:** Native [Responses request](codex-responses.md); HTTP POST or WebSocket `response.create`. Request bytes and frames, including native compression and additional fields, are forwarded.

**Response:** SSE or WebSocket Responses events.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/endpoint/responses.rs#L43) · [Native rules](native.md)
