# Restricted API mode

By default, the gateway exposes only the inference allowlist below. Account,
stored-data and generic proxy APIs are disabled. To allow these restricted APIs
and enable all gateway routes, pass `--restricted-api` (or
`--restricted-api=true`) or set `CODEX_GATEWAY_RESTRICTED_API=true`. Here the
flag means "allow restricted APIs." Restart the gateway after changing it.

```sh
# Default: basic inference only.
cargo run --locked --release -- --listen 127.0.0.1:8080
# Explicitly enable the full API:
cargo run --locked --release -- --restricted-api
# Equivalent environment variable:
CODEX_GATEWAY_RESTRICTED_API=true cargo run --locked --release
```

For Docker, pass `-e CODEX_GATEWAY_RESTRICTED_API=true` before the image name
only when the full API is needed. Unset or `false` keeps the default restrictions.
The CLI flag takes precedence over the environment variable;
`--restricted-api=false` explicitly keeps the restrictions enabled.
The gateway key and all existing request limits still apply in either mode.

## Allowed endpoints

Only these complete method/path combinations are allowed. Queries do not
change the policy. No wildcard proxy prefix is allowed.

| Methods | Paths | Purpose |
| --- | --- | --- |
| GET, HEAD | `/healthz` | Gateway health; no gateway key required |
| GET, HEAD | `/v1/models`, `/codex/models`, `/backend-api/codex/models` | Available model catalog |
| POST, GET | `/v1/responses`, `/codex/responses`, `/backend-api/codex/responses` | Inference over HTTP/SSE or WebSocket |
| POST | `/v1/responses/compact`, `/codex/responses/compact`, `/backend-api/codex/responses/compact` | Context compaction |
| POST | `/v1/images/generations`, `/codex/images/generations`, `/backend-api/codex/images/generations` | Image generation |
| POST | `/v1/images/edits`, `/codex/images/edits`, `/backend-api/codex/images/edits` | Image editing |
| POST | `/v1/alpha/search`, `/codex/alpha/search`, `/backend-api/codex/alpha/search` | Standalone search |
| POST | `/v1/audio/transcriptions`, `/transcribe`, `/backend-api/transcribe` | Audio transcription |

Everything else returns HTTP **403** with `error.code = "endpoint_disabled"`,
including unknown routes, unsupported methods, and new endpoints added later.
Protected requests without a valid gateway key still return 401. Rejection
happens before body extraction, concurrency acquisition, upstream forwarding
or WebSocket upgrade. A trailing slash, encoded path, or extra path component
does not grant access to an unlisted route.

## Account and stored-data API audit

The following groups were reviewed against the router, native origin/auth
selection, and all `gateway_paths` in
[`baseline/api-inventory.json`](../baseline/api-inventory.json). All are
disabled in this mode, including aliases.

| Group | Existing routes or families | Account or data exposure |
| --- | --- | --- |
| Account identity and profiles | `/backend-api/wham/accounts/check`, `/backend-api/wham/profiles/me`; `/api/codex/` aliases | Account membership, capabilities and profile statistics |
| Settings and workspace | `/backend-api/wham/settings/user`, `/backend-api/wham/config/bundle`, `/backend-api/wham/workspace-messages`; `/api/codex/` aliases | User preferences, configuration and workspace content |
| Usage and credits | `/backend-api/wham/usage`, `/backend-api/wham/usage/thread-estimates/query`, `/backend-api/wham/usage/thread_usage/query`, `/backend-api/wham/rate-limit-reset-credits` and `/consume`, `/backend-api/wham/accounts/send_add_credits_nudge_email`; `/api/codex/` aliases | Subscription usage, per-thread estimates, credits and credit-related actions |
| Cloud tasks and environments | `/backend-api/wham/tasks/*`, `/backend-api/wham/tasks`, `/backend-api/wham/environments/*`, `/backend-api/wham/environments`; `/api/codex/` aliases | Tasks, task turns, repository and environment metadata |
| History, notes and memory | `/codex/alpha/history/v2/*`, `/codex/alpha/notes/v2/*`, `/codex/memories/trace_summarize`; `/backend-api/codex/` aliases | Stored conversation history and notes; excluded memory helpers |
| Files and transfers | `/v1/files`, `/backend-api/files`, `/backend-api/files/*`, `/transfers/*` | Account file uploads, upload completion and signed blob capabilities |
| Apps, connectors, plugins and skills | `/backend-api/connectors/*`, `/backend-api/ps/*`, `/backend-api/public/plugins/*`, `/backend-api/plugins/*`, `/api/codex/ps/mcp` | Connected apps, installed/shared plugins, workspace skills and mutation operations |
| Remote control | `/backend-api/wham/remote/control/*`; `/api/codex/` aliases | Enrolled servers, pairing, clients and control channels |
| Authentication and agent identity | `/auth/*`, `/oauth/authorize`, `/backend-api/wham/agent-identities/jwks`; `/api/codex/` alias | Token/device exchanges, revocation, personal-token identity and agent registration |
| Platform proxy | `/platform/*` | Arbitrary public API resources using caller credentials, including stored resources |
| Realtime, live and guardian | `/v1/realtime*`, `/codex/realtime*`, `/backend-api/codex/realtime*`, `/v1/live*`, `/codex/live*`, `/backend-api/codex/live*`, `/codex/guardian`, `/codex/guardian-classifier`; backend aliases | Excluded session/control APIs and specialized inference surfaces |
| Telemetry and distribution | `/telemetry/*`, `/codex/analytics-events/events`, `/backend-api/codex/analytics-events/events`, `/distribution/*`, `/backend-api/wham/app/appcast`; `/api/codex/` alias | Cost reporting, metrics, feedback, updates and assets are outside the minimal surface |
| Generic native forwarding | Unlisted `/backend-api/*`, `/codex/*`, `/api/codex/*`, `/auth/*` paths | Prevents unreviewed or future upstream account APIs from bypassing this mode |

Some excluded endpoints use the caller's upstream credential rather than the
gateway's subscription, and some serve public data. They remain excluded to
keep the exposed surface explicit and small.

## Scope

This is an HTTP endpoint policy. Generation still runs with the configured
ChatGPT subscription and consumes its quota. Model discovery, authentication
refresh and startup device login continue internally; they are not exposed
as caller-accessible authentication routes.

Model availability, native inference responses, generated files and upstream
headers can still reflect subscription capabilities or inference metadata.
This mode does not redact inference output, isolate callers' upstream state,
or restrict tool execution inside an allowed inference request.
Account-management features in a connected Codex client that need excluded
routes will fail with 403. File uploads and Realtime/live are also unavailable.
