# Explicit Platform API forwarding

`/platform/*` → `https://api.openai.com/v1/*`  
[Native rules](native.md)

Send the original method, query, content type and body from the official API. For example, `/platform/audio/transcriptions` accepts the official multipart transcription request; `/platform/files` accepts the official Files API. Send an upstream API key in `Authorization` and the gateway key in `X-Codex-Gateway-Authorization`.

Responses retain the original status, end-to-end headers and body or WebSocket messages. Official resource IDs, organization/project permissions, billing and service limits apply. The gateway adds a 16 MiB request limit and its configured concurrency and timeout.

This namespace also reaches public Skills, Agents, Conversations, Realtime controls and usage APIs using their own credentials. Those resources differ from native plugin skills, cloud tasks, notes and subscription quota; see the [source-backed comparison](../baseline/openai-api-counterparts.json).

[Official API reference](https://developers.openai.com/api/reference)

## OAuth audio transcription

`POST /v1/audio/transcriptions`, `POST /transcribe` and the existing `POST /backend-api/transcribe` forward to the gateway's configured `chatgpt_base_url` plus `/transcribe`. The default base is `https://chatgpt.com/backend-api`, so the default upstream URL is `https://chatgpt.com/backend-api/transcribe`.

These routes use the gateway's saved ChatGPT OAuth credentials upstream. Send the gateway key in `Authorization`; the caller's key is not forwarded as an OpenAI API key. See [authentication and limits](common.md).

Send a multipart `file` part containing WAV audio:

```sh
curl http://127.0.0.1:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $CODEX_GATEWAY_API_KEY" \
  -F 'file=@audio.wav;type=audio/wav'
```

The [historical Codex OAuth client](https://github.com/openai/codex/blob/4e119a3b38e4a4decfccb003acecabc4614142b6/codex-rs/tui/src/voice.rs#L787) sends only this file part and reads a JSON `text` field. This is the source-backed contract for these aliases. The gateway forwards the original multipart bytes, content type and query, and preserves upstream status, end-to-end headers and response body, including errors. The shared 16 MiB request limit, concurrency and timeout apply.

The aliases do not implement the full OpenAI Audio contract: model selection, prompt handling, output formats and streaming options are not implemented by the gateway. Additional fields are passed through for the upstream to accept or reject. Historical source and local forwarding tests do not establish current account entitlement or live upstream availability. Use `/platform/audio/transcriptions` with your own OpenAI API key for the official Platform contract described above.
