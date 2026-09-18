# Otel metrics

`POST /telemetry/metrics`

**Request:** OTLP HTTP JSON metrics (OtelHttpProtocol::Json); statsig-api-key header.

**Headers:** `statsig-api-key`.

**Response:** OTLP collector HTTP response.

Original `statsig-api-key` is preserved. This route sends no subscription Bearer credential.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/otel/src/config.rs#L9) · [Native rules](native.md)
