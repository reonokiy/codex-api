# Feedback envelope

`POST /telemetry/sentry`

**Request:** Sentry envelope binary/text; x-sentry-auth.

**Response:** Sentry upload status/retry headers.

Original `x-sentry-auth` is preserved. This route sends no subscription Bearer credential.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/feedback/src/upload.rs#L42) · [Native rules](native.md)
