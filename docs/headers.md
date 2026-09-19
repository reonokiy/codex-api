# Gateway header handling

The gateway opens its own upstream connections. All request paths share cleanup
of downstream client identity and connection-local fields, including native
forwarding, Responses HTTP/WebSocket, Images, search, Realtime and signed transfers.
Codex upstream clients use their own default identity headers.

Removed client metadata includes:

- `User-Agent`, `originator`, `Origin`, `Referer`, language/encoding preferences,
  browser priority, tracking preferences and browser client hints.
- `x-stainless-*`, `sec-ch-*`, `sec-fetch-*`, `x-openai-client-user-agent` and
  related browser/SDK identity fields.
- `Forwarded`, `x-forwarded-*`, `Via`, real/client IP fields and downstream
  Cloudflare connection metadata.
- Gateway-specific credentials and generic `x-api-key`/`api-key` headers.

`Connection`, `Proxy-Connection`, the standard hop-by-hop headers and every field
nominated by **any** `Connection` value are removed. This applies to responses too,
including the Responses metadata allowlist. WebSocket keys, versions, accept
values and extension negotiation are regenerated; needed subprotocol negotiation
is preserved.

Authentication is checked before cleaning. Subscription routes discard caller
authorization, cookies and account/project/organization selectors, then use the
gateway's own subscription. Explicit passthrough routes retain their designated
upstream `Authorization`, cookies and account/project/organization fields;
`X-Codex-Gateway-Authorization` never leaves the gateway. Merely sending that
header does not change a subscription route into a passthrough route.

Protocol/business headers remain available according to each route's existing
contract: beta/version flags, session/turn metadata, MCP headers, tracing,
idempotency keys, content types, conditions and byte ranges. Native forwarding
retains unknown business headers rather than maintaining a fragile full allowlist.
Typed adapters retain their narrower protocol allowlist. Signed storage transfers
also strip account credentials, while keeping storage-specific headers and ranges.

This cleanup does not rewrite business payloads or response cookies, and it does
not claim to remove arbitrary secrets hidden in unknown custom business headers.
