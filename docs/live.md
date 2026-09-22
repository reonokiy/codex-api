# Pinned Live transport

`POST /v1/live` · `GET /v1/live`  
Aliases: `/codex/live`, `/backend-api/codex/live` · [Native rules](native.md)

**POST request:** multipart `sdp` and JSON `session` fields, native JSON `{sdp,session}`, or raw SDP, as in [Realtime calls](realtime-calls.md). Subscription mode sends the native call to `/backend-api/codex/realtime/calls`; if no query is supplied it adds `intent=quicksilver&architecture=avas`. Explicit upstream-credential mode forwards the original request to `/v1/live` on Platform.

**POST response:** upstream SDP answer and `Location` call ID.

**GET request/response:** WebSocket upgrade, followed by original frameless Live messages relayed in both directions to Platform `/v1/live`. Existing-call sidebands use [the call endpoint](live-call.md).

This is the transport used by the pinned source. The currently documented public Live API uses [session routes](live-sessions.md) and a different JSON negotiation format.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-api/src/endpoint/realtime_call.rs#L143)
