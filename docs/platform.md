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

If transcription returns HTML, inspect the audio log entries: `audio request received` records the incoming method and path, `audio upstream request` records the effective upstream host and path, and `audio upstream response` records status and Content-Type. HTML responses and unsuccessful statuses produce warnings; the original response remains unchanged. `audio upstream request failed` records a fixed failure kind for transport/API errors. These entries exclude queries, credentials and bodies.

The default log filter is `codex_api_gateway=info`. If `RUST_LOG` overrides it, include `codex_api_gateway=info` to see incoming and outgoing audio entries. Reproduce the request and compare the effective target with the expected `/backend-api/transcribe`; a custom `chatgpt_base_url` is used as configured. An incoming entry without an upstream entry points to a failure before forwarding, such as gateway authorization. No incoming entry means the request did not reach an audio handler in this running gateway, or the log filter hides it.

For transport comparison, set `RUST_LOG=codex_api_gateway=debug`. `audio upstream wire request` records the effective User-Agent, originator, Accept, Content-Type, request byte count, explicit Cookie-header presence, whether the shared cookie jar is enabled, and whether additional ChatGPT cookies are configured on the factory. The configured-cookie flag does not inspect infrastructure cookies retained by the shared jar; the explicit Cookie flag does not show cookies attached automatically by it. `audio upstream wire response` records the actual HTTP response version, Server, CF-Mitigated, X-OAI-Request-ID or X-Request-ID, and CF-Ray. These DEBUG entries exclude authorization, cookie values, account identifiers, bodies, full URLs, and queries. `audio input metadata` records request size and whether `X-Codex-Base64` is `1`. For non-base64 requests, `audio input multipart` records parse success or a fixed error kind, `audio input multipart field` records recognized field names and sizes (other names are labeled `other`), `audio input model` records at most 128 model characters, and `audio input file` records MIME type, safe extension, byte count and magic-based container classification. These diagnostics inspect a copy of the incoming body without changing forwarding. They exclude filenames, audio bytes, prompt and language values; base64 bodies are not decoded for diagnostics.
