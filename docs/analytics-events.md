# Analytics events

`POST /backend-api/codex/analytics-events/events` · Alias: `/codex/analytics-events/events`

**Request:** JSON {events:[{event_type:string,event_params:object}]}; event_params is an event-specific union of Codex analytics DTOs.

**Response:** Upstream success status; no result object is decoded by the client.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/analytics/src/client.rs#L141) · [Native rules](native.md)
