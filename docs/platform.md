# Explicit Platform API forwarding

`/platform/*` → `https://api.openai.com/v1/*`  
[Native rules](native.md)

Send the original method, query, content type and body from the official API. For example, `/platform/audio/transcriptions` accepts the official multipart transcription request; `/platform/files` accepts the official Files API. Send an upstream API key in `Authorization` and the gateway key in `X-Codex-Gateway-Authorization`.

Responses retain the original status, end-to-end headers and body or WebSocket messages. Official resource IDs, organization/project permissions, billing and service limits apply. The gateway adds a 16 MiB request limit and its configured concurrency and timeout.

This namespace also reaches public Skills, Agents, Conversations, Realtime controls and usage APIs using their own credentials. Those resources differ from native plugin skills, cloud tasks, notes and subscription quota; see the [source-backed comparison](../baseline/openai-api-counterparts.json). Exact API-key routes `POST /v1/chat/completions`, `POST /v1/memories/trace_summarize` and `POST /v1/analytics/codex/turn-costs` also forward original requests using caller credentials.

Subscription-backed `POST /transcribe`, `POST /v1/audio/transcriptions` and `POST /backend-api/transcribe` forward multipart audio to ChatGPT’s `/backend-api/transcribe` using its [historical OAuth voice contract](https://github.com/openai/codex/blob/4e119a3b38e4a4decfccb003acecabc4614142b6/codex-rs/tui/src/voice.rs#L787), which sends file-only multipart. Broader model, output-format and streaming options depend on upstream support. Use `/platform/audio/transcriptions` for the public API-key Audio API.

[Official API reference](https://developers.openai.com/api/reference)
