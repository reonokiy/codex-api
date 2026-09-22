# Oauth authorize

`GET /oauth/authorize`

**Request:** Browser authorization query response_type/client_id/redirect_uri/scope/code_challenge/state/etc.

**Response:** Interactive browser redirects.

Preserves response status, `Location` and cookies. This is a route relay, not a complete hosted sign-in callback; use the original redirect URI and PKCE flow. Browser links are not automatically rewritten.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/login/src/server.rs#L611) · [Native rules](native.md)
