# Otel metrics

`POST /telemetry/metrics`

**Request:** OTLP HTTP JSON metrics (OtelHttpProtocol::Json); statsig-api-key header.

**Headers:** `statsig-api-key`.

**Response:** OTLP collector HTTP response.

Original `statsig-api-key` is preserved. This route sends no subscription Bearer credential.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/otel/src/config.rs#L9) · [Native rules](native.md)
