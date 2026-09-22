# Public model catalog

`GET /v1/models` · [Common rules](common.md)

No request body or required query parameters.

```json
{"object":"list","data":[{"id":"gpt-5.5","object":"model","created":0,"owned_by":"openai"}]}
```

At startup, the gateway fetches the current account catalog using Codex 0.155.1. Public Responses (HTTP and WebSocket) and compaction validate model names and capabilities against this snapshot, so newly available models such as `gpt-6-sol` and `gpt-6-luna` do not require a new bundled catalog. Restart the gateway to refresh it.

If fetching fails, times out (after at most 30 seconds), or returns an empty/invalid catalog, startup logs a warning and falls back to the pinned Codex bundled catalog. Models absent from that fallback remain unavailable through public Responses. This is not a complete image-model or OpenAI Platform model directory; image endpoints default to `gpt-image-2` separately.

Use the [native catalog](codex-models.md) for the upstream Codex model metadata. No pagination or individual-model retrieval endpoint is implemented.
