# Realtime call creation

`POST /v1/realtime/calls` · Aliases: `/codex/realtime/calls`, `/backend-api/codex/realtime/calls`  
[Native rules](native.md)

**Request:** one of:

- `multipart/form-data`: exactly one `sdp` string and one `session` field containing a JSON object. This matches the public WebRTC call format.
- `application/json`: native `{sdp:string,session:object}`.
- `application/sdp`: raw SDP offer for a sessionless native request.

With ordinary gateway Bearer authentication, multipart fields become the native JSON request and go to `/backend-api/codex/realtime/calls` using the gateway subscription. Other accepted bodies and query parameters are preserved.

With `X-Codex-Gateway-Authorization` plus upstream `Authorization`, the original body and content type are forwarded to `https://api.openai.com/v1/realtime/calls`. Use this mode for public API credentials; there is no subscription conversion of API keys or session resources.

**Response:** upstream SDP answer bytes, status and `Location` containing the call ID. `Location` is not rewritten. Connect a [sideband WebSocket](realtime.md) using that ID. Session options and entitlement are validated upstream.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/codex-api/src/endpoint/realtime_call.rs#L143) · [Official WebRTC guide](https://developers.openai.com/api/docs/guides/voice-webrtc?api=realtime)
