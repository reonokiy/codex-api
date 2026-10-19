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

Each HTTP inference (including streaming and compact requests) and each
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

The Talos configuration routes OTLP through Alloy to Mimir, Tempo and Loki.
The provisioned `codex-api-usage` Grafana dashboard uses Loki usage events for
user/model filters, token totals and per-request details, and Mimir for rates
and latency. Request details are limited by Loki retention (currently 14 days)
and the query row limit. Deploy the gateway image containing this change
before enabling its telemetry environment variables.
