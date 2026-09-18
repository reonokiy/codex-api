# Analytics events

`POST /backend-api/codex/analytics-events/events` · Alias: `/codex/analytics-events/events`

**Request:** JSON {events:[{event_type:string,event_params:object}]}; event_params is an event-specific union of Codex analytics DTOs.

**Response:** Upstream success status; no result object is decoded by the client.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/analytics/src/client.rs#L141) · [Native rules](native.md)
