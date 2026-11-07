# Gateway usage telemetry

Set `OTEL_EXPORTER_OTLP_ENDPOINT=http://alloy.observability.svc.cluster.local:4318`
and `OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf` to enable outbound OTLP logs,
metrics and traces. `OTEL_SERVICE_NAME` defaults to `codex-api`; standard OTLP
endpoint, headers and timeout variables are supported. `OTEL_SDK_DISABLED=true`
disables collection. This is independent of the existing `/telemetry/metrics`
proxy route. Queued telemetry is best effort, not a billing ledger.

Set `CODEX_GATEWAY_TRUST_IDENTITY_HEADERS=true` only behind a trusted ingress.
Envoy maps verified `X-Keygate-User-Id` and `X-Keygate-Key-Id` authorization
response headers to `X-User-Id` and `X-Api-Key-Id`, overwriting client values.
These are conventional custom headers, not IETF standardized identity headers.
Internal routes overwrite the user with
`internal` and remove the key ID. Direct service access must be restricted;
without trusted identity a request is attributed to `unknown`. These headers
are never forwarded to the model provider.

Each Responses HTTP inference (including streaming, native aliases and compact requests) and each
WebSocket response.create generates an OTLP usage log and correlated span.
Logs include user/key IDs, request/response IDs, requested and returned model,
start time, duration, outcome, input/output/total tokens and cache/reasoning
details when supplied by the upstream. Missing usage remains missing, rather
than being reported as zero. Cache and reasoning counts are subsets of input
and output respectively; do not add them again to total tokens. Prompts,
responses, tool arguments and credentials are not collected.

Metrics use the development GenAI inference conventions:
`gen_ai.client.inference.usage.input_tokens`, `output_tokens`,
`cache_read.input_tokens`, `reasoning.output_tokens`, and
`gen_ai.client.inference.duration`. User and request IDs are excluded from
metric dimensions. `codex_api.usage.missing` counts requests without usage.
See the [upstream convention](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/client-inference.md).

## Per-request performance

The same OTLP pipeline also exports these gateway-specific histograms. These
names are custom extensions, separate from the development GenAI conventions
above. No additional configuration or telemetry endpoint is needed.

| Metric | Unit | Definition |
| --- | --- | --- |
| `codex_api.inference.time_to_first_token` | `s` | Time from inference start to the first non-empty generated content delta (proxy-observed TTFT) |
| `codex_api.inference.output_tokens_per_second` | `{token}/s` | Upstream-reported `output_tokens` divided by inference duration (overall TPS, including initial waiting time) |
| `codex_api.inference.duration` | `s` | Time from inference start to observation of the terminal response event |

Each usage JSON log includes `inference_duration_ms`, and, when available,
`ttft_ms` and `tps`. The correlated span and log attributes also include the
three metric names above with values in seconds and tokens/second. Histograms
keep the existing model/operation/transport/status dimensions; user, key,
request and response IDs remain confined to logs and traces.

The clock starts when a validated HTTP inference enters the execution path,
before concurrency acquisition and upstream setup/retries. For WebSocket it
starts separately for each generating `response.create`, after request parsing
and before forwarding. HTTP body upload, validation and WebSocket handshake
time are excluded.

TTFT ignores `response.created`, item creation, empty deltas and other metadata.
It recognizes text, refusal, visible reasoning/reasoning-summary, function-call
argument and custom-tool-input deltas. This measures the first observable model
content at the gateway, not server-internal token generation or the time the
caller receives it. Hidden reasoning before any visible delta is included in
the wait. A response with no recognized delta has no TTFT sample, even if it
reports output tokens in its terminal response.

TPS uses the terminal response's reported token count, including reasoning
tokens already contained in `output_tokens`. It never estimates tokens from
text length, event counts or chunk counts. Only completed, successful responses
with a positive output count and positive inference duration have a TPS sample.
Failed, incomplete, cancelled, missing-usage and zero-token responses omit TPS;
an observed TTFT can still be reported for them.

TPS is an **overall request rate**, not a decode-only model benchmark. For
example, 100 output tokens and a 5-second inference duration yield 20 tokens/s.
The denominator is not reduced by TTFT, because a delta can contain multiple
tokens and the reported count can include earlier hidden reasoning.

