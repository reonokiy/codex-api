# Remote server

`GET /backend-api/wham/remote/control/server` · Alias: `/api/codex/remote/control/server`

**Request:** WebSocket upgrade.

**Headers:** `x-codex-server-id`, `x-codex-name`, `x-codex-protocol-version`, `x-codex-installation-id`, `x-codex-host-device-kind`, `x-codex-subscribe-cursor`.

**Response:** Multiplexed remote-control WebSocket protocol carrying app-server messages.

Use the issued `remote_control_token` as upstream Bearer credentials and the separate gateway auth header. Envelopes use a flattened `type` discriminator plus `client_id`, `stream_id`, `seq_id`; client messages may include `cursor`. Segmented messages retain `segment_id`, `segment_count`, `message_size_bytes`, `message_chunk_base64`. The gateway relays frames; the Codex client implements reconnection and app-server RPC.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/app-server-transport/src/transport/remote_control/protocol.rs#L236) · [Native rules](native.md)
