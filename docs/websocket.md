# Responses over WebSocket

`GET /v1/responses` · `GET /codex/responses` · `GET /backend-api/codex/responses`  
[Common rules](common.md)

Connect using a WebSocket client with Bearer authorization. The gateway completes the upstream handshake before accepting the client, preserving HTTP handshake errors so Codex can fall back to HTTP.

## Client messages

Public SDK-style input:

```json
{"type":"response.create","model":"gpt-5.5","input":"Hello"}
```

The public endpoint accepts [Responses fields](responses.md), plus:

| Field | Type / behavior |
| --- | --- |
| `previous_response_id` | String; continuation on the upstream connection |
| `generate` | Boolean |
| `stream_id` | Opaque lane identifier, preserved |
| `client_metadata` | Object of string values |

The public adapter uses the original Codex WebSocket serializer and adds Responses Lite metadata when needed. Hosted `web_search` uses regular Responses for both HTTP and WebSocket requests. A `response.create` frame already containing `stream:true` is treated as a prepared native frame and preserved instead of adapted.

Native paths preserve text frames unchanged. Other event types, such as cancellation/steering, pass through to the upstream without public-schema conversion. Binary messages are relayed; ping/pong is handled independently by each connection.

## Server messages

Each text message contains a native event JSON object, for example:

```json
{"type":"response.output_text.delta","delta":"Hello","sequence_number":1}
```

Completion/failure events contain the upstream terminal response. Public mode fills an empty terminal `output` from preceding output-item events; native mode preserves it. Unknown events and binary messages remain intact.

Public validation errors are sent as `{"type":"error","error":{"type":"invalid_request_error","code":"invalid_request_error","message":"Invalid request","param":null}}`. Upstream close frames are relayed. Idle timeout closes with code `1011`. The gateway keeps one upstream connection per client connection; it does not persist continuation state across reconnects.
