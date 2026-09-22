# Official Python SDK compatibility

Run the gateway compatibility suite with the official `openai` Python package:

```sh
uv sync --locked
cargo sdk
```

This tests the gateway with the pinned official SDK, not a replacement SDK or the
SDK repository's own client-unit tests. Rust starts isolated loopback gateways and
controlled upstream fixtures. No account login or subscription usage is required.
Dependencies are managed with `pyproject.toml` and `uv.lock`.
Missing Python dependencies fail the suite instead of silently skipping it.

The SDK suite is also included in `cargo test-all`. Plain `cargo test` runs the
Rust regression tests; SDK tests are opt-in because they require Python packages.

## Coverage

| Area | SDK cases |
| --- | --- |
| Models and authentication | List, raw response, incorrect gateway key |
| Responses HTTP | Sync/async create, raw-response and streaming-response wrappers |
| SSE | Sync/async low-level streams, high-level `responses.stream`, final-response accumulation |
| WebSocket | Official sync/async `responses.connect`; same-connection continuation |
| Inputs | String, message arrays, typed content, image input, assistant history, encrypted reasoning, compaction history, function/custom-tool outputs, two-turn tool-call roundtrips |
| Reasoning | `context`: `auto`, `current_turn`, `all_turns`, null; explicit value preserved for standard and Lite models; legacy `generate_summary` alias |
| Tools | Function, custom grammar, namespaces, hosted web search; existing image/search SDK integration cases |
| Structured output | JSON schema, Pydantic `responses.parse`, parsed streaming, async variants; explicit plain-text format |
| Compaction | Official `responses.compact`, opaque compaction output; Rust verifies forwarded context and trigger |
| Files | Sync/async multipart `files.create`, using the native three-stage upload pipeline |
| Images | Official `images.generate` and multipart `images.edit` |
| Errors | SDK authentication/rate-limit exceptions, failed/incomplete responses, invalid enum values, unsupported parameters and endpoints |

The Responses matrix runs against one standard model and one Responses Lite model
from the pinned catalog. Rust inspects the upstream requests as well as the SDK
results: successful parsing alone is not sufficient. Model identifiers in these
synthetic fixtures do not establish real-account availability.

`artifacts/sdk/compatibility.json` records named cases and the SDK version. The
report labels synthetic fixtures explicitly and classifies every top-level
`responses.create` parameter in the installed SDK as supported or rejected. Independent cases continue after a
failure; any failed case makes the command fail. The parameter matrix separately
checks rejected options never reach upstream.

## Parameter audit

The public adapter explicitly maps parameters into Codex requests. It must not
accept options that disappear during serialization.

| Parameter | Behavior |
| --- | --- |
| `reasoning.context` | Accepted and forwarded; explicit `auto`/`current_turn` uses regular Responses because Lite requires `all_turns` |
| `reasoning.generate_summary` | Accepted as the deprecated alias of `summary`; supplying both is rejected as ambiguous |
| `text.format.type = "text"` | Accepted as default text output, without a JSON schema requirement |
| `text.format.type = "json_schema"` | Accepted with name and object schema; Pydantic SDK helpers supported |
| `reasoning.mode`, `text.format.type = "json_object"` | Not implemented by the pinned public adapter; rejected |
| `max_output_tokens`, `background`, HTTP `previous_response_id` | Not implemented; rejected instead of ignored |
| `store=true`, forced tool choice | Unsupported by the adapter; rejected |
| Unknown public request fields / invalid enum values | Rejected; native Responses continues preserving opaque request fields |

The original SDK reproduction returned HTTP 422 for all three valid context
values before reaching OpenAI. That was an adapter schema omission, not an
upstream model restriction. Live verification also found that Responses Lite rejects `auto` and `current_turn`.
The public adapter sends those explicit modes through regular Responses, preserving
the requested value, tools and instructions. Omitted/null context retains the
existing Lite default; the native endpoint is unaffected.

## Real-upstream checks

`cargo e2e` is separate and consumes subscription usage. It checks real model
responses, tool continuation, compaction, search, uploads, image generation/editing,
and a Realtime call. See [test setup](e2e.md). Synthetic SDK tests establish client
and wire compatibility; they cannot establish model behavior, entitlements, quota,
or production deployment status.

Realtime media, live account mutations, the Platform API namespace, and the full
OpenAI SDK product catalog are not covered by `cargo sdk`. Stored-response CRUD,
file listing, Chat Completions, and other unimplemented public resources have
negative tests; these do not imply support for those resources.
