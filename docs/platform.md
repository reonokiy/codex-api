# Explicit Platform API forwarding

`/platform/*` → `https://api.openai.com/v1/*`  
[Native rules](native.md)

Send the original method, query, content type and body from the official API. For example, `/platform/audio/transcriptions` accepts the official multipart transcription request; `/platform/files` accepts the official Files API. Send an upstream API key in `Authorization` and the gateway key in `X-Codex-Gateway-Authorization`.

Responses retain the original status, end-to-end headers and body or WebSocket messages. Official resource IDs, organization/project permissions, billing and service limits apply. The gateway adds a 16 MiB request limit and its configured concurrency and timeout.

This namespace also reaches public Skills, Agents, Conversations, Realtime controls and usage APIs using their own credentials. Those resources differ from native plugin skills, cloud tasks, notes and subscription quota; see the [source-backed comparison](../baseline/openai-api-counterparts.json). There is no fabricated subscription-backed standalone transcription or generic resource lifecycle adapter.

[Official API reference](https://developers.openai.com/api/reference)
