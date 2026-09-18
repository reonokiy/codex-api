# Current Live sessions

`POST /v1/live/sessions` · `GET /v1/live/sessions` · Alias: `/platform/live/sessions`  
[Native rules](native.md)

Requires an upstream API credential in `Authorization` and the gateway key in `X-Codex-Gateway-Authorization`. These routes forward directly to Platform; the gateway does not convert a ChatGPT subscription into a current Live session.

**POST request:** JSON `{session:object,transport:{type:"webrtc",sdp:string}}` with the official session configuration.

**POST response:** upstream 201 JSON `{session:{id:string},transport:{type:"webrtc",sdp:string}}`, including additional upstream fields. The `sdp` is the answer.

**GET request/response:** WebSocket upgrade with the official session query/configuration; original messages and errors are forwarded. Attach to an existing session through [its attach endpoint](live-sessions-attach.md).

[Official Live API](https://developers.openai.com/api/docs/guides/voice-webrtc#understand-the-connection-sequence)
