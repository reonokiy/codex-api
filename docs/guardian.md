# Guardian

`POST / GET /backend-api/codex/guardian` · Alias: `/codex/guardian`

**Request:** Native [Responses request](codex-responses.md); HTTP POST or WebSocket `response.create`. Request bytes and frames, including native compression and additional fields, are forwarded.

**Response:** SSE or WebSocket Responses events.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-api/src/endpoint/responses.rs#L43) · [Native rules](native.md)
