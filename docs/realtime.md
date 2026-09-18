# Realtime WebSocket

`GET /v1/realtime` · Aliases: `/codex/realtime`, `/backend-api/codex/realtime`  
[Native rules](native.md)

**Request:** WebSocket upgrade with original query parameters such as `model`, `intent` or `call_id`. Send original Realtime events, for example `session.update`, `input_audio_buffer.append`, `input_audio_buffer.commit` and conversation events. `audio` in append events is base64 audio in the negotiated format.

**Response:** full-duplex Realtime JSON text and native binary/control frames. Event payloads, transcription events, errors and close behavior are relayed; no Responses event conversion is applied. The configured timeout is an idle timeout.

The upstream is `https://api.openai.com/v1/realtime`. Use `X-Codex-Gateway-Authorization` for the gateway key and `Authorization` for an API key when starting a direct session. Codex 0.155.0 requires an API key for its direct Realtime connection. Its existing WebRTC-call sideband explicitly supports the ChatGPT authorization used to create that call; ordinary gateway Bearer mode reuses the gateway subscription for this case. Subscription access to arbitrary new public Realtime sessions is not established by the source.

[Pinned sideband authorization](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core/src/client.rs#L412) · [Pinned direct-session gate](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core/src/realtime_conversation.rs#L1773) · [Official WebSocket guide](https://developers.openai.com/api/docs/guides/voice-websockets?api=realtime)
