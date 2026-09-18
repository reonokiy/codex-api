# Current Live session attachment

`GET /v1/live/sessions/{session_id}/attach` · Alias: `/platform/live/sessions/{session_id}/attach`  
[Native rules](native.md)

**Request:** WebSocket upgrade using the ID from [session creation](live-sessions.md). Send an upstream API credential in `Authorization` and the gateway key in `X-Codex-Gateway-Authorization`.

**Response:** original Live WebSocket messages, errors and close frames. The gateway forwards the existing session's protocol without conversion.

[Official Live API](https://developers.openai.com/api/docs/guides/voice-server-controls#attach-to-the-existing-session)
