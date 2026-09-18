# Pinned Live call sideband

`GET /v1/live/{call_id}` · Aliases: `/codex/live/{call_id}`, `/backend-api/codex/live/{call_id}`  
[Native rules](native.md)

**Request:** WebSocket upgrade; `call_id` is the ID from call creation. Query and original protocol headers are preserved. Use the same upstream identity that created the call: ordinary gateway auth selects the gateway subscription; the separate gateway auth header preserves explicit caller credentials.

**Response:** original full-duplex frameless text/binary messages from Platform `/v1/live/{call_id}`. This uses the pinned Live protocol; current public Live attaches at [the session endpoint](live-sessions-attach.md).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core/src/client.rs#L412)
