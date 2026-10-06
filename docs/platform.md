# Explicit Platform API forwarding

`/platform/*` → `https://api.openai.com/v1/*`  
[Native rules](native.md)

Send the original method, query, content type and body from the official API. For example, `/platform/audio/transcriptions` accepts the official multipart transcription request; `/platform/files` accepts the official Files API. Send an upstream API key in `Authorization` and the gateway key in `X-Codex-Gateway-Authorization`.

Responses retain the original status, end-to-end headers and body or WebSocket messages. Official resource IDs, organization/project permissions, billing and service limits apply. The gateway adds a 16 MiB request limit and its configured concurrency and timeout.

This namespace also reaches public Skills, Agents, Conversations, Realtime controls and usage APIs using their own credentials. Those resources differ from native plugin skills, cloud tasks, notes and subscription quota; see the [source-backed comparison](../baseline/openai-api-counterparts.json).

[Official API reference](https://developers.openai.com/api/reference)

## OAuth audio forwarding

`/v1/audio` and `/v1/audio/*` forward to `chatgpt_base_url` plus `/audio` and `/audio/*`, defaulting to `https://chatgpt.com/backend-api/audio`. The gateway uses saved ChatGPT OAuth credentials and preserves the request method, query, content type and body, and the upstream response status, headers and body. Available operations depend on the backend API.

### Transcription

`POST /v1/audio/transcriptions` · Aliases: `/transcribe`, `/backend-api/transcribe`

These routes use the gateway's saved ChatGPT OAuth credentials and forward to `chatgpt_base_url` plus `/transcribe`, defaulting to `https://chatgpt.com/backend-api/transcribe`. Send the gateway key in `Authorization`; see [authentication and limits](common.md).

Send a multipart `file` part containing WAV audio:

```sh
curl http://127.0.0.1:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $CODEX_GATEWAY_API_KEY" \
  -F 'file=@audio.wav;type=audio/wav'
```

The [Codex OAuth client](https://github.com/openai/codex/blob/4e119a3b38e4a4decfccb003acecabc4614142b6/codex-rs/tui/src/voice.rs#L787) reads the transcript from the JSON `text` field. Requests and upstream responses, including errors, are forwarded unchanged. The shared request limit is 16 MiB.
