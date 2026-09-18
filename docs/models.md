# Public model catalog

`GET /v1/models` · [Common rules](common.md)

No request body or required query parameters.

```json
{"object":"list","data":[{"id":"gpt-5.5","object":"model","created":0,"owned_by":"openai"}]}
```

The catalog is bundled with pinned Codex 0.155.0; it does not query current account availability. Public Responses validates model names against it. This is not a complete image-model or OpenAI Platform model directory; image endpoints default to `gpt-image-2` separately.

Use the [native catalog](codex-models.md) for the upstream Codex model metadata. No pagination or individual-model retrieval endpoint is implemented.
