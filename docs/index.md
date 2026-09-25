# API reference

[Common rules](common.md) · [Native routing and credentials](native.md) · [Pinned inventory](../baseline/api-inventory.json) · [Official counterpart mapping](../baseline/openai-api-counterparts.json)

Each page describes one logical endpoint and its aliases. The inventory is a snapshot of first-party routes audited against Codex 0.155.1; the gateway now uses 0.157.0. Transparent forwarding does not establish entitlement or full public API compatibility. `field?` means optional/nullable in the inventory's client revision; server-owned and additional fields are preserved.

## Public adapters

| Endpoint | Reference |
| --- | --- |
| `POST /v1/responses` | [Responses](responses.md) |
| `GET /v1/responses` | [Responses WebSocket](websocket.md) |
| `POST /v1/responses/compact` | [Compaction](responses-compact.md) |
| `GET /v1/models` | [Models](models.md) |
| `POST /v1/files` | [File upload](files.md) |
| `POST /v1/images/generations` | [Image generation](images-generations.md) |
| `POST /v1/images/edits` | [Image editing](images-edits.md) |
| `POST /v1/realtime/calls` | [Realtime calls](realtime-calls.md) |
| `GET /v1/realtime` | [Realtime WebSocket](realtime.md) |
| `POST /v1/live/sessions` | [Current Live](live-sessions.md) |
| `/platform/*` | [Explicit Platform credentials](platform.md) |
| `GET /healthz` | [Health](health.md) |

## Native endpoints

