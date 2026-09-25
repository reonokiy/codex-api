# Native routing

The gateway uses Codex **0.157.0** and retains the native route inventory audited against **0.155.1**. Source anchors in the endpoint pages refer to that inventory snapshot; upgrading the client does not establish continued upstream availability for every server-owned route. See the [endpoint index](index.md), [source inventory](../baseline/api-inventory.json) and [official API comparison](../baseline/openai-api-counterparts.json). These routes relay server-owned contracts; account entitlement and upstream validation still apply.

| Gateway prefix | Upstream prefix |
| --- | --- |
| `/codex/*`, `/backend-api/codex/*` | `https://chatgpt.com/backend-api/codex/*` |
| `/backend-api/*` | `https://chatgpt.com/backend-api/*` |
| `/api/codex/*` | `https://chatgpt.com/backend-api/wham/*` |
| `/api/codex/ps/mcp` | `https://chatgpt.com/backend-api/ps/mcp` |
| `/auth/*` | `https://auth.openai.com/*` |
| `/platform/*` | `https://api.openai.com/v1/*` |
| `/telemetry/costs` | `https://api.chatgpt.com/v1/analytics/codex/turn-costs` |
| `/telemetry/metrics` | `https://ab.chatgpt.com/otlp/v1/metrics` |
| `/telemetry/sentry` | `https://o33249.ingest.us.sentry.io/api/4510195390611458/envelope/` |
| `/distribution/chatgpt/*` | `https://chatgpt.com/*` |
| `/distribution/static/*` | `https://persistent.oaistatic.com/*` |

## Credentials

Subscription routes use the gateway's Codex credentials. Send `Authorization: Bearer <gateway-key>`. The gateway removes caller authorization, account ID and actor-auth headers, then adds the original subscription identity. Subscription authorization is refreshed through Codex's auth manager.

Platform, API-key turn costs, personal-token identity, OAuth browser authorization, and remote-control server/pairing routes need their own upstream credentials:

```http
Authorization: Bearer <upstream-credential>
X-Codex-Gateway-Authorization: Bearer <gateway-key>
```

The second header authenticates the gateway and is removed before forwarding. Original `Authorization`, account/project/organization headers and cookies remain available to the designated upstream. This header does not switch ordinary subscription routes to caller credentials; the [Realtime routes](realtime-calls.md) explicitly support both modes.

OAuth token/device exchange, agent task registration, JWKS, public distribution and telemetry use their original body/header authentication without a subscription Bearer token. Every gateway route still requires the gateway key when configured, except health and expiring [transfer URLs](transfers.md).

## Transport and client configuration

Transparent routes retain the HTTP method, query, body bytes, status, business/protocol headers and WebSocket messages. [Header cleanup](headers.md) removes downstream client identity and connection-local headers before forwarding; gateway auth, host, content length and WebSocket handshake fields are rebuilt for each connection. Redirects are returned to the caller. Requests are limited to 16 MiB; the configured timeout covers response setup and subsequent stream inactivity. Specialized Responses, Images, Models, search and Realtime routes apply the behavior documented on their own pages.

A Codex model provider's `base_url` redirects inference. Other ChatGPT backend clients use top-level `chatgpt_base_url = "http://127.0.0.1:8080/backend-api"`; they must also send valid gateway authentication. Some original client features require a local ChatGPT login regardless of the model provider setting. The actor marker below enables the image-tool gate only.

Auth, telemetry, distribution and some Realtime origins are fixed separately in Codex. Changing the model provider URL does not redirect them automatically: clients must select the documented gateway namespace through a supported override or a client routing change. Remote-control URL validation in the pin accepts ChatGPT domains and localhost, so an arbitrary Kubernetes hostname cannot be used as that override unchanged. Local tools and app-server RPC continue to execute in the calling Codex.

## Connect another Codex

Set `CODEX_GATEWAY_API_KEY` to the gateway key and add this to the client's `config.toml`:

```toml
model = "gpt-5.5"
model_provider = "gateway"

[model_providers.gateway]
name = "OpenAI"
base_url = "http://127.0.0.1:8080/backend-api/codex"
wire_api = "responses"
env_key = "CODEX_GATEWAY_API_KEY"
requires_openai_auth = false
supports_websockets = true
http_headers = { "x-openai-actor-authorization" = "gateway" }
```

Keep the provider name and URL suffix shown above. The actor header enables the
pinned client's image-tool capability; the gateway removes it before forwarding.
Image generation is enabled by default, or explicitly with
`codex --enable image_generation`. For native `web.run`, use
`codex --enable standalone_web_search --search`.

## History and notes

Every native history/notes request includes `context: {session_id:string,current_agent_name:string}`. Codex sends `x-openai-tool-output-truncation-policy` as serialized policy JSON; search/write/append operations may also send `x-openai-encrypted-tool-arguments: true`. Preserve these headers and encrypted payloads when generated by Codex.

Note paths address a virtual notes namespace. Relative paths use the current agent's directory; absolute paths select an agent directory. Empty components, `.` and `..` are invalid, and shell expansion is not supported. The pinned tool contract limits each note to 1,000,000 UTF-8 bytes. Direct reads reflect writes immediately; listings and searches may lag. Response DTOs are server-owned unless an endpoint page states otherwise.
