# Feedback envelope

`POST /telemetry/sentry`

**Request:** Sentry envelope binary/text; x-sentry-auth.

**Response:** Sentry upload status/retry headers.

Original `x-sentry-auth` is preserved. This route sends no subscription Bearer credential.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/feedback/src/upload.rs#L42) · [Native rules](native.md)