| Method and path | Reference |
| --- | --- |
| `POST / GET /backend-api/codex/responses` | [responses](codex-responses.md) |
| `POST / GET /backend-api/codex/guardian` | [guardian](guardian.md) |
| `POST / GET /backend-api/codex/guardian-classifier` | [guardian classifier](guardian-classifier.md) |
| `GET /backend-api/codex/models` | [models](codex-models.md) |
| `POST /backend-api/codex/images/generations` | [image generations](images-generations.md) |
| `POST /backend-api/codex/images/edits` | [image edits](images-edits.md) |
| `POST /backend-api/codex/alpha/search` | [search](search.md) |
| `POST /backend-api/codex/memories/trace_summarize` | [memory trace summarize](memory-trace-summarize.md) |
| `POST /backend-api/codex/alpha/history/v2/list_windows` | [history list windows](history-list-windows.md) |
| `POST /backend-api/codex/alpha/history/v2/list_items` | [history list items](history-list-items.md) |
| `POST /backend-api/codex/alpha/history/v2/read_item` | [history read item](history-read-item.md) |
| `POST /backend-api/codex/alpha/history/v2/search_contents` | [history search contents](history-search-contents.md) |
| `POST /backend-api/codex/alpha/notes/v2/list_files_by_prefix` | [notes list files by prefix](notes-list-files-by-prefix.md) |
| `POST /backend-api/codex/alpha/notes/v2/read_file` | [notes read file](notes-read-file.md) |
| `POST /backend-api/codex/alpha/notes/v2/search_contents` | [notes search contents](notes-search-contents.md) |
| `POST /backend-api/codex/alpha/notes/v2/append_to_file` | [notes append to file](notes-append-to-file.md) |
| `POST /backend-api/codex/alpha/notes/v2/write_file` | [notes write file](notes-write-file.md) |
| `POST /backend-api/codex/alpha/notes/v2/thread_hint` | [notes thread hint](notes-thread-hint.md) |
| `POST /backend-api/codex/realtime/calls` | [realtime calls](realtime-calls.md) |
| `GET /v1/realtime` | [realtime](realtime.md) |
| `GET / POST /v1/live` | [realtime live](live.md) |
| `GET /v1/live/{call_id}` | [realtime live call](live-call.md) |
| `GET /backend-api/wham/accounts/check` | [accounts check](accounts-check.md) |
| `GET /backend-api/wham/profiles/me` | [profiles me](profiles-me.md) |
| `POST /backend-api/wham/accounts/send_add_credits_nudge_email` | [accounts credit nudge](accounts-credit-nudge.md) |
| `GET /backend-api/wham/tasks/list` | [tasks list](tasks-list.md) |
| `POST /backend-api/wham/tasks` | [task create](task-create.md) |
| `GET /backend-api/wham/tasks/{task_id}` | [task read](task-read.md) |
| `GET /backend-api/wham/tasks/{task_id}/turns/{turn_id}/sibling_turns` | [task sibling turns](task-sibling-turns.md) |
| `GET /backend-api/wham/config/bundle` | [config bundle](config-bundle.md) |
| `GET /backend-api/wham/settings/user` | [user settings](user-settings.md) |
| `GET /backend-api/wham/workspace-messages` | [workspace messages](workspace-messages.md) |
| `GET /backend-api/wham/usage` | [usage](usage.md) |
| `GET /backend-api/wham/rate-limit-reset-credits` | [reset credits list](reset-credits-list.md) |
| `POST /backend-api/wham/rate-limit-reset-credits/consume` | [reset credit consume](reset-credit-consume.md) |
| `POST /backend-api/wham/usage/thread-estimates/query` | [thread estimates](thread-estimates.md) |
| `POST /backend-api/wham/usage/thread_usage/query` | [thread usage](thread-usage.md) |
| `GET /backend-api/wham/environments` | [environments list](environments-list.md) |
| `GET /backend-api/wham/environments/by-repo/{provider}/{owner}/{repo}` | [environments by repo](environments-by-repo.md) |
| `POST /backend-api/files` | [file create](file-create.md) |
| `POST /backend-api/files/{file_id}/uploaded` | [file upload complete](file-upload-complete.md) |
| `GET / PUT /transfers/{handle}` | [signed transfers](transfers.md) |
| `GET /backend-api/connectors/directory/list` | [connectors directory](connectors-directory.md) |
| `GET /backend-api/connectors/directory/list_workspace` | [connectors workspace directory](connectors-workspace-directory.md) |
| `POST /backend-api/ps/apps/batch` | [apps batch](apps-batch.md) |
| `POST / GET / DELETE /backend-api/ps/mcp` | [apps mcp](apps-mcp.md) |
| `GET /backend-api/ps/plugins/suggested/codex` | [plugins suggested](plugins-suggested.md) |
| `GET /backend-api/ps/plugins/list` | [plugins list](plugins-list.md) |
| `GET /backend-api/ps/plugins/workspace/shared` | [plugins shared](plugins-shared.md) |
| `GET /backend-api/ps/plugins/installed` | [plugins installed](plugins-installed.md) |
| `GET /backend-api/ps/plugins/{plugin_id}` | [plugin read](plugin-read.md) |
| `GET /backend-api/ps/plugins/{plugin_id}/skills/{skill_name}` | [plugin skill read](plugin-skill-read.md) |
| `POST /backend-api/ps/plugins/{plugin_id}/install` | [plugin install](plugin-install.md) |
| `POST /backend-api/ps/plugins/{plugin_id}/uninstall` | [plugin uninstall](plugin-uninstall.md) |
| `GET /backend-api/ps/plugins/search` | [plugins search](plugins-search.md) |
| `GET /backend-api/ps/plugins/workspace/created` | [plugins created](plugins-created.md) |
| `PUT /backend-api/ps/plugins/{plugin_id}/shares` | [plugin share targets](plugin-share-targets.md) |
| `POST /backend-api/public/plugins/workspace/upload-url` | [plugin upload url](plugin-upload-url.md) |
| `POST /backend-api/public/plugins/workspace` | [plugin share create](plugin-share-create.md) |
| `POST /backend-api/public/plugins/workspace/{plugin_id}` | [plugin share update](plugin-share-update.md) |
| `DELETE /backend-api/public/plugins/workspace/{plugin_id}` | [plugin share delete](plugin-share-delete.md) |
| `GET /backend-api/plugins/featured` | [plugins featured legacy](plugins-featured-legacy.md) |
| `POST /backend-api/plugins/{plugin_id}/enable` | [plugin enable legacy](plugin-enable-legacy.md) |
| `POST /backend-api/plugins/{plugin_id}/uninstall` | [plugin uninstall legacy](plugin-uninstall-legacy.md) |
| `GET /backend-api/plugins/export/curated` | [plugins curated export](plugins-curated-export.md) |
| `POST /backend-api/wham/remote/control/server/enroll` | [remote enroll](remote-enroll.md) |
| `POST /backend-api/wham/remote/control/server/refresh` | [remote refresh](remote-refresh.md) |
| `POST /backend-api/wham/remote/control/server/pair` | [remote pair](remote-pair.md) |
| `POST /backend-api/wham/remote/control/server/pair/status` | [remote pair status](remote-pair-status.md) |
| `GET /backend-api/wham/remote/control/server` | [remote server](remote-server.md) |
| `GET /backend-api/wham/remote/control/environments/{environment_id}/clients` | [remote clients list](remote-clients-list.md) |
| `DELETE /backend-api/wham/remote/control/environments/{environment_id}/clients/{client_id}` | [remote client revoke](remote-client-revoke.md) |
| `GET /backend-api/wham/agent-identities/jwks` | [agent jwks](agent-jwks.md) |
| `POST /backend-api/codex/analytics-events/events` | [analytics events](analytics-events.md) |
| `POST /telemetry/costs` | [api key turn costs](api-key-turn-costs.md) |
| `POST /auth/oauth/token` | [oauth token](oauth-token.md) |
| `POST /auth/oauth/revoke` | [oauth revoke](oauth-revoke.md) |
| `POST /auth/api/accounts/deviceauth/usercode` | [device usercode](device-usercode.md) |
| `POST /auth/api/accounts/deviceauth/token` | [device token](device-token.md) |
| `GET /auth/api/accounts/v1/user-auth-credential/whoami` | [personal token whoami](personal-token-whoami.md) |
| `POST /auth/api/accounts/v1/agent/register` | [agent register](agent-register.md) |
| `POST /auth/api/accounts/v1/agent/{agent_runtime_id}/task/register` | [agent task register](agent-task-register.md) |
| `GET /oauth/authorize` | [oauth authorize](oauth-authorize.md) |
| `POST /telemetry/metrics` | [otel metrics](otel-metrics.md) |
| `POST /telemetry/sentry` | [feedback envelope](feedback-envelope.md) |
| `GET /backend-api/wham/app/appcast` | [desktop appcast](desktop-appcast.md) |
| `GET /distribution/chatgpt/codex/install.sh` | [cli installer sh](cli-installer-sh.md) |
| `GET /distribution/chatgpt/codex/install.ps1` | [cli installer ps1](cli-installer-ps1.md) |
| `GET /distribution/static/codex/pets/v1/{spritesheet_file}` | [pet assets](pet-assets.md) |
| `GET /distribution/static/codex-app-prod/appcast.xml` | [desktop appcast cdn](desktop-appcast-cdn.md) |
| `GET /distribution/static/codex-app-prod/appcast-x64.xml` | [desktop appcast cdn x64](desktop-appcast-cdn-x64.md) |
| `GET /distribution/static/codex-app-prod/windows-store-update.json` | [desktop windows update](desktop-windows-update.md) |
| `GET /distribution/static/codex-app-prod/Codex.dmg` | [desktop dmg arm64](desktop-dmg-arm64.md) |
| `GET /distribution/static/codex-app-prod/Codex-latest-x64.dmg` | [desktop dmg x64](desktop-dmg-x64.md) |

The [transfer endpoint](transfers.md) groups signed upload/download operations. Platform-only services such as standalone transcription, Skills uploads and Agents sessions retain their official contracts through `/platform/*`; they do not become subscription-backed adapters.

Local execution, stdio/UDS app-server RPC, third-party MCP/OAuth, alternative model providers and external distribution hosts are listed as separate interfaces in the inventory. They are outside these fixed first-party routes.
