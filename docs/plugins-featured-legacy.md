# Plugins featured legacy

`GET /backend-api/plugins/featured`

**Request:** Query platform. Saved authentication is added only when `uses_codex_backend` is true; otherwise the route forwards without subscription authentication. Gateway authentication still applies.

**Response:** string[] plugin IDs.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/core-plugins/src/remote_legacy.rs#L129) · [Native rules](native.md)