HTTP event timestamps are captured while reading upstream SSE, before internal
channel delivery. Timing freezes at the first terminal event, so later parser
completion, telemetry export and response-body draining do not extend inference
duration or lower TPS. Existing `duration_ms` and
`gen_ai.client.inference.duration` still measure the full guard lifetime and can
include downstream drain time. Upstream reads can themselves be delayed by
network or downstream backpressure; this is gateway-observed performance, not
isolated model execution time. Requests without a terminal event use their
lifetime for inference duration and never report TPS.

WebSocket timing is per response, including multiple turns on one connection.
Deltas are matched using response IDs, known output item IDs or an unambiguous
stream lane. Ambiguous multiplexed deltas are ignored instead of assigned to
another request. Deltas referring to unknown items also remain unattributed
unless they identify an active response explicitly. Terminal duplicates do not
produce duplicate records. Output
item routing is bounded and cleared as responses finish.

Performance metrics currently cover Responses and compact inference through
these instrumented paths. Generic native proxies, standalone images/search,
audio and Realtime are not given synthetic TTFT or TPS samples.

## HTTP ingress and response lifetime

All HTTP requests handled by the gateway router additionally produce a
`codex_api.http.request` JSON log and a correlated server span through the same
OTLP pipeline. This includes authentication/validation failures, disabled
endpoints, health/model requests, and streaming and buffered responses.

| Metric | Unit | Definition |
| --- | --- | --- |
| `codex_api.http.time_to_first_byte` | `s` | Request middleware entry to response headers ready (application-observed TTFB) |
| `codex_api.http.request.duration` | `s` | Request middleware entry to response-body end, body error, or cancellation |

The HTTP JSON log includes `ttfb_ms` when headers became ready,
`total_duration_ms`, `method`, a matched route template, `status_code`, a
generated request ID and `outcome`. Both timing values are also exported as
numeric span/log attributes using the metric names above. HTTP records are
separate from `codex_api.usage` records: filter by `event` when querying logs.
The HTTP request ID identifies that HTTP record and span; it is independent of
inference usage IDs. Supplied W3C trace context is honored for both.

The clock starts at the outer request middleware, before authorization,
body reading/decompression, validation, concurrency waiting and upstream
setup/retries. It excludes time before the HTTP server dispatches the request
(connection acceptance, TLS and header parsing). TTFB ends when the application
has the response headers ready, including for an SSE stream that has not
produced any content yet. It does not wait for the first body chunk or model
token. Actual socket writes, network delivery and client-observed TTFB require
measurement at the transport or client.

Total duration follows the response body until the final frame or EOF, including
time waiting for later stream chunks and downstream consumption as observed by
the application. Returning response headers does not end this timer. Body
frames, trailers, size hints and backpressure are preserved. An empty body
or HEAD response completes at headers. Kernel/proxy buffers and delivery to
the client's application are outside this measurement.

`outcome` is `completed` for a normally exhausted body, `body_error` for a
body read error, or `cancelled` if handling/body consumption is dropped early.
Cancellation duration stops at that drop and must not be treated as a
successfully completed response duration. A cancellation before headers has
no TTFB or status code. HTTP error responses with a fully consumed body have
`completed` outcome and their actual HTTP error status. A `101` upgrade has
`upgraded` outcome and measures the HTTP handshake only; the upgraded socket's
lifetime is excluded. WebSocket inference continues to use per-turn TTFT/TPS.

Histogram dimensions are limited to method (unknown methods use `_OTHER`),
matched route template (fallback paths use `unmatched`), HTTP status when known,
and outcome. Raw paths, query strings, user/key/request IDs and response content
are never metric dimensions. HTTP telemetry does not add synthetic token
measurements to endpoints without upstream usage.

The Talos configuration routes OTLP through Alloy to Mimir, Tempo and Loki.
The provisioned `codex-api-usage` Grafana dashboard uses Loki usage events for
user/model filters, token totals and per-request details, and Mimir for rates
and latency. Request details are limited by Loki retention (currently 14 days)
and the query row limit. Deploy the gateway image containing this change
before enabling its telemetry environment variables.
